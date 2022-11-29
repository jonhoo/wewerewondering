use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::{QueryError, QueryErrorKind, ResourceNotFoundException},
    model::AttributeValue,
    output::QueryOutput,
    types::SdkError,
};
use aws_smithy_types::Error;
use axum::response::Json;
use axum::{
    extract::{Extension, Path},
    response::AppendHeaders,
};
use http::{
    header::{self, HeaderName},
    StatusCode,
};
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
                    .expression_attribute_values(":eid", AttributeValue::S(eid.to_string()));

                let query = if has_secret {
                    query
                } else {
                    query
                        .filter_expression("#hidden = :false")
                        .expression_attribute_names("#hidden", "hidden".to_string())
                        .expression_attribute_values(":false", AttributeValue::Bool(false))
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
                    .set_count(Some(qs.len() as i32))
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
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    list_inner(Path((eid, None)), Extension(dynamo)).await
}

pub(super) async fn list_all(
    Path((eid, secret)): Path<(Uuid, String)>,
    Extension(dynamo): Extension<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    list_inner(Path((eid, Some(secret))), Extension(dynamo)).await
}

async fn list_inner(
    Path((eid, secret)): Path<(Uuid, Option<String>)>,
    Extension(dynamo): Extension<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    let has_secret = if let Some(secret) = secret {
        debug!("list questions with admin access");
        if let Err(e) = super::check_secret(&dynamo, &eid, &secret).await {
            // a bad secret will not turn good and
            // events are unlikely to re-appear with the same uuid
            return (
                AppendHeaders([(header::CACHE_CONTROL, "max-age=86400")]),
                Err(e),
            );
        }
        true
    } else {
        trace!("list questions with guest access");
        // ensure that the event exists:
        // this is _just_ so give 404s for old events so clients stop polling
        if let Err(e) = super::get_secret(&dynamo, &eid).await {
            // events are unlikely to re-appear with the same uuid
            return (
                AppendHeaders([(header::CACHE_CONTROL, "max-age=86400")]),
                Err(e),
            );
        }
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

            let max_age = if has_secret {
                // hosts should be allowed to see more up-to-date views
                "max-age=3"
            } else {
                // guests don't need super up-to-date, so cache for longer
                "max-age=10"
            };
            (
                AppendHeaders([(header::CACHE_CONTROL, max_age)]),
                Ok(Json(serde_json::Value::from(questions))),
            )
        }
        Err(e) => {
            if let SdkError::ServiceError { ref err, .. } = e {
                if err.is_resource_not_found_exception() {
                    warn!(%eid, error = %e, "request for non-existing event");
                    return (
                        // it's relatively unlikely that an event uuid that didn't exist will start
                        // existing. but just in case, don't make it _too_ long.
                        AppendHeaders([(header::CACHE_CONTROL, "max-age=3600")]),
                        Err(http::StatusCode::NOT_FOUND),
                    );
                }
            }
            error!(%eid, error = %e, "dynamodb request for question list failed");
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
        let e = crate::new::new(Extension(backend.clone())).await.unwrap();
        let eid = Uuid::parse_str(e["id"].as_str().unwrap()).unwrap();
        let secret = e["secret"].as_str().unwrap();
        let q = crate::ask::ask(
            Path(eid.clone()),
            Extension(backend.clone()),
            Json(crate::ask::Question {
                body: "hello world".into(),
                asker: None,
            }),
        )
        .await
        .unwrap();
        let qid = q["id"].as_str().unwrap();

        let check = |qids: serde_json::Value| {
            let qids = qids.as_array().unwrap();
            let q = qids.iter().find(|q| q["qid"] == qid);
            assert!(
                q.is_some(),
                "newly created question {qid} was not listed in {qids:?}"
            );
            let q = q.unwrap();
            assert_eq!(q["votes"], 1);
            assert_eq!(q["answered"], false);
            assert_eq!(q["hidden"], false);
            assert_eq!(qids.len(), 1, "extra questions in response: {qids:?}");
        };

        check(
            super::list_all(
                Path((eid.clone(), secret.to_string())),
                Extension(backend.clone()),
            )
            .await
            .1
            .unwrap()
            .0,
        );
        check(
            super::list(Path(eid.clone()), Extension(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
        );

        // lookup with wrong secret gives 401
        assert_eq!(
            super::list_all(
                Path((eid.clone(), "wrong".to_string())),
                Extension(backend.clone()),
            )
            .await
            .1
            .unwrap_err(),
            StatusCode::UNAUTHORIZED
        );

        // lookup for non-existing event with secret gives 404
        assert_eq!(
            super::list_all(
                Path((
                    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
                    secret.to_string()
                )),
                Extension(backend.clone()),
            )
            .await
            .1
            .unwrap_err(),
            StatusCode::NOT_FOUND
        );

        backend.delete(&eid).await;

        // lookup for empty but existing event gives 200
        let e = crate::new::new(Extension(backend.clone())).await.unwrap();
        let eid = Uuid::parse_str(e["id"].as_str().unwrap()).unwrap();
        super::list(Path(eid), Extension(backend.clone()))
            .await
            .1
            .unwrap();
        backend.delete(&eid).await;

        // lookup for non-existing event without secret gives 404
        assert_eq!(
            super::list(
                Path(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
                Extension(backend.clone()),
            )
            .await
            .1
            .unwrap_err(),
            StatusCode::NOT_FOUND
        );
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
