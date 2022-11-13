use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::PutItemError, model::AttributeValue, output::PutItemOutput, types::SdkError,
};
use axum::extract::{Extension, Path};
use axum::response::Json;
use http::StatusCode;
use serde::Deserialize;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

const QUESTIONS_EXPIRE_AFTER_DAYS: u64 = 30;

impl Backend {
    pub(super) async fn ask(
        &self,
        eid: &Uuid,
        qid: &Uuid,
        q: Question,
    ) -> Result<PutItemOutput, SdkError<PutItemError>> {
        let attrs = [
            ("id", AttributeValue::S(qid.to_string())),
            ("eid", AttributeValue::S(eid.to_string())),
            ("votes", AttributeValue::N(1.to_string())),
            ("text", AttributeValue::S(q.body.into())),
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
            (
                "expire",
                AttributeValue::N(
                    (SystemTime::now()
                        + Duration::from_secs(QUESTIONS_EXPIRE_AFTER_DAYS * 24 * 60 * 60))
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
                if let Some(asker) = q.asker {
                    r = r.item("who", AttributeValue::S(asker));
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

                let mut question = HashMap::from_iter(attrs);
                if let Some(asker) = q.asker {
                    question.insert("who", AttributeValue::S(asker));
                }
                questions.insert(qid.clone(), question);
                questions_by_eid
                    .get_mut(eid)
                    .expect("adding question to event that doesn't exist")
                    .push(qid.clone());
                Ok(PutItemOutput::builder().build())
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub(super) struct Question {
    pub(super) body: String,
    pub(super) asker: Option<String>,
}

pub(super) async fn ask(
    Path(eid): Path<Uuid>,
    q: Json<Question>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if q.body.trim().is_empty() {
        warn!(%eid, "ignoring empty question");
        return Err(http::StatusCode::BAD_REQUEST);
    } else if !q.body.trim().contains(' ') {
        warn!(%eid, body = q.body, "rejecting single-word question");
        return Err(http::StatusCode::BAD_REQUEST);
    }

    // TODO: check that eid actually exists
    // TODO: UUIDv7
    let qid = uuid::Uuid::new_v4();
    match dynamo.ask(&eid, &qid, q.0).await {
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
        let e = crate::new::new(Extension(backend.clone())).await.unwrap();
        let eid = Uuid::parse_str(e["id"].as_str().unwrap()).unwrap();
        let _secret = e["secret"].as_str().unwrap();
        let q = super::ask(
            Path(eid.clone()),
            Json(Question {
                body: "hello world".into(),
                asker: Some("person".into()),
            }),
            Extension(backend.clone()),
        )
        .await
        .unwrap();
        let _qid = Uuid::parse_str(q["id"].as_str().unwrap()).unwrap();
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
