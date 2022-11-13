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
        set: bool,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let q = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let q = q.update_expression("SET #field = :set");
                let q = match property {
                    Property::Hidden => q.expression_attribute_names("#field", "hidden"),
                    Property::Answered => q.expression_attribute_names("#field", "answered"),
                };
                let q = q.expression_attribute_values(":set", AttributeValue::Bool(set));

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
    body: String,
    Extension(dynamo): Extension<Backend>,
) -> Result<(), StatusCode> {
    super::check_secret(&dynamo, &eid, &secret).await?;

    let set = match &*body {
        "on" => true,
        "off" => false,
        _ => {
            error!(%qid, body, "invalid toggle value");
            return Err(http::StatusCode::BAD_REQUEST);
        }
    };

    match dynamo.toggle(&qid, property, set).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn inner(backend: Backend) {
        let eid = Uuid::new_v4();
        let secret = "cargo-test";
        let _ = backend.new(&eid, secret).await.unwrap();
        let qid = Uuid::new_v4();
        let qid_v = AttributeValue::S(qid.to_string());
        backend.ask(&eid, &qid, "hello world").await.unwrap();

        let check = |qids: aws_sdk_dynamodb::output::QueryOutput,
                     expect: Option<(bool, bool, usize)>| {
            let q = qids
                .items()
                .into_iter()
                .flatten()
                .find(|q| q["id"] == qid_v);
            if let Some((hidden, answered, votes)) = expect {
                assert_ne!(
                    q, None,
                    "newly created question {qid} was not listed in {qids:?}"
                );
                let q = q.unwrap();
                assert_eq!(q["votes"], AttributeValue::N(votes.to_string()));
                assert_eq!(q["answered"], AttributeValue::Bool(answered));
                assert_eq!(q["hidden"], AttributeValue::Bool(hidden));
                assert_eq!(qids.count(), 1, "extra questions in response: {qids:?}");
            } else {
                assert_eq!(
                    q, None,
                    "newly created question {qid} was not listed in {qids:?}"
                );
            }
        };

        // only admin should see hidden
        backend.toggle(&qid, Property::Hidden, true).await.unwrap();
        check(
            backend.list(&eid, true).await.unwrap(),
            Some((true, false, 1)),
        );
        check(backend.list(&eid, false).await.unwrap(), None);

        // should toggle back
        backend.toggle(&qid, Property::Hidden, false).await.unwrap();
        // and should now show up as answered
        backend
            .toggle(&qid, Property::Answered, true)
            .await
            .unwrap();
        check(
            backend.list(&eid, true).await.unwrap(),
            Some((false, true, 1)),
        );
        check(
            backend.list(&eid, false).await.unwrap(),
            Some((false, true, 1)),
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
