use super::{Backend, Local};
use crate::{utils, QUESTIONS_TTL};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::put_item::{PutItemError, PutItemOutput},
    types::AttributeValue,
};
use axum::extract::{Path, State};
use axum::response::Json;
use http::StatusCode;
use serde::Deserialize;
use std::{collections::HashMap, time::SystemTime};
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(crate) async fn ask(
        &self,
        eid: &Ulid,
        qid: &Ulid,
        q: Question,
    ) -> Result<PutItemOutput, SdkError<PutItemError>> {
        let attrs = [
            ("id", AttributeValue::S(qid.to_string())),
            ("eid", AttributeValue::S(eid.to_string())),
            ("votes", AttributeValue::N(1.to_string())),
            ("text", AttributeValue::S(q.body)),
            ("when", utils::to_dynamo_timestamp(SystemTime::now())),
            (
                "expire",
                utils::to_dynamo_timestamp(SystemTime::now() + QUESTIONS_TTL),
            ),
            ("hidden", AttributeValue::Bool(false)),
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
                questions.insert(*qid, question);
                questions_by_eid
                    .get_mut(eid)
                    .expect("adding question to event that doesn't exist")
                    .push(*qid);
                Ok(PutItemOutput::builder().build())
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct Question {
    pub body: String,
    pub asker: Option<String>,
}

pub async fn ask(
    Path(eid): Path<Ulid>,
    State(dynamo): State<Backend>,
    q: Json<Question>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if q.body.trim().is_empty() {
        warn!(%eid, "ignoring empty question");
        return Err(http::StatusCode::BAD_REQUEST);
    } else if !q.body.trim().contains(' ') {
        warn!(%eid, body = q.body, "rejecting single-word question");
        return Err(http::StatusCode::BAD_REQUEST);
    }

    // TODO: check that eid actually exists
    let qid = ulid::Ulid::new();
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
        let e = crate::new::new(State(backend.clone())).await.unwrap();
        let eid = Ulid::from_string(e["id"].as_str().unwrap()).unwrap();
        let _secret = e["secret"].as_str().unwrap();
        let q = super::ask(
            Path(eid),
            State(backend.clone()),
            Json(Question {
                body: "hello world".into(),
                asker: Some("person".into()),
            }),
        )
        .await
        .unwrap();
        let _qid = Ulid::from_string(q["id"].as_str().unwrap()).unwrap();
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
