use std::collections::HashMap;

use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::BatchGetItemError,
    model::{AttributeValue, KeysAndAttributes},
    output::BatchGetItemOutput,
    types::SdkError,
};
use axum::{
    extract::{Extension, Path},
    Json,
};
use http::StatusCode;
use serde_json::Value;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(super) async fn questions(
        &self,
        qids: &[Uuid],
    ) -> Result<BatchGetItemOutput, SdkError<BatchGetItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let keys = qids
                    .iter()
                    .map(|qid| {
                        HashMap::from_iter([(
                            String::from("id"),
                            AttributeValue::S(qid.to_string()),
                        )])
                    })
                    .collect();
                dynamo
                    .batch_get_item()
                    .request_items(
                        "questions",
                        KeysAndAttributes::builder()
                            .set_keys(Some(keys))
                            .projection_expression("text,when")
                            .build(),
                    )
                    .send()
                    .await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                let unprocessed: Vec<_> = qids
                    .iter()
                    .filter(|qid| !questions.contains_key(qid))
                    .map(|qid| {
                        HashMap::from_iter([(
                            String::from("id"),
                            AttributeValue::S(qid.to_string()),
                        )])
                    })
                    .collect();
                let unprocessed = if unprocessed.is_empty() {
                    None
                } else {
                    Some(HashMap::from_iter([(
                        String::from("questions"),
                        KeysAndAttributes::builder()
                            .set_keys(Some(unprocessed))
                            .projection_expression("text,when")
                            .build(),
                    )]))
                };

                Ok(BatchGetItemOutput::builder()
                    .set_unprocessed_keys(unprocessed)
                    .set_responses(Some(HashMap::from_iter([(
                        String::from("questions"),
                        qids.iter()
                            .filter_map(|qid| {
                                Some(
                                    questions
                                        .get(qid)?
                                        .iter()
                                        .map(|(k, v)| (k.to_string(), v.clone()))
                                        .collect(),
                                )
                            })
                            .collect(),
                    )])))
                    .build())
            }
        }
    }
}

pub(super) async fn questions(
    Path(qids): Path<String>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<Value>, StatusCode> {
    let qids: Vec<_> = match qids.split(',').map(Uuid::parse_str).collect() {
        Ok(v) => v,
        Err(e) => {
            warn!(%qids, error = %e, "got invalid uuid set");
            return Err(http::StatusCode::BAD_REQUEST);
        }
    };
    match dynamo.questions(&qids).await {
        Ok(v) => {
            if v.responses().map_or(true, |r| r.is_empty()) {
                warn!(?qids, "no valid qids");
                return Err(http::StatusCode::NOT_FOUND);
            }
            let r = v.responses().unwrap();
            let t = if let Some(t) = r.get("questions") {
                t
            } else {
                error!(?qids, ?v, "got non-empty non-questions response");
                return Err(http::StatusCode::INTERNAL_SERVER_ERROR);
            };

            // TODO: never-expire cache header
            Ok(Json(
                t.iter()
                    .map(|q| {
                        let qid = q
                            .get("id")
                            .and_then(|v| v.as_s().ok())
                            .and_then(|v| Uuid::parse_str(v).ok());
                        let text = q.get("text").and_then(|v| v.as_s().ok());
                        let when = q
                            .get("when")
                            .and_then(|v| v.as_n().ok())
                            .and_then(|v| v.parse::<usize>().ok());
                        match (qid, text, when) {
                            (Some(qid), Some(text), Some(when)) => Ok((
                                qid.to_string(),
                                serde_json::json!({
                                    "text": text,
                                    "when": when,
                                }),
                            )),
                            _ => {
                                error!(?qids, ?q, "bad data types for id/text/when");
                                Err(StatusCode::INTERNAL_SERVER_ERROR)
                            }
                        }
                    })
                    .collect::<Result<_, _>>()?,
            ))
        }
        Err(e) => {
            error!(?qids, error = %e, "dynamodb question request failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
