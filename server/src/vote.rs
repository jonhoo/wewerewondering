use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::update_item::{UpdateItemError, UpdateItemOutput},
    types::{AttributeValue, ReturnValue},
};
use axum::extract::{Path, State};
use axum::response::Json;
use http::StatusCode;
use serde::Deserialize;
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum UpDown {
    Up,
    Down,
}

impl Backend {
    pub async fn vote(
        &self,
        qid: &Ulid,
        direction: UpDown,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let upd = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let upd = match direction {
                    UpDown::Up => upd.update_expression("SET votes = votes + :one"),
                    UpDown::Down => upd
                        .update_expression("SET votes = votes - :one")
                        .condition_expression("votes > :zero")
                        .expression_attribute_values(":zero", AttributeValue::N(0.to_string())),
                };
                let upd = upd.expression_attribute_values(":one", AttributeValue::N(1.to_string()));

                upd.return_values(ReturnValue::AllNew).send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                let ret = UpdateItemOutput::builder();
                let q = questions
                    .get_mut(qid)
                    .expect("voting for non-existing question");
                if let Some(AttributeValue::N(n)) = q.get_mut("votes") {
                    let real_n = n.parse::<usize>().expect("votes values are numbers");
                    match direction {
                        UpDown::Up => {
                            *n = (real_n + 1).to_string();
                        }
                        UpDown::Down => {
                            if real_n > 0 {
                                *n = (real_n - 1).to_string();
                            }
                        }
                    };
                } else {
                    unreachable!("no votes for question");
                }
                let ret = ret.set_attributes(Some(
                    q.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                ));
                Ok(ret.build())
            }
        }
    }
}

pub(super) async fn vote(
    Path((qid, direction)): Path<(Ulid, UpDown)>,
    State(dynamo): State<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match dynamo.vote(&qid, direction).await {
        Ok(v) => {
            debug!(%qid, "voted for question");
            let new_count = v
                .attributes()
                .and_then(|a| a.get("votes"))
                .and_then(|v| v.as_n().ok())
                .and_then(|v| v.parse::<usize>().ok());
            Ok(Json(serde_json::json!({ "votes": new_count })))
        }
        Err(ref error @ SdkError::ServiceError(ref e)) => {
            if e.err().is_conditional_check_failed_exception() {
                Ok(Json(serde_json::json!({"votes": 0})))
            } else {
                error!(%qid, error = %error, "dynamodb request to vote for question failed");
                Err(http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb request to vote for question failed");
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
        let qid1 = Ulid::from_string(q1["id"].as_str().unwrap()).unwrap();
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
        let qid2 = Ulid::from_string(q2["id"].as_str().unwrap()).unwrap();

        let check = |qs: serde_json::Value, expect: &[(&Ulid, u64)]| {
            let qs = qs.as_array().unwrap();
            for (was, should_be) in qs.iter().zip(expect) {
                assert_eq!(was["qid"].as_str().unwrap(), should_be.0.to_string());
                assert_eq!(was["votes"].as_u64().unwrap(), should_be.1);
            }
        };

        let _ = super::vote(Path((qid2, UpDown::Up)), State(backend.clone()))
            .await
            .unwrap();
        check(
            crate::list::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            &[(&qid2, 2), (&qid1, 1)],
        );

        let _ = super::vote(Path((qid1, UpDown::Up)), State(backend.clone()))
            .await
            .unwrap();
        let _ = super::vote(Path((qid2, UpDown::Down)), State(backend.clone()))
            .await
            .unwrap();
        check(
            crate::list::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            &[(&qid1, 2), (&qid2, 1)],
        );

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
