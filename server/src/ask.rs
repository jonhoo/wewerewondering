use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::PutItemError, model::AttributeValue, output::PutItemOutput, types::SdkError,
};
use axum::extract::{Extension, Path};
use axum::response::Json;
use http::StatusCode;
use std::{collections::HashMap, time::SystemTime};
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(super) async fn ask(
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
            (
                "when",
                AttributeValue::N(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .to_string(),
                ),
            ),
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
}

pub(super) async fn ask(
    Path(eid): Path<Uuid>,
    body: String,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if body.trim().is_empty() {
        warn!(%eid, "ignoring empty question");
        return Err(http::StatusCode::BAD_REQUEST);
    } else if !body.trim().contains(' ') {
        warn!(%eid, body, "rejecting single-word question");
        return Err(http::StatusCode::BAD_REQUEST);
    }

    // TODO: check that eid actually exists
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn inner(backend: Backend) {
        let eid = Uuid::new_v4();
        let secret = "cargo-test";
        let _ = backend.new(&eid, secret).await.unwrap();
        let qid = Uuid::new_v4();
        backend.ask(&eid, &qid, "hello world").await.unwrap();
        // the list test checks that it's actually returned
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
