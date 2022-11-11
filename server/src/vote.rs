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
                    UpDown::Up => upd.update_expression("SET votes = votes + 1"),
                    UpDown::Down => upd.update_expression("SET votes = votes - 1"),
                };

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
