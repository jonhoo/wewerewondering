use aws_sdk_dynamodb::{
    error::{GetItemError, PutItemError, QueryError, UpdateItemError},
    model::{AttributeValue, ReturnValue},
    output::{GetItemOutput, PutItemOutput, QueryOutput, UpdateItemOutput},
    types::SdkError,
};
use axum::extract::{Extension, Path};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use http::StatusCode;
use lambda_http::Error;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tower::Layer;
use tower_http::{compression::CompressionLayer, limit::RequestBodyLimitLayer};
use tower_service::Service;
use uuid::Uuid;

#[derive(Clone, Debug)]
enum Backend {
    Dynamo(aws_sdk_dynamodb::Client),
    Local(Arc<Mutex<Local>>),
}

#[derive(Clone, Debug, Default)]
struct Local {
    events: HashMap<Uuid, String>,
    questions: HashMap<Uuid, HashMap<&'static str, AttributeValue>>,
    questions_by_eid: HashMap<Uuid, Vec<Uuid>>,
}

impl Backend {
    async fn new(
        &self,
        eid: &Uuid,
        secret: impl Into<String>,
    ) -> Result<PutItemOutput, SdkError<PutItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                dynamo
                    .put_item()
                    .table_name("events")
                    .item("id", AttributeValue::S(eid.to_string()))
                    .item("secret", AttributeValue::S(secret.into()))
                    .send()
                    .await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local {
                    events,
                    questions_by_eid,
                    ..
                } = &mut *local;

                questions_by_eid.insert(eid.clone(), Vec::new());
                if events.insert(eid.clone(), secret.into()).is_some() {
                    Ok(PutItemOutput::builder().build())
                } else {
                    Ok(PutItemOutput::builder().build())
                }
            }
        }
    }

    async fn ask(
        &self,
        eid: &Uuid,
        qid: &Uuid,
        text: impl Into<String>,
    ) -> Result<PutItemOutput, SdkError<PutItemError>> {
        let attrs = [
            ("id", AttributeValue::S(qid.to_string())),
            ("eid", AttributeValue::S(eid.to_string())),
            ("votes", AttributeValue::N(1.to_string())),
            ("text", AttributeValue::S(text.into())),
            ("hidden", AttributeValue::Bool(false)),
            ("answered", AttributeValue::Bool(false)),
        ];
        match self {
            Self::Dynamo(dynamo) => {
                let mut r = dynamo.put_item().table_name("questions");
                for (k, v) in attrs {
                    r = r.item(k, v);
                }
                r.send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local {
                    questions,
                    questions_by_eid,
                    ..
                } = &mut *local;

                questions.insert(qid.clone(), HashMap::from_iter(attrs));
                questions_by_eid
                    .get_mut(eid)
                    .expect("adding question to event that doesn't exist")
                    .push(qid.clone());
                Ok(PutItemOutput::builder().build())
            }
        }
    }

    async fn vote(
        &self,
        qid: &Uuid,
        direction: UpDown,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let upd = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let upd = match direction {
                    UpDown::Up => upd.update_expression("SET votes = votes + 1"),
                    UpDown::Down => upd.update_expression("SET votes = votes - 1"),
                };

                upd.return_values(ReturnValue::AllNew).send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                let ret = UpdateItemOutput::builder();
                let q = questions
                    .get_mut(qid)
                    .expect("voting for non-existing question");
                if let Some(AttributeValue::N(n)) = q.get_mut("votes") {
                    let real_n = n.parse::<usize>().expect("votes values are numbers");
                    let new_n = match direction {
                        UpDown::Up => real_n + 1,
                        UpDown::Down => real_n - 1,
                    };
                    *n = new_n.to_string();
                } else {
                    unreachable!("no votes for question");
                }
                let ret = ret.set_attributes(Some(
                    q.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                ));
                Ok(ret.build())
            }
        }
    }

    async fn list(
        &self,
        eid: &Uuid,
        has_secret: bool,
    ) -> Result<QueryOutput, SdkError<QueryError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let query = dynamo.query();
                let query = query
                    .table_name("questions")
                    .index_name("top")
                    .scan_index_forward(false)
                    .key_condition_expression("eid = :eid")
                    .expression_attribute_names("eid", eid.to_string());

                let query = if has_secret {
                    query
                } else {
                    query.filter_expression("NOT hidden")
                };

                query.send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local {
                    questions,
                    questions_by_eid,
                    ..
                } = &mut *local;

                let qs = questions_by_eid
                    .get_mut(eid)
                    .expect("list for non-existing event");
                qs.sort_unstable_by_key(|qid| {
                    std::cmp::Reverse(
                        questions[qid]["votes"]
                            .as_n()
                            .expect("votes is always set")
                            .parse::<usize>()
                            .expect("votes are always numbers"),
                    )
                });

                Ok(QueryOutput::builder()
                    .set_items(Some(
                        qs.iter()
                            .filter_map(|qid| {
                                let q = &questions[qid];
                                if has_secret {
                                    Some(
                                        q.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                                    )
                                } else if q["hidden"] == AttributeValue::Bool(false) {
                                    Some(
                                        q.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                                    )
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    ))
                    .build())
            }
        }
    }

    async fn question(&self, qid: &Uuid) -> Result<GetItemOutput, SdkError<GetItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                dynamo
                    .get_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()))
                    .projection_expression("text")
                    .send()
                    .await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                Ok(GetItemOutput::builder()
                    .set_item(Some(
                        questions[qid]
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect(),
                    ))
                    .build())
            }
        }
    }

    async fn toggle(
        &self,
        qid: &Uuid,
        property: Property,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let q = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let q = match property {
                    Property::Hidden => q.update_expression("SET hidden = NOT hidden"),
                    Property::Answered => q.update_expression("SET answered = NOT answered"),
                };

                q.send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                fn invert(q: &mut HashMap<&'static str, AttributeValue>, key: &'static str) {
                    if let AttributeValue::Bool(b) = q[key] {
                        q.insert(key, AttributeValue::Bool(!b));
                    } else {
                        unreachable!("all properties are bools");
                    }
                }

                let q = questions
                    .get_mut(qid)
                    .expect("toggle property on unknown question ");
                match property {
                    Property::Hidden => invert(q, "hidden"),
                    Property::Answered => invert(q, "answered"),
                }

                Ok(UpdateItemOutput::builder().build())
            }
        }
    }
}

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

async fn new(Extension(dynamo): Extension<Backend>) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: UUIDv7
    let eid = uuid::Uuid::new_v4();
    let secret: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    match dynamo.new(&eid, &secret).await {
        Ok(_) => {
            debug!(%eid, "created event");
            Ok(Json(
                serde_json::json!({ "id": eid.to_string(), "secret": secret }),
            ))
        }
        Err(e) => {
            error!(%eid, error = %e, "dynamodb request to create event failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn ask(
    Path(eid): Path<Uuid>,
    body: String,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: UUIDv7
    let qid = uuid::Uuid::new_v4();
    match dynamo.ask(&eid, &qid, &body).await {
        Ok(_) => {
            debug!(%eid, %qid, "created question");
            Ok(Json(serde_json::json!({ "id": qid.to_string() })))
        }
        Err(e) => {
            error!(%eid, %qid, error = %e, "dynamodb request to create question failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
enum UpDown {
    Up,
    Down,
}

async fn vote(
    Path((qid, direction)): Path<(Uuid, UpDown)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match dynamo.vote(&qid, direction).await {
        Ok(v) => {
            debug!(%qid, "voted for question");
            let new_count = v
                .attributes()
                .and_then(|a| a.get("votes"))
                .and_then(|v| v.as_n().ok())
                .and_then(|v| v.parse::<isize>().ok());
            Ok(Json(serde_json::json!({ "votes": new_count })))
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb request to vote for question failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn list(
    Path(eid): Path<Uuid>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    list_inner(Path((eid, None)), Extension(dynamo)).await
}

async fn list_all(
    Path((eid, secret)): Path<(Uuid, String)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    list_inner(Path((eid, Some(secret))), Extension(dynamo)).await
}

async fn list_inner(
    Path((eid, secret)): Path<(Uuid, Option<String>)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let has_secret = if let Some(secret) = secret {
        debug!("list questions with admin access");
        check_secret(&dynamo, &eid, &secret).await?;
        true
    } else {
        trace!("list questions with guest access");
        false
    };

    match dynamo.list(&eid, has_secret).await {
        Ok(qs) => {
            trace!(%eid, n = %qs.count(), "listed questions");
            let questions: Vec<_> = qs
                .items()
                .map(|qs| {
                    qs.iter()
                        .filter_map(|doc| {
                            let qid = doc["id"].as_s().ok();
                            let votes = doc["votes"]
                                .as_n()
                                .ok()
                                .and_then(|v| v.parse::<usize>().ok());
                            let hidden = doc["hidden"]
                                .as_bool()
                                .ok();
                            let answered = doc["answered"]
                                .as_bool()
                                .ok();
                            match (qid, votes, hidden, answered) {
                                (Some(qid), Some(votes), Some(hidden), Some(answered)) => Some(serde_json::json!({
                                    "qid": qid,
                                    "votes": votes,
                                    "hidden": hidden,
                                    "answered": answered
                                })),
                                (Some(qid), _, _, _) => {
                                    error!(%eid, %qid, votes = ?doc.get("votes"), "found non-numeric vote count");
                                    None
                                },
                                _ => {
                                    error!(%eid, ?doc, "found non-string question id");
                                    None
                                }
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Json(serde_json::Value::from(questions)))
        }
        Err(e) => {
            error!(%eid, error = %e, "dynamodb request for question list failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
enum Property {
    Hidden,
    Answered,
}

async fn question(
    Path(qid): Path<Uuid>,
    Extension(dynamo): Extension<Backend>,
) -> Result<String, StatusCode> {
    match dynamo.question(&qid).await {
        Ok(v) => {
            if let Some(text) = v
                .item()
                .and_then(|i| i.get("text"))
                .and_then(|t| t.as_s().ok())
            {
                Ok(text.clone())
            } else {
                warn!(%qid, ?v, "invalid question data");
                Err(http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb question request failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn check_secret(dynamo: &Backend, eid: &Uuid, secret: &str) -> Result<(), StatusCode> {
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
                    if v.item()
                        .and_then(|e| e.get("secret"))
                        .and_then(|s| s.as_s().ok())
                        .map_or(false, |s| s == secret)
                    {
                        Ok(())
                    } else {
                        warn!(%eid, secret, "attempted to access event with incorrect secret");
                        Err(StatusCode::FORBIDDEN)
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
            if events[eid] == secret {
                Ok(())
            } else {
                Err(StatusCode::FORBIDDEN)
            }
        }
    }
}

async fn toggle(
    Path((eid, secret, qid, property)): Path<(Uuid, String, Uuid, Property)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<(), StatusCode> {
    check_secret(&dynamo, &eid, &secret).await?;

    match dynamo.toggle(&qid, property).await {
        Ok(_) => {
            debug!(%eid, %qid, p = ?property, "toggled question property");
            Ok(())
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb request to vote for question failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        // .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    let config = aws_config::load_from_env().await;

    let backend = if cfg!(debug_assertions) {
        Backend::Local(Default::default())
    } else {
        Backend::Dynamo(aws_sdk_dynamodb::Client::new(&config))
    };

    let app = Router::new()
        .route("/event", post(new))
        .route("/event/:eid", get(list))
        .route("/event/:eid/:secret", get(list_all))
        .route("/event/:eid/:secret/:qid/toggle/:property", post(toggle))
        .route("/event/:eid", post(ask))
        .route("/vote/:qid/:updown", post(vote))
        .route("/question/:qid", get(question))
        .layer(Extension(backend))
        .layer(CompressionLayer::new().gzip(true).deflate(true))
        .layer(RequestBodyLimitLayer::new(512));

    if cfg!(debug_assertions) {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
        Ok(axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?)
    } else {
        // If we compile in release mode, use the Lambda Runtime
        // To run with AWS Lambda runtime, wrap in our `LambdaLayer`
        let app = tower::ServiceBuilder::new()
            .layer(LambdaLayer::default())
            .service(app);

        Ok(lambda_http::run(app).await?)
    }
}

#[derive(Default, Clone, Copy)]
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
            let bytes = hyper::body::to_bytes(body).await?;
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
