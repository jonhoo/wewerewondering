use std::collections::HashMap;

use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::GetItemError, model::AttributeValue, output::GetItemOutput, types::SdkError,
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
    pub(super) async fn event(&self, eid: &Uuid) -> Result<GetItemOutput, SdkError<GetItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                dynamo
                    .get_item()
                    .table_name("events")
                    .key("id", AttributeValue::S(eid.to_string()))
                    .projection_expression("id")
                    .send()
                    .await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { events, .. } = &mut *local;

                Ok(GetItemOutput::builder()
                    .set_item(events.get(eid).map(|_| {
                        HashMap::from_iter([(
                            String::from("id"),
                            AttributeValue::S(eid.to_string()),
                        )])
                    }))
                    .build())
            }
        }
    }
}

pub(super) async fn event(
    Path(eid): Path<Uuid>,
    Extension(dynamo): Extension<Backend>,
) -> Result<Json<Value>, StatusCode> {
    match dynamo.event(&eid).await {
        Ok(v) => {
            if let Some(_) = v.item() {
                // TODO: never-expire cache header
                Ok(Json(serde_json::json!({})))
            } else {
                warn!(%eid, "non-existing event");
                return Err(http::StatusCode::NOT_FOUND);
            }
        }
        Err(e) => {
            error!(%eid, error = %e, "dynamodb event request failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
