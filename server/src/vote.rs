use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::UpdateItemError,
    model::{AttributeValue, ReturnValue},
    output::UpdateItemOutput,
    types::SdkError,
};
use axum::extract::{Extension, Path};
use axum::response::Json;
use http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub(super) enum UpDown {
    Up,
    Down,
}

impl Backend {
    pub(super) async fn vote(
        &self,
        qid: &Uuid,
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
                    UpDown::Down => upd.update_expression("SET votes = votes - :one"),
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
                    let new_n = match direction {
                        UpDown::Up => real_n + 1,
                        UpDown::Down => real_n - 1,
                    };
                    *n = new_n.to_string();
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
    Path((qid, direction)): Path<(Uuid, UpDown)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match dynamo.vote(&qid, direction).await {
        Ok(v) => {
            debug!(%qid, "voted for question");
            let new_count = v
                .attributes()
                .and_then(|a| a.get("votes"))
                .and_then(|v| v.as_n().ok())
                .and_then(|v| v.parse::<isize>().ok());
            Ok(Json(serde_json::json!({ "votes": new_count })))
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
        let e = crate::new::new(Extension(backend.clone())).await.unwrap();
        let eid = Uuid::parse_str(e["id"].as_str().unwrap()).unwrap();
        let _secret = e["secret"].as_str().unwrap();
        let q1 = crate::ask::ask(
            Path(eid.clone()),
            String::from("hello world"),
            Extension(backend.clone()),
        )
        .await
        .unwrap();
        let qid1 = Uuid::parse_str(q1["id"].as_str().unwrap()).unwrap();
        let q2 = crate::ask::ask(
            Path(eid.clone()),
            String::from("hello moon"),
            Extension(backend.clone()),
        )
        .await
        .unwrap();
        let qid2 = Uuid::parse_str(q2["id"].as_str().unwrap()).unwrap();

        let check = |qs: serde_json::Value, expect: &[(&Uuid, u64)]| {
            let qs = qs.as_array().unwrap();
            for (was, should_be) in qs.iter().zip(expect) {
                assert_eq!(was["qid"].as_str().unwrap(), should_be.0.to_string());
                assert_eq!(was["votes"].as_u64().unwrap(), should_be.1);
            }
        };

        super::vote(Path((qid2.clone(), UpDown::Up)), Extension(backend.clone()))
            .await
            .unwrap();
        check(
            crate::list::list(Path(eid.clone()), Extension(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            &[(&qid2, 2), (&qid1, 1)],
        );

        super::vote(Path((qid1.clone(), UpDown::Up)), Extension(backend.clone()))
            .await
            .unwrap();
        super::vote(
            Path((qid2.clone(), UpDown::Down)),
            Extension(backend.clone()),
        )
        .await
        .unwrap();
        check(
            crate::list::list(Path(eid.clone()), Extension(backend.clone()))
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
