use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::GetItemError, model::AttributeValue, output::GetItemOutput, types::SdkError,
};
use axum::extract::{Extension, Path};
use http::StatusCode;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

impl Backend {
    pub(super) async fn question(
        &self,
        qid: &Uuid,
    ) -> Result<GetItemOutput, SdkError<GetItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                dynamo
                    .get_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()))
                    .projection_expression("text")
                    .send()
                    .await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                Ok(GetItemOutput::builder()
                    .set_item(Some(
                        questions[qid]
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.clone()))
                            .collect(),
                    ))
                    .build())
            }
        }
    }
}

pub(super) async fn question(
    Path(qid): Path<Uuid>,
    Extension(dynamo): Extension<Backend>,
) -> Result<String, StatusCode> {
    match dynamo.question(&qid).await {
        Ok(v) => {
            if let Some(text) = v
                .item()
                .and_then(|i| i.get("text"))
                .and_then(|t| t.as_s().ok())
            {
                Ok(text.clone())
            } else {
                warn!(%qid, ?v, "invalid question data");
                Err(http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb question request failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
