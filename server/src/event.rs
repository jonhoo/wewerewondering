use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::get_item::{GetItemError, GetItemOutput},
    types::AttributeValue,
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
    pub(super) async fn event(&self, eid: &Ulid) -> Result<GetItemOutput, SdkError<GetItemError>> {
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
    Path(eid): Path<Ulid>,
    State(dynamo): State<Backend>,
) -> (
    AppendHeaders<[(HeaderName, &'static str); 1]>,
    Result<Json<Value>, StatusCode>,
) {
    match dynamo.event(&eid).await {
        Ok(v) => {
            if v.item().is_some() {
                (
                    AppendHeaders([(header::CACHE_CONTROL, "max-age=864001")]),
                    Ok(Json(serde_json::json!({}))),
                )
            } else {
                warn!(%eid, "non-existing event");
                (
                    // it's relatively unlikely that an event Ulid that didn't exist will start
                    // existing. but just in case, don't make it _too_ long.
                    AppendHeaders([(header::CACHE_CONTROL, "max-age=3600")]),
                    Err(http::StatusCode::NOT_FOUND),
                )
            }
        }
        Err(e) => {
            error!(%eid, error = %e, "dynamodb event request failed");
            (
                AppendHeaders([(header::CACHE_CONTROL, "no-cache")]),
                Err(http::StatusCode::INTERNAL_SERVER_ERROR),
            )
        }
    }
}
