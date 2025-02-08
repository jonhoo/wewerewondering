use crate::{to_dynamo_timestamp, EVENTS_TTL};

use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::put_item::{PutItemError, PutItemOutput},
    types::AttributeValue,
};
use axum::extract::State;
use axum::response::Json;
use http::StatusCode;
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use std::time::SystemTime;
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    #[allow(clippy::wrong_self_convention)]
    #[allow(clippy::new_ret_no_self)]
    pub(super) async fn new(
        &self,
        eid: &Ulid,
        secret: impl Into<String>,
    ) -> Result<PutItemOutput, SdkError<PutItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                dynamo
                    .put_item()
                    .table_name("events")
                    .item("id", AttributeValue::S(eid.to_string()))
                    .item("secret", AttributeValue::S(secret.into()))
                    .item("when", to_dynamo_timestamp(SystemTime::now()))
                    .item(
                        "expire",
                        to_dynamo_timestamp(SystemTime::now() + EVENTS_TTL),
                    )
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

                questions_by_eid.insert(*eid, Vec::new());
                let _ = events.insert(*eid, secret.into()).is_some();
                Ok(PutItemOutput::builder().build())
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn delete(&self, eid: &Ulid) {
        let qs = self.list(eid, false).await.unwrap();
        let qids: Vec<_> = qs
            .items()
            .iter()
            .filter_map(|doc| doc["id"].as_s().ok())
            .cloned()
            .collect();

        match self {
            Self::Dynamo(dynamo) => {
                for qid in qids {
                    dynamo
                        .delete_item()
                        .table_name("questions")
                        .key("id", AttributeValue::S(qid))
                        .send()
                        .await
                        .unwrap();
                }
                dynamo
                    .delete_item()
                    .table_name("events")
                    .key("id", AttributeValue::S(eid.to_string()))
                    .send()
                    .await
                    .unwrap();
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local {
                    events,
                    questions,
                    questions_by_eid,
                    ..
                } = &mut *local;

                for qid in questions_by_eid.remove(eid).unwrap() {
                    questions.remove(&qid).unwrap();
                }
                events.remove(eid).unwrap();
            }
        }
    }
}

pub(super) async fn new(
    State(dynamo): State<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let eid = ulid::Ulid::new();
    let secret: String = rng()
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
            eprintln!("{e:?}");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn inner(backend: Backend) {
        let e = crate::new::new(State(backend.clone())).await.unwrap();
        let eid = Ulid::from_string(e["id"].as_str().unwrap()).unwrap();
        let _secret = e["secret"].as_str().unwrap();
        backend.delete(&eid).await;
    }

    #[tokio::test]
    async fn local() {
        inner(Backend::local().await).await;
    }

    #[tokio::test]
    #[ignore]
    async fn dynamodb() {
        inner(Backend::dynamo().await).await;
    }
}
