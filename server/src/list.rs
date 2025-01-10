use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::query::{QueryError, QueryOutput},
    types::{error::ResourceNotFoundException, AttributeValue},
};
use axum::response::Json;
use axum::{
    extract::{Path, State},
    response::AppendHeaders,
};
use http::{
    header::{self, HeaderName},
    StatusCode,
};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

const TOP_N: usize = 20; // TODO: how to make it configurable ?

impl Backend {
    pub(super) async fn list(
        &self,
        eid: &Ulid,
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

                if !events.contains_key(eid) {
                    return Err(super::mint_service_error(
                        QueryError::ResourceNotFoundException(
                            ResourceNotFoundException::builder().build(),
                        ),
                    ));
                }

                let qs = questions_by_eid
                    .get_mut(eid)
                    .expect("list for non-existing event");

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
    Path(eid): Path<Ulid>,
    State(dynamo): State<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    list_inner(Path((eid, None)), State(dynamo)).await
}

pub(super) async fn list_all(
    Path((eid, secret)): Path<(Ulid, String)>,
    State(dynamo): State<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    list_inner(Path((eid, Some(secret))), State(dynamo)).await
}

async fn list_inner(
    Path((eid, secret)): Path<(Ulid, Option<String>)>,
    State(dynamo): State<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<serde_json::Value>, StatusCode>,
) {
    let has_secret = if let Some(secret) = secret {
        debug!("list questions with admin access");
        if let Err(e) = super::check_secret(&dynamo, &eid, &secret).await {
            // a bad secret will not turn good and
            // events are unlikely to re-appear with the same Ulid
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
            // events are unlikely to re-appear with the same Ulid
            return (
                AppendHeaders([(header::CACHE_CONTROL, "max-age=86400")]),
                Err(e),
            );
        }
        false
    };

    // Closure moved out of the filter_map due to rustfmt failing to format the
    // code properly.
    let serialize_question = |doc: &HashMap<String, AttributeValue>| {
        let qid = doc["id"].as_s().ok();
        let votes = doc["votes"]
            .as_n()
            .ok()
            .and_then(|v| v.parse::<usize>().ok());
        let hidden = doc["hidden"].as_bool().ok();
        let answered = doc
            .get("answered")
            .and_then(|v| v.as_n().ok())
            .and_then(|v| v.parse::<usize>().ok());
        match (qid, votes, hidden, answered) {
            (Some(qid), Some(votes), Some(hidden), answered) => {
                let mut v = serde_json::json!({
                    "qid": qid,
                    "votes": votes,
                    "hidden": hidden,
                });
                if let Some(answered) = answered {
                    v["answered"] = answered.into();
                }
                Some(v)
            }
            (Some(qid), _, _, _) => {
                error!(%eid, %qid, votes = ?doc.get("votes"), "found non-numeric vote count");
                None
            }
            _ => {
                error!(%eid, ?doc, "found non-string question id");
                None
            }
        }
    };

    match dynamo.list(&eid, has_secret).await {
        Ok(qs) => {
            trace!(%eid, n = %qs.count(), "listed questions");
            let questions: Vec<_> = qs.items().iter().filter_map(serialize_question).collect();

            #[derive(Debug, Default)]
            struct JQuestion(serde_json::Value);
            impl PartialEq for JQuestion {
                fn eq(&self, other: &Self) -> bool {
                    self.cmp(other).is_eq()
                }
            }
            impl Eq for JQuestion {}
            impl PartialOrd for JQuestion {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }
            impl Ord for JQuestion {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    // make answered and hidden always less than unanswered.
                    let answered = self.0.get("answered");
                    let hidden = self
                        .0
                        .get("hidden")
                        .unwrap_or(&serde_json::Value::Bool(false))
                        .as_bool()
                        .unwrap();
                    let answered_other = other.0.get("answered");
                    let hidden_other = other
                        .0
                        .get("hidden")
                        .unwrap_or(&serde_json::Value::Bool(false))
                        .as_bool()
                        .unwrap();
                    match (answered, answered_other, hidden, hidden_other) {
                        (Some(_), None, _, false) => return std::cmp::Ordering::Less,
                        (None, Some(_), false, _) => return std::cmp::Ordering::Greater,
                        (None, None, false, true) => return std::cmp::Ordering::Greater,
                        (None, None, true, false) => return std::cmp::Ordering::Less,
                        _ => {}
                    }

                    let votes = self.0["votes"].as_u64().expect("votes is a number") as f64;
                    let other_votes = other.0["votes"].as_u64().expect("votes is a number") as f64;
                    votes.total_cmp(&other_votes)
                }
            }

            // sort based on "hotness" of the question over time:
            // https://www.evanmiller.org/ranking-news-items-with-upvotes.html
            // the wrapper struct is needed because f64 doesn't impl Ord
            #[derive(Debug)]
            #[repr(transparent)]
            struct Score(f64);
            impl PartialEq for Score {
                fn eq(&self, other: &Self) -> bool {
                    self.cmp(other).is_eq()
                }
            }
            impl Eq for Score {}
            impl PartialOrd for Score {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }
            impl Ord for Score {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.0.total_cmp(&other.0)
                }
            }
            let now = SystemTime::now();
            let score = |q: &serde_json::Value| {
                let dt_in_minutes_rounded_down = now
                    .duration_since(
                        q["qid"]
                            .as_str()
                            .expect("it's a ULID")
                            .parse::<Ulid>()
                            .expect("produced as ULID by us")
                            .datetime(),
                    )
                    .unwrap_or(Duration::ZERO)
                    .as_secs()
                    // in minutes so questions don't jump around quite as much
                    / 60;
                // +1 so that first minute questions don't get inf scores (for the ln)
                let dt = dt_in_minutes_rounded_down + 1;
                // +1 again to avoid NaN scores for first-minute questions (for / (1 - e^0)).
                let dt = dt + 1;
                // ln so that stories get less penalized for age over time
                // after all, this is Q&A, not minute-to-minute hot news
                let dt = (dt as f64).ln();
                let votes = q["votes"].as_u64().expect("votes is a number") as f64;
                // max so that even if vote count somehow got to 0, count it as 1
                let votes = votes.max(1.);
                let exp = (-1. * dt as f64).exp_m1() + 1.;
                Score(exp * votes / (1. - exp))
            };

            let mut questions: Vec<JQuestion> = questions.into_iter().map(JQuestion).collect();
            top_n_sort(&mut questions, TOP_N);
            let mut questions: Vec<serde_json::Value> =
                questions.into_iter().map(|e| e.0).collect();
            questions[TOP_N..].sort_by_cached_key(|q| std::cmp::Reverse(score(q)));
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
            if let SdkError::ServiceError(ref err) = e {
                if err.err().is_resource_not_found_exception() {
                    warn!(%eid, error = %e, "request for non-existing event");
                    return (
                        // it's relatively unlikely that an event Ulid that didn't exist will start
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

fn top_n_sort<T: std::cmp::Ord + std::cmp::Eq + Default>(vec: &mut Vec<T>, top: usize) {
    for i in 1..vec.len() {
        let high = top.min(i);
        if vec[high - 1] > vec[i] {
            continue;
        }
        let key = vec.remove(i);
        let mut pos = vec[..high]
            .binary_search_by(|e| key.cmp(e))
            .unwrap_or_else(|pos| pos);
        if pos == high {
            pos -= 1;
        }
        vec.insert(pos, key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn inner(backend: Backend) {
        let e = crate::new::new(State(backend.clone())).await.unwrap();
        let eid = Ulid::from_string(e["id"].as_str().unwrap()).unwrap();
        let secret = e["secret"].as_str().unwrap();
        let q = crate::ask::ask(
            Path(eid),
            State(backend.clone()),
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
            assert_eq!(q.get("answered"), None);
            assert_eq!(q["hidden"], false);
            assert_eq!(qids.len(), 1, "extra questions in response: {qids:?}");
        };

        check(
            super::list_all(Path((eid, secret.to_string())), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
        );
        check(
            super::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
        );

        // lookup with wrong secret gives 401
        assert_eq!(
            super::list_all(Path((eid, "wrong".to_string())), State(backend.clone()),)
                .await
                .1
                .unwrap_err(),
            StatusCode::UNAUTHORIZED
        );

        // lookup for non-existing event with secret gives 404
        assert_eq!(
            super::list_all(
                Path((
                    Ulid::from_string("00000000000000000000000001").unwrap(),
                    secret.to_string()
                )),
                State(backend.clone()),
            )
            .await
            .1
            .unwrap_err(),
            StatusCode::NOT_FOUND
        );

        backend.delete(&eid).await;

        // lookup for empty but existing event gives 200
        let e = crate::new::new(State(backend.clone())).await.unwrap();
        let eid = Ulid::from_string(e["id"].as_str().unwrap()).unwrap();
        let _ = super::list(Path(eid), State(backend.clone()))
            .await
            .1
            .unwrap();
        backend.delete(&eid).await;

        // lookup for non-existing event without secret gives 404
        assert_eq!(
            super::list(
                Path(Ulid::from_string("00000000000000000000000001").unwrap()),
                State(backend.clone()),
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

    #[test]
    fn test_top_n_sort() {
        let mut vec = vec![4, 5, 9, 8, 1, 3];
        top_n_sort(&mut vec, 2);
        assert_eq!(vec, vec![9, 8, 5, 4, 1, 3]);

        let mut vec = vec![9, 8, 1, 3];
        top_n_sort(&mut vec, 2);
        assert_eq!(vec, vec![9, 8, 1, 3]);

        let mut vec = vec![9];
        top_n_sort(&mut vec, 2);
        assert_eq!(vec, vec![9]);

        let mut vec = vec![4, 5, 9, 8, 1, 3];
        top_n_sort(&mut vec, 10);
        assert_eq!(vec, vec![9, 8, 5, 4, 3, 1]);

        let mut vec = vec![4, 5, 9, 8];
        top_n_sort(&mut vec, 4);
        assert_eq!(vec, vec![9, 8, 5, 4]);
    }
}
