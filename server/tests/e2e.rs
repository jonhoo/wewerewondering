#![cfg(feature = "e2e-test")]

use aws_sdk_dynamodb::types::AttributeValue;
use fantoccini::wd::WebDriverCompatibleCommand;
use fantoccini::{Client, ClientBuilder, Locator};
use serial_test::serial;
use std::io;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;
use url::{ParseError, Url};

type ServerTaskHandle = JoinHandle<Result<(), io::Error>>;

const TESTRUN_SETUP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

static WEBDRIVER_ADDRESS: LazyLock<String> = LazyLock::new(|| {
    let port = std::env::var("WEBDRIVER_PORT")
        .ok()
        .unwrap_or("4444".into());
    format!("http://localhost:{}", port)
});

async fn init_webdriver_client() -> Client {
    let mut chrome_args = Vec::new();
    if std::env::var("HEADLESS").ok().is_some() {
        chrome_args.extend(["--headless", "--disable-gpu", "--disable-dev-shm-usage"]);
    }
    let mut caps = serde_json::map::Map::new();
    caps.insert(
        "goog:chromeOptions".to_string(),
        serde_json::json!({
            "args": chrome_args,
        }),
    );
    ClientBuilder::native()
        .capabilities(caps)
        .connect(&WEBDRIVER_ADDRESS)
        .await
        .expect("web driver to be available")
}

async fn init() -> (String, ServerTaskHandle) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        let app = wewerewondering_api::new().await;
        let app = app.fallback_service(ServeDir::new(
            std::env::current_dir().unwrap().join("../client/dist"),
        ));
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        let assigned_addr = listener.local_addr().unwrap();
        tx.send(assigned_addr).unwrap();
        axum::serve(listener, app.into_make_service()).await
    });
    let assigned_addr = tokio::time::timeout(TESTRUN_SETUP_TIMEOUT, rx)
        .await
        .expect("test setup to not have timed out")
        .expect("socket address to have been received from the channel");
    let app_addr = format!("http://localhost:{}", assigned_addr.port());
    (app_addr, handle)
}

struct TestContext {
    fantoccini: Client,
    dynamo: aws_sdk_dynamodb::Client,
    url: String,
}

// With out tests setup, we've got isolated sessions and a dedicated
// app per test but we still cannot run certain tests in parallel, e.g.
// we are currently missing clipboard isolation and need to run tests
// that are accessing navigator.clipboard _sequentially_.
macro_rules! serial_test {
    ($test_name:ident, $test_fn:expr) => {
        #[tokio::test(flavor = "multi_thread")]
        #[serial]
        async fn $test_name() {
            let (app_addr, _) = init().await;
            let c = init_webdriver_client().await;
            let dynamodb_client = wewerewondering_api::init_dynamodb_client().await;
            let ctx = TestContext {
                fantoccini: c.clone(),
                dynamo: dynamodb_client,
                url: app_addr,
            };
            // run the test as a task catching any errors
            let res = tokio::spawn($test_fn(ctx)).await;
            // clean up and ...
            c.close().await.unwrap();
            //  ... fail the test, if errors returned from the task
            if let Err(e) = res {
                std::panic::resume_unwind(Box::new(e));
            }
        }
    };
}

#[derive(Debug, Clone)]
struct GrantClipboardReadCmd;

impl WebDriverCompatibleCommand for GrantClipboardReadCmd {
    fn endpoint(&self, base_url: &Url, session_id: Option<&str>) -> Result<Url, ParseError> {
        base_url.join(format!("session/{}/permissions", session_id.as_ref().unwrap()).as_str())
    }

    fn method_and_body(&self, _: &url::Url) -> (http::Method, Option<String>) {
        (
            http::Method::POST,
            Some(
                serde_json::json!({"descriptor": {"name": "clipboard-read"}, "state": "granted"})
                    .to_string(),
            ),
        )
    }
}

// ------------------------------- TESTS --------------------------------------

async fn start_new_q_and_a_session(
    TestContext {
        fantoccini,
        dynamo,
        url,
    }: TestContext,
) {
    // the host novigates to the app's welcome page
    fantoccini.goto(&url).await.unwrap();
    assert_eq!(
        fantoccini.current_url().await.unwrap().as_ref(),
        format!("{}/", url)
    );
    assert_eq!(fantoccini.title().await.unwrap(), "Q&A");
    let create_event_btn = fantoccini
        .wait()
        .at_most(DEFAULT_WAIT_TIMEOUT)
        .for_element(Locator::Id("create-event-button"))
        .await
        .unwrap();

    // starts a new Q&A session and ...
    create_event_btn.click().await.unwrap();

    // ... gets redirected to the event's host view where they can ...
    let share_event_btn = fantoccini
        .wait()
        .at_most(DEFAULT_WAIT_TIMEOUT)
        .for_element(Locator::Id("share-event-button"))
        .await
        .unwrap();
    let event_url_for_host = fantoccini.current_url().await.unwrap();
    let mut params = event_url_for_host.path_segments().unwrap();
    assert_eq!(params.next().unwrap(), "event");
    let event_id = params.next().unwrap();
    let event_secret = params.next().unwrap();
    assert!(params.next().is_none());

    // ... grab the event's guest url to share it later with folks
    share_event_btn.click().await.unwrap();
    fantoccini.issue_cmd(GrantClipboardReadCmd).await.unwrap();
    let event_url_for_guest: Url = fantoccini
        .execute_async(
            r#"
                const [callback] = arguments;
                navigator.clipboard.readText().then((text) => callback(text));
            "#,
            vec![],
        )
        .await
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let mut params = event_url_for_guest.path_segments().unwrap();
    assert_eq!(params.next().unwrap(), "event");
    let event_id_for_guest = params.next().unwrap();

    assert_eq!(event_id_for_guest, event_id); // same event id
    assert!(params.next().is_none()); // but no secret

    // and there are currently no pending, answered, or hidden questions
    // related to the newly created event
    let pending_questions = fantoccini
        .find(Locator::Id("pending-questions"))
        .await
        .unwrap()
        .find_all(Locator::Css("article"))
        .await
        .unwrap();
    assert!(pending_questions.is_empty());
    assert!(fantoccini
        .find(Locator::Id("answered-questions"))
        .await
        .unwrap_err()
        .is_no_such_element());
    assert!(fantoccini
        .find(Locator::Id("hidden-questions"))
        .await
        .unwrap_err()
        .is_no_such_element());

    // let's make sure we are persisting the event...
    let event = dynamo
        .get_item()
        .table_name("events")
        .key("id", AttributeValue::S(event_id.into()))
        .send()
        .await
        .unwrap()
        .item
        .unwrap();
    assert_eq!(
        event.get("id").unwrap(),
        &AttributeValue::S(event_id.into())
    );
    assert_eq!(
        event.get("secret").unwrap(),
        &AttributeValue::S(event_secret.into())
    );
    // ... and there are actually no questions associated
    // with that event
    assert_eq!(
        dynamo
            .query()
            .table_name("questions")
            .index_name("top")
            .key_condition_expression("eid = :eid")
            .expression_attribute_values(":eid", AttributeValue::S(event_id.into()))
            .send()
            .await
            .unwrap()
            .count,
        0
    )
}

serial_test!(test_start_new_q_and_a_session, start_new_q_and_a_session);
