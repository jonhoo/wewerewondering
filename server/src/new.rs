use crate::to_dynamo_timestamp;

use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::PutItemError, model::AttributeValue, output::PutItemOutput, types::SdkError,
};
use axum::extract::State;
use axum::response::Json;
use http::StatusCode;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

const EVENTS_EXPIRE_AFTER_DAYS: u64 = 60;

impl Backend {
    pub(super) async fn new(
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
                    .item("when", to_dynamo_timestamp(SystemTime::now()))
                    .item(
                        "expire",
                        to_dynamo_timestamp(
                            SystemTime::now()
                                + Duration::from_secs(EVENTS_EXPIRE_AFTER_DAYS * 24 * 60 * 60),
                        ),
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

                questions_by_eid.insert(eid.clone(), Vec::new());
                if events.insert(eid.clone(), secret.into()).is_some() {
                    Ok(PutItemOutput::builder().build())
                } else {
                    Ok(PutItemOutput::builder().build())
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn delete(&self, eid: &Uuid) {
        let qs = self.list(eid, false).await.unwrap();
        let qids: Vec<_> = qs
            .items()
            .into_iter()
            .flat_map(|qs| qs.iter().filter_map(|doc| doc["id"].as_s().ok()))
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

                for qid in questions_by_eid.remove(&eid).unwrap() {
                    questions.remove(&qid).unwrap();
                }
                events.remove(&eid).unwrap();
            }
        }
    }
}

pub(super) async fn new(
    State(dynamo): State<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn inner(backend: Backend) {
        let e = crate::new::new(State(backend.clone())).await.unwrap();
        let eid = Uuid::parse_str(e["id"].as_str().unwrap()).unwrap();
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
