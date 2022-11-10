use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::PutItemError, model::AttributeValue, output::PutItemOutput, types::SdkError,
};
use axum::extract::Extension;
use axum::response::Json;
use http::StatusCode;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::time::SystemTime;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

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
                    .item(
                        "when",
                        AttributeValue::N(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                .to_string(),
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
}

pub(super) async fn new(
    Extension(dynamo): Extension<Backend>,
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
