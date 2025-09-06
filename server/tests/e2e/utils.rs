use axum_reverse_proxy::ReverseProxy;
use fantoccini::wd::WebDriverCompatibleCommand;
use fantoccini::Locator;
use fantoccini::{elements::Element, error::CmdError};
use std::io;
use std::ops::Deref;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use url::{ParseError, Url};

pub(crate) type ServerTaskHandle = JoinHandle<Result<(), io::Error>>;

pub(crate) const TESTRUN_SETUP_TIMEOUT: Duration = Duration::from_secs(5);
// this is also configurable via `WAIT_TIMEOUT` environment variable:
// when testing with SAM local setup - especially on shared CI runners - you
// might want to increase this
pub(crate) const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

static WEBDRIVER_ADDRESS: LazyLock<String> = LazyLock::new(|| {
    let port = std::env::var("WEBDRIVER_PORT")
        .ok()
        .unwrap_or("4444".into());
    format!("http://localhost:{}", port)
});

pub(crate) static WAIT_TIMEOUT: LazyLock<Duration> = LazyLock::new(|| {
    std::env::var("WAIT_TIMEOUT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(DEFAULT_WAIT_TIMEOUT)
});

#[derive(Debug, Clone)]
pub(crate) struct Client {
    pub homepage: Url,
    pub fantoccini: fantoccini::Client,
    pub wait_timeout: Duration,

    /// Front-end's poll interval.
    ///
    /// We are using a polling approach in our front-end (as opposed to socket connection
    /// or server-sent events) mainly to be able to use serverless architecture.
    /// This implies, for example, that there is at least a polling interval delay
    /// between one Q&A session participant upvoting a question and all others
    /// see the counter go up. To "accelerate" this, we are specifying a lower
    /// polling interval when building the front-end for end-to-end test run.
    /// We are storing this interval here to know how long (at the very least)
    /// we should be awaiting prior to making assertions in some test scenarios.
    pub poll_interval: Duration,
}

impl Deref for Client {
    type Target = fantoccini::Client;
    fn deref(&self) -> &Self::Target {
        &self.fantoccini
    }
}

impl Client {
    pub(crate) fn into_inner(self) -> fantoccini::Client {
        self.fantoccini
    }

    pub(crate) async fn goto_homepage(&self) {
        self.goto(self.homepage.as_str()).await.unwrap();
    }

    /// Await one data polling interval.
    ///
    /// Internally, will call [`tokio::time::sleep`] with [`Client::poll_interval`]
    /// scaled a bit to adjust for some latency and resource-constrained test
    /// runners.
    pub(crate) async fn wait_for_polling(&self) {
        tokio::time::sleep(*WAIT_TIMEOUT).await;
    }

    /// Wait for an element with default timeout.
    ///
    /// Internally, uses [`fantoccini::Client::wait_for`] with the timeout
    /// specified in the test module.
    pub(crate) async fn wait_for_element(&self, locator: Locator<'_>) -> Result<Element, CmdError> {
        self.wait()
            .at_most(self.wait_timeout)
            .for_element(locator)
            .await
    }

    /// Wait for pending questions on the current page.
    ///
    /// Internally, uses [`Client::wait_for_element`] specifying the identifier
    /// of the unanswered questions container and selecting the items it holds.
    pub(crate) async fn wait_for_pending_questions(&self) -> Result<Vec<Element>, CmdError> {
        self.wait_for_element(Locator::Id("pending-questions"))
            .await?
            .find_all(Locator::Css("article"))
            .await
    }

    /// Creates a new Q&A session and returns a guest link.
    ///
    /// Internally, will navigate to the app's homepage, locate and click
    /// the "Open new Q&A session" button, then - in the event's page already -
    /// locate and click the "Share event" button, and read the event's guest url
    /// from the clipboard.
    ///
    /// The client will end up in the newly created event's _host_ page, so if
    /// you need the url with the host's secret, just call `current_url` on the
    /// client (see [`fantoccini::Client::current_url`]).
    pub(crate) async fn create_event(&self) -> Url {
        // go to homepage and create a new event
        self.goto(self.homepage.as_str()).await.unwrap();
        self.wait_for_element(Locator::Id("create-event-button"))
            .await
            .unwrap()
            .click()
            .await
            .unwrap();
        // wait for the event's page
        self.wait_for_element(Locator::Id("share-event-button"))
            .await
            .unwrap()
            .click()
            .await
            .unwrap();
        // figure the guests' url
        self.issue_cmd(GrantClipboardReadCmd).await.unwrap();
        self.execute_async(
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
        .unwrap()
    }
}

pub(crate) async fn init_webdriver_client() -> fantoccini::Client {
    let mut chrome_args = Vec::new();
    if std::env::var("HEADLESS").ok().is_some() {
        chrome_args.extend([
            "--headless",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--no-sandbox",
        ]);
    }
    let mut caps = serde_json::map::Map::new();
    caps.insert(
        "goog:chromeOptions".to_string(),
        serde_json::json!({
            "args": chrome_args,
        }),
    );
    fantoccini::ClientBuilder::native()
        .capabilities(caps)
        .connect(&WEBDRIVER_ADDRESS)
        .await
        .expect("web driver to be available")
}

pub(crate) async fn init() -> (Url, ServerTaskHandle) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        let app = match std::env::var("BACKEND_URL") {
            Err(_) => wewerewondering_api::new().await,
            Ok(url) => ReverseProxy::new("/api", &format!("{}/api", url)).into(),
        };
        let app = app
            // similar to what AWS Cloudfront (see `infra/index-everywhere.js`)
            // does for us at edge
            .route_service(
                "/event/{*params}",
                ServeFile::new("../client/dist/index.html"),
            )
            // our front-end distribution
            .fallback_service(ServeDir::new(
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
    let app_addr = format!("http://localhost:{}", assigned_addr.port())
        .parse()
        .unwrap();
    (app_addr, handle)
}

pub(crate) struct TestContext {
    pub host: Client,
    pub guest1: Client,
    pub guest2: Client,
    pub dynamo: aws_sdk_dynamodb::Client,
}

/// With our test setup, we've got isolated sessions and a dedicated
/// app per test but we still cannot run certain tests in parallel, e.g.
/// we are currently missing clipboard isolation and need to run tests
/// that are accessing `navigator.clipboard` _sequentially_.
///
/// Usage:
/// ```no_run
/// async fn start_new_q_and_a_session(ctx: TestContext) {
///     // test logic here
/// }
///
/// async fn guest_asks_question(ctx: TestContext) {
///     // test logic here
/// }
///
/// mod tests {
///     crate::serial_test!(start_new_q_and_a_session);
///     crate::serial_test!(guest_asks_question);
/// }
/// ```
#[macro_export]
macro_rules! serial_test {
    ($test_fn:ident) => {
        #[tokio::test(flavor = "multi_thread")]
        #[::serial_test::serial]
        async fn $test_fn() {
            let (app_addr, handle) = $crate::utils::init().await;
            let (f1, f2, f3) = tokio::join!(
                tokio::spawn($crate::utils::init_webdriver_client()),
                tokio::spawn($crate::utils::init_webdriver_client()),
                tokio::spawn($crate::utils::init_webdriver_client()),
            );
            let poll_interval = std::time::Duration::from_millis(1000);
            let host = $crate::utils::Client {
                homepage: app_addr.clone(),
                fantoccini: f1.unwrap(),
                wait_timeout: *$crate::utils::WAIT_TIMEOUT,
                poll_interval,
            };
            let guest1 = $crate::utils::Client {
                homepage: app_addr.clone(),
                fantoccini: f2.unwrap(),
                wait_timeout: *$crate::utils::WAIT_TIMEOUT,
                poll_interval,
            };
            let guest2 = $crate::utils::Client {
                homepage: app_addr.clone(),
                fantoccini: f3.unwrap(),
                wait_timeout: *$crate::utils::WAIT_TIMEOUT,
                poll_interval,
            };
            let dynamodb_client = wewerewondering_api::init_dynamodb_client().await;
            let ctx = super::TestContext {
                host: host.clone(),
                guest1: guest1.clone(),
                guest2: guest2.clone(),
                dynamo: dynamodb_client,
            };
            // run the test as a task catching any errors
            let res = tokio::spawn(super::$test_fn(ctx)).await;
            // clean up and ...
            host.into_inner().close().await.unwrap();
            guest1.into_inner().close().await.unwrap();
            guest2.into_inner().close().await.unwrap();
            handle.abort();
            //  ... fail the test, if errors returned from the task
            if let Err(e) = res {
                std::panic::resume_unwind(Box::new(e));
            }
        }
    };
}

#[derive(Debug, Clone)]
pub(crate) struct GrantClipboardReadCmd;

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
