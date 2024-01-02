use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::batch_get_item::{BatchGetItemError, BatchGetItemOutput},
    types::{AttributeValue, KeysAndAttributes},
};
use axum::{
    extract::{Path, State},
    response::AppendHeaders,
    Json,
};
use http::{
    header::{self, HeaderName},
    StatusCode,
};
use serde_json::Value;
use std::collections::HashMap;
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(super) async fn questions(
        &self,
        qids: &[Ulid],
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
                            .projection_expression("id,#text,#when,who")
                            .expression_attribute_names("#text", "text")
                            .expression_attribute_names("#when", "when")
                            .build()
                            .expect("we're building correct things"),
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
                            .projection_expression("text,when,who")
                            .build()
                            .expect("we build correctly"),
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
                                        .filter(|&(k, _)| {
                                            matches!(*k, "id" | "text" | "when" | "who")
                                        })
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
    State(dynamo): State<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<Value>, StatusCode>,
) {
    let qids: Vec<_> = match qids.split(',').map(Ulid::from_string).collect() {
        Ok(v) => v,
        Err(e) => {
            warn!(%qids, error = %e, "got invalid ulid set");
            return (
                // a bad request will never become good
                AppendHeaders([(header::CACHE_CONTROL, "max-age=864001")]),
                Err(http::StatusCode::BAD_REQUEST),
            );
        }
    };
    match dynamo.questions(&qids).await {
        Ok(v) => {
            if v.responses().map_or(true, |r| r.is_empty()) {
                warn!(?qids, "no valid qids");
                return (
                    // it should be unlikely that someone fetches a question that hasn't been asked
                    // it's _possible_ that it happens and _then_ a question is assigned that ulid,
                    // but it too seems rare.
                    AppendHeaders([(header::CACHE_CONTROL, "max-age=600")]),
                    Err(http::StatusCode::NOT_FOUND),
                );
            }
            let r = v.responses().unwrap();
            let t = if let Some(t) = r.get("questions") {
                t
            } else {
                error!(?qids, ?v, "got non-empty non-questions response");
                return (
                    AppendHeaders([(header::CACHE_CONTROL, "no-cache")]),
                    Err(http::StatusCode::INTERNAL_SERVER_ERROR),
                );
            };

            let r = t
                .iter()
                .map(|q| {
                    let qid = q
                        .get("id")
                        .and_then(|v| v.as_s().ok())
                        .and_then(|v| ulid::Ulid::from_string(v).ok());
                    let text = q.get("text").and_then(|v| v.as_s().ok());
                    let who = q.get("who").and_then(|v| v.as_s().ok());
                    let when = q
                        .get("when")
                        .and_then(|v| v.as_n().ok())
                        .and_then(|v| v.parse::<usize>().ok());
                    match (qid, text, when) {
                        (Some(qid), Some(text), Some(when)) => {
                            let mut v = serde_json::json!({
                                "text": text,
                                "when": when,
                            });
                            if let Some(who) = who {
                                v["who"] = who.clone().into();
                            }
                            Ok((qid.to_string(), v))
                        }
                        _ => {
                            error!(?qids, ?q, "bad data types for id/text/when");
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                })
                .collect::<Result<_, _>>()
                .map(Json);
            if r.is_ok() {
                (
                    AppendHeaders([(header::CACHE_CONTROL, "max-age=864001")]),
                    r,
                )
            } else {
                (AppendHeaders([(header::CACHE_CONTROL, "no-cache")]), r)
            }
        }
        Err(e) => {
            error!(?qids, error = %e, "dynamodb question request failed");
            (
                AppendHeaders([(header::CACHE_CONTROL, "no-cache")]),
                Err(http::StatusCode::INTERNAL_SERVER_ERROR),
            )
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
        let q1 = crate::ask::ask(
            Path(eid),
            State(backend.clone()),
            Json(crate::ask::Question {
                body: "hello world".into(),
                asker: None,
            }),
        )
        .await
        .unwrap();
        let qid1 = q1["id"].as_str().unwrap();
        let q2 = crate::ask::ask(
            Path(eid),
            State(backend.clone()),
            Json(crate::ask::Question {
                body: "hello moon".into(),
                asker: Some("person".into()),
            }),
        )
        .await
        .unwrap();
        let qid2 = q2["id"].as_str().unwrap();

        let qids = super::questions(Path(format!("{qid1},{qid2}")), State(backend.clone()))
            .await
            .1
            .unwrap();

        let qids = qids.as_object().unwrap();
        let q1 = &qids[qid1];
        assert_eq!(q1["text"], "hello world");
        assert_eq!(q1["who"], serde_json::Value::Null);
        assert!(q1["when"].is_u64());
        let q2 = &qids[qid2];
        assert_eq!(q2["text"], "hello moon");
        assert_eq!(q2["who"], "person");
        assert!(q2["when"].is_u64());

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
