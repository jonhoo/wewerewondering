use aws_sdk_dynamodb::{error::SdkError, types::AttributeValue};
use aws_smithy_types::body::SdkBody;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use http::StatusCode;
use http_body_util::BodyExt;
use lambda_http::Error;
use std::time::{Duration, SystemTime};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tower::Layer;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tower_service::Service;
use tracing_subscriber::EnvFilter;
use ulid::Ulid;

const QUESTIONS_EXPIRE_AFTER_DAYS: u64 = 30;
const QUESTIONS_TTL: Duration = Duration::from_secs(QUESTIONS_EXPIRE_AFTER_DAYS * 24 * 60 * 60);

const EVENTS_EXPIRE_AFTER_DAYS: u64 = 60;
const EVENTS_TTL: Duration = Duration::from_secs(EVENTS_EXPIRE_AFTER_DAYS * 24 * 60 * 60);

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[cfg(debug_assertions)]
const SEED: &str = include_str!("test.json");

#[cfg(debug_assertions)]
use serde::Deserialize;

#[cfg(debug_assertions)]
#[derive(Deserialize)]
struct LiveAskQuestion {
    likes: usize,
    text: String,
    hidden: bool,
    answered: bool,
    #[serde(rename = "createTimeUnix")]
    created: usize,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Backend {
    Dynamo(aws_sdk_dynamodb::Client),
    Local(Arc<Mutex<Local>>),
}

impl Backend {
    #[cfg(test)]
    async fn local() -> Self {
        Backend::Local(Arc::new(Mutex::new(Local::default())))
    }

    /// Instantiate a DynamoDB backend.
    ///  
    /// If `USE_DYNAMODB` is set to "local", the `AWS_ENDPOINT_URL` will be set
    /// to "http://localhost:8000", the `AWS_DEFAULT_REGION` will be "us-east-1",
    /// and the [test credentials](https://docs.rs/aws-config/latest/aws_config/struct.ConfigLoader.html#method.test_credentials)
    /// will be used to sign requests.
    ///
    /// This spares setting those environment variables (including `AWS_ACCESS_KEY_ID`
    /// and `AWS_SECRET_ACCESS_KEY`) via the command line or configuration files,
    /// and allows to run the application against a local dynamodb instance with just:
    /// ```sh
    /// USE_DYNAMODB=local cargo run
    /// ```
    /// While the entire test suite can be run with:
    /// ```sh
    /// USE_DYNAMODB=local cargo t -- --include-ignored
    /// ```
    ///
    /// If customization is needed, set `USE_DYNAMODB` to e.g. "custom", and
    /// set the evironment variables to whatever values you need or let them be
    /// picked up from your `~/.aws` files (see [`aws_config::load_from_env`](https://docs.rs/aws-config/latest/aws_config/fn.load_from_env.html))
    async fn dynamo() -> Self {
        let config = if std::env::var("USE_DYNAMODB")
            .ok()
            .is_some_and(|v| v == "local")
        {
            aws_config::from_env()
                .endpoint_url("http://localhost:8000")
                .region("us-east-1")
                .test_credentials()
                .load()
                .await
        } else {
            aws_config::load_from_env().await
        };
        Backend::Dynamo(aws_sdk_dynamodb::Client::new(&config))
    }
}

fn to_dynamo_timestamp(time: SystemTime) -> AttributeValue {
    AttributeValue::N(
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string(),
    )
}

#[derive(Clone, Debug, Default)]
struct Local {
    events: HashMap<Ulid, String>,
    questions: HashMap<Ulid, HashMap<&'static str, AttributeValue>>,
    questions_by_eid: HashMap<Ulid, Vec<Ulid>>,
}

mod ask;
mod event;
mod list;
mod new;
mod questions;
mod toggle;
mod vote;

async fn get_secret(dynamo: &Backend, eid: &Ulid) -> Result<String, StatusCode> {
    match dynamo {
        Backend::Dynamo(dynamo) => {
            match dynamo
                .get_item()
                .table_name("events")
                .key("id", AttributeValue::S(eid.to_string()))
                .projection_expression("secret")
                .send()
                .await
            {
                Ok(v) => {
                    if let Some(s) = v
                        .item()
                        .and_then(|e| e.get("secret"))
                        .and_then(|s| s.as_s().ok())
                    {
                        Ok(s.clone())
                    } else {
                        warn!(%eid, "attempted to access non-existing event");
                        Err(StatusCode::NOT_FOUND)
                    }
                }
                Err(e) => {
                    error!(%eid, error = %e, "dynamodb event request for secret verificaton failed");
                    Err(http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Backend::Local(local) => {
            let mut local = local.lock().unwrap();
            let Local { events, .. } = &mut *local;
            match events.get(eid) {
                Some(s) => Ok(s.clone()),
                None => Err(StatusCode::NOT_FOUND),
            }
        }
    }
}

async fn check_secret(dynamo: &Backend, eid: &Ulid, secret: &str) -> Result<(), StatusCode> {
    let s = get_secret(dynamo, eid).await?;
    if s == secret {
        Ok(())
    } else {
        warn!(%eid, secret, "attempted to access event with incorrect secret");
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn mint_service_error<E>(e: E) -> SdkError<E> {
    SdkError::service_error(
        e,
        aws_smithy_runtime_api::http::Response::new(
            aws_smithy_runtime_api::http::StatusCode::try_from(200).unwrap(),
            SdkBody::empty(),
        ),
    )
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time(/* cloudwatch does that */).init();

    #[cfg(not(debug_assertions))]
    let backend = Backend::dynamo().await;
    #[cfg(debug_assertions)]
    let (backend, qids) = if std::env::var_os("USE_DYNAMODB").is_some() {
        use aws_sdk_dynamodb::types::{PutRequest, WriteRequest};

        let mut backend = Backend::dynamo().await;
        let qids: Vec<Ulid> = if let Backend::Dynamo(ref mut client) = backend {
            info!("going to seed test event");
            let seed: Vec<LiveAskQuestion> = serde_json::from_str(SEED).unwrap();
            let seed_e = Ulid::from_string("00000000000000000000000000").unwrap();
            match client
                .put_item()
                .table_name("events")
                .condition_expression("attribute_not_exists(id)")
                .item("id", AttributeValue::S(seed_e.to_string()))
                .item("secret", AttributeValue::S("secret".into()))
                .item("when", to_dynamo_timestamp(SystemTime::now()))
                .item(
                    "expire",
                    to_dynamo_timestamp(SystemTime::now() + EVENTS_TTL),
                )
                .send()
                .await
            {
                Err(ref error @ SdkError::ServiceError(ref e)) => {
                    if e.err().is_conditional_check_failed_exception() {
                        warn!("test event is already there, skipping seeding questions");
                    } else {
                        panic!("failed to seed test event {:?}", error)
                    }
                }
                Err(e) => panic!("failed to seed test event {:?}", e),
                Ok(_) => {
                    info!("successfully registered test event, going to seed questions now");
                    // DynamoDB supports batch write operations with `25` as max batch size
                    // https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_BatchWriteItem.html
                    for chunk in seed.chunks(25) {
                        client
                            .batch_write_item()
                            .request_items(
                                "questions",
                                chunk
                                    .iter()
                                    .map(
                                        |LiveAskQuestion {
                                             likes,
                                             text,
                                             hidden,
                                             answered,
                                             created,
                                         }| {
                                            let mut item = HashMap::from([
                                                (
                                                    "id".to_string(),
                                                    AttributeValue::S(
                                                        ulid::Ulid::new().to_string(),
                                                    ),
                                                ),
                                                (
                                                    "eid".to_string(),
                                                    AttributeValue::S(seed_e.to_string()),
                                                ),
                                                (
                                                    "votes".to_string(),
                                                    AttributeValue::N(likes.to_string()),
                                                ),
                                                (
                                                    "text".to_string(),
                                                    AttributeValue::S(text.clone()),
                                                ),
                                                (
                                                    "when".to_string(),
                                                    AttributeValue::N(created.to_string()),
                                                ),
                                                (
                                                    "expire".to_string(),
                                                    to_dynamo_timestamp(
                                                        SystemTime::now() + QUESTIONS_TTL,
                                                    ),
                                                ),
                                                (
                                                    "hidden".to_string(),
                                                    AttributeValue::Bool(*hidden),
                                                ),
                                            ]);
                                            if *answered {
                                                item.insert(
                                                    "answered".to_string(),
                                                    to_dynamo_timestamp(SystemTime::now()),
                                                );
                                            }
                                            WriteRequest::builder()
                                                .put_request(
                                                    PutRequest::builder()
                                                        .set_item(Some(item))
                                                        .build()
                                                        .expect("request to have been built ok"),
                                                )
                                                .build()
                                        },
                                    )
                                    .collect::<Vec<_>>(),
                            )
                            .send()
                            .await
                            .expect("batch to have been written ok");
                    }
                    info!("successfully registered questions");
                }
            }
            // let's collect ids of the questions related to the test event,
            // we can then use them to auto-generate user votes over time
            client
                .query()
                .table_name("questions")
                .index_name("top")
                .key_condition_expression("eid = :eid")
                .expression_attribute_values(":eid", AttributeValue::S(seed_e.to_string()))
                .send()
                .await
                .expect("scanned index ok")
                .items()
                .iter()
                .filter_map(|item| {
                    let id = item
                        .get("id")
                        .expect("id is in projection")
                        .as_s()
                        .expect("id is of type string");
                    ulid::Ulid::from_string(id).ok()
                })
                .collect()
        } else {
            unreachable!()
        };
        (backend, qids)
    } else {
        let mut state = Local::default();
        let seed: Vec<LiveAskQuestion> = serde_json::from_str(SEED).unwrap();
        let seed_e = "00000000000000000000000000";
        let seed_e = Ulid::from_string(seed_e).unwrap();
        state.events.insert(seed_e, String::from("secret"));
        state.questions_by_eid.insert(seed_e, Vec::new());
        let mut state = Backend::Local(Arc::new(Mutex::new(state)));
        let mut qs = Vec::new();
        for q in seed {
            let qid = ulid::Ulid::new();
            state
                .ask(
                    &seed_e,
                    &qid,
                    ask::Question {
                        body: q.text,
                        asker: None,
                    },
                )
                .await
                .unwrap();
            qs.push((qid, q.created, q.likes, q.hidden, q.answered));
        }
        let mut qids = Vec::new();
        {
            let Backend::Local(ref mut state): Backend = state else {
                unreachable!();
            };
            let state = Arc::get_mut(state).unwrap();
            let state = Mutex::get_mut(state).unwrap();
            for (qid, created, votes, hidden, answered) in qs {
                let q = state.questions.get_mut(&qid).unwrap();
                q.insert("votes", AttributeValue::N(votes.to_string()));
                if answered {
                    q.insert("answered", to_dynamo_timestamp(SystemTime::now()));
                }
                q.insert("hidden", AttributeValue::Bool(hidden));
                q.insert("when", AttributeValue::N(created.to_string()));
                qids.push(qid);
            }
        }
        (state, qids)
    };

    // to aid in development, auto-generate user votes over time
    #[cfg(debug_assertions)]
    {
        use rand::prelude::SliceRandom;
        let backend = backend.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.tick().await;
            loop {
                interval.tick().await;
                let qid = qids.choose(&mut rand::thread_rng()).unwrap();
                let _ = backend.vote(qid, vote::UpDown::Up).await;
            }
        });
    }

    let app = Router::new()
        .route("/api/event", post(new::new))
        .route("/api/event/:eid", post(ask::ask))
        .route("/api/event/:eid", get(event::event))
        .route("/api/event/:eid/questions", get(list::list))
        .route("/api/event/:eid/questions/:secret", get(list::list_all))
        .route(
            "/api/event/:eid/questions/:secret/:qid/toggle/:property",
            post(toggle::toggle),
        )
        .route("/api/vote/:qid/:updown", post(vote::vote))
        .route("/api/questions/:qids", get(questions::questions))
        .layer(RequestBodyLimitLayer::new(1024))
        .with_state(backend);

    if cfg!(debug_assertions) {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        Ok(axum::serve(listener, app.into_make_service()).await?)
    } else {
        // If we compile in release mode, use the Lambda Runtime
        // To run with AWS Lambda runtime, wrap in our `LambdaLayer`
        let app = tower::ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(LambdaLayer)
            .service(app);

        Ok(lambda_http::run(app).await?)
    }
}

#[derive(Clone, Copy)]
pub struct LambdaLayer;

impl<S> Layer<S> for LambdaLayer {
    type Service = LambdaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LambdaService { inner }
    }
}

pub struct LambdaService<S> {
    inner: S,
}

impl<S> Service<lambda_http::Request> for LambdaService<S>
where
    S: Service<axum::http::Request<axum::body::Body>>,
    S::Response: axum::response::IntoResponse + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    type Response = lambda_http::Response<lambda_http::Body>;
    type Error = lambda_http::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: lambda_http::Request) -> Self::Future {
        let (parts, body) = req.into_parts();
        let body = match body {
            lambda_http::Body::Empty => axum::body::Body::default(),
            lambda_http::Body::Text(t) => t.into(),
            lambda_http::Body::Binary(v) => v.into(),
        };

        let request = axum::http::Request::from_parts(parts, body);

        let fut = self.inner.call(request);
        let fut = async move {
            let resp = fut.await?;
            let (parts, body) = resp.into_response().into_parts();
            let bytes = body.collect().await?.to_bytes();
            let bytes: &[u8] = &bytes;
            let resp: hyper::Response<lambda_http::Body> = match std::str::from_utf8(bytes) {
                Ok(s) => hyper::Response::from_parts(parts, s.into()),
                Err(_) => hyper::Response::from_parts(parts, bytes.into()),
            };
            Ok(resp)
        };

        Box::pin(fut)
    }
}
