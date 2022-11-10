use super::{Backend, Local};
use aws_sdk_dynamodb::{
    error::UpdateItemError, model::AttributeValue, output::UpdateItemOutput, types::SdkError,
};
use axum::extract::{Extension, Path};
use http::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub(super) enum Property {
    Hidden,
    Answered,
}

impl Backend {
    pub(super) async fn toggle(
        &self,
        qid: &Uuid,
        property: Property,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let q = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let q = match property {
                    Property::Hidden => q.update_expression("SET hidden = NOT hidden"),
                    Property::Answered => q.update_expression("SET answered = NOT answered"),
                };

                q.send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                fn invert(q: &mut HashMap<&'static str, AttributeValue>, key: &'static str) {
                    if let AttributeValue::Bool(b) = q[key] {
                        q.insert(key, AttributeValue::Bool(!b));
                    } else {
                        unreachable!("all properties are bools");
                    }
                }

                let q = questions
                    .get_mut(qid)
                    .expect("toggle property on unknown question ");
                match property {
                    Property::Hidden => invert(q, "hidden"),
                    Property::Answered => invert(q, "answered"),
                }

                Ok(UpdateItemOutput::builder().build())
            }
        }
    }
}

pub(super) async fn toggle(
    Path((eid, secret, qid, property)): Path<(Uuid, String, Uuid, Property)>,
    Extension(dynamo): Extension<Backend>,
) -> Result<(), StatusCode> {
    super::check_secret(&dynamo, &eid, &secret).await?;

    match dynamo.toggle(&qid, property).await {
        Ok(_) => {
            debug!(%eid, %qid, p = ?property, "toggled question property");
            Ok(())
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb request to vote for question failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
