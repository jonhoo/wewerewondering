use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::{QueryError, QueryErrorKind, ResourceNotFoundException},
    model::AttributeValue,
    output::QueryOutput,
    types::SdkError,
};
use aws_smithy_types::Error;
use axum::extract::{Extension, Path};
use axum::response::Json;
use http::StatusCode;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(super) async fn list(
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
                    events,
                    ..
                } = &mut *local;

                if !events.contains_key(&eid) {
                    return Err(super::mint_service_error(QueryError::new(
                        QueryErrorKind::ResourceNotFoundException(
                            ResourceNotFoundException::builder().build(),
                        ),
                        Error::builder().build(),
                    )));
                }

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
}

pub(super) async fn list(
    Path(eid): Path<Uuid>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    list_inner(Path((eid, None)), Extension(dynamo)).await
}

pub(super) async fn list_all(
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
        super::check_secret(&dynamo, &eid, &secret).await?;
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

            // TODO: cache header (no-cache w/ secret)
            Ok(Json(serde_json::Value::from(questions)))
        }
        Err(e) => {
            if let SdkError::ServiceError { ref err, .. } = e {
                if err.is_resource_not_found_exception() {
                    warn!(%eid, error = %e, "request for non-existing event");
                    return Err(http::StatusCode::NOT_FOUND);
                }
            }
            error!(%eid, error = %e, "dynamodb request for question list failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
