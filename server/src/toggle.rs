use crate::utils;
use crate::{Backend, Local};
use aws_sdk_dynamodb::{
    error::SdkError,
    operation::update_item::{UpdateItemError, UpdateItemOutput},
    types::AttributeValue,
};
use axum::{
    extract::{Path, State},
    Json,
};
use http::StatusCode;
use serde::Deserialize;
use std::time::SystemTime;
use ulid::Ulid;

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub(super) enum Property {
    Hidden,
    Answered,
}

#[derive(Debug, Copy, Clone)]
pub(super) enum ToggleRequest {
    Hidden(bool),
    Answered(Option<SystemTime>),
}

impl Backend {
    pub(super) async fn toggle(
        &self,
        qid: &Ulid,
        req: ToggleRequest,
    ) -> Result<UpdateItemOutput, SdkError<UpdateItemError>> {
        match self {
            Self::Dynamo(dynamo) => {
                let q = dynamo
                    .update_item()
                    .table_name("questions")
                    .key("id", AttributeValue::S(qid.to_string()));

                let q = match req {
                    ToggleRequest::Hidden(set) => q
                        .update_expression("SET #field = :set")
                        .expression_attribute_names("#field", "hidden")
                        .expression_attribute_values(":set", AttributeValue::Bool(set)),
                    ToggleRequest::Answered(time) => {
                        if let Some(time) = time {
                            q.update_expression("SET #field = :set")
                                .expression_attribute_names("#field", "answered")
                                .expression_attribute_values(
                                    ":set",
                                    utils::to_dynamo_timestamp(time),
                                )
                        } else {
                            q.update_expression("REMOVE #field")
                                .expression_attribute_names("#field", "answered")
                        }
                    }
                };
                q.send().await
            }
            Self::Local(local) => {
                let mut local = local.lock().unwrap();
                let Local { questions, .. } = &mut *local;

                let q = questions
                    .get_mut(qid)
                    .expect("toggle property on unknown question ");
                match req {
                    ToggleRequest::Hidden(set) => q.insert("hidden", AttributeValue::Bool(set)),
                    ToggleRequest::Answered(time) => {
                        if let Some(time) = time {
                            q.insert("answered", utils::to_dynamo_timestamp(time))
                        } else {
                            q.remove("answered")
                        }
                    }
                };

                Ok(UpdateItemOutput::builder().build())
            }
        }
    }
}

pub(super) async fn toggle(
    Path((eid, secret, qid, property)): Path<(Ulid, String, Ulid, Property)>,
    State(dynamo): State<Backend>,
    body: String,
) -> Result<Json<serde_json::Value>, StatusCode> {
    utils::check_secret(&dynamo, &eid, &secret).await?;

    let req = match (&*body, property) {
        ("on", Property::Hidden) => ToggleRequest::Hidden(true),
        ("off", Property::Hidden) => ToggleRequest::Hidden(false),
        ("on", Property::Answered) => ToggleRequest::Answered(Some(SystemTime::now())),
        ("off", Property::Answered) => ToggleRequest::Answered(None),
        _ => {
            error!(%qid, body, "invalid toggle value");
            return Err(http::StatusCode::BAD_REQUEST);
        }
    };

    match dynamo.toggle(&qid, req).await {
        Ok(_) => {
            debug!(%eid, %qid, p = ?property, "toggled question property");
            match req {
                ToggleRequest::Hidden(set) => Ok(Json(serde_json::json!({ "hidden": set }))),
                ToggleRequest::Answered(time) => {
                    if let Some(time) = time {
                        let time = time
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        Ok(Json(serde_json::json!({ "answered": time })))
                    } else {
                        Ok(Json(serde_json::json!({})))
                    }
                }
            }
        }
        Err(e) => {
            error!(%qid, error = %e, "dynamodb request to toggle question property failed");
            Err(http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;
    use axum::Json;
    use serde_json::Value;

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
        let qid_u = Ulid::from_string(qid).unwrap();

        #[allow(clippy::type_complexity)]
        let check = |qids: Value, expect: Option<(bool, Box<dyn Fn(&Value)>, u64)>| {
            let qids = qids.as_array().unwrap();
            let q = qids.iter().find(|q| dbg!(&q["qid"]) == dbg!(qid));
            if let Some((hidden, check_answered, votes)) = expect {
                assert_ne!(
                    q, None,
                    "newly created question {qid} was not listed in {qids:?}"
                );
                let q = q.unwrap();
                assert_eq!(q["votes"].as_u64().unwrap(), votes);
                check_answered(q);
                assert_eq!(q["hidden"].as_bool().unwrap(), hidden);
                assert_eq!(qids.len(), 1, "extra questions in response: {qids:?}");
            } else {
                assert_eq!(
                    q, None,
                    "newly created question {qid} was not listed in {qids:?}"
                );
            }
        };

        let check_answered_set = |json: &Value| {
            let answered = &json["answered"];
            assert!(answered.is_u64(), "answered is not a u64: {answered}");
            let answered = answered.as_u64().unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            assert!(
                now >= answered,
                "answered is time travelling: [now: {now} | answered: {answered}]"
            );
            assert!(
                now - answered <= 30,
                "answered not within the last 30 sec: [now: {now} | answered: {answered}]"
            )
        };

        let check_answered_unset = |json: &Value| {
            let answered = &json["answered"];
            assert!(answered.is_null(), "answered should be null: {answered}");
        };

        // only admin should see hidden
        let toggle_res = super::toggle(
            Path((eid, secret.to_string(), qid_u, Property::Hidden)),
            State(backend.clone()),
            String::from("on"),
        )
        .await
        .unwrap();
        assert!(toggle_res["hidden"]
            .as_bool()
            .expect("hidden should be a bool"));

        check(
            crate::list::list_all(Path((eid, secret.to_string())), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            Some((true, Box::new(check_answered_unset), 1)),
        );
        check(
            crate::list::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            None,
        );

        // should toggle back
        let toggle_res = super::toggle(
            Path((eid, secret.to_string(), qid_u, Property::Hidden)),
            State(backend.clone()),
            String::from("off"),
        )
        .await
        .unwrap();
        assert!(!toggle_res["hidden"]
            .as_bool()
            .expect("hidden should be a bool"));

        // and should now show up as answered
        let toggle_res = super::toggle(
            Path((eid, secret.to_string(), qid_u, Property::Answered)),
            State(backend.clone()),
            String::from("on"),
        )
        .await
        .unwrap();
        check_answered_set(&toggle_res);

        check(
            crate::list::list_all(Path((eid, secret.to_string())), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            Some((false, Box::new(check_answered_set), 1)),
        );
        check(
            crate::list::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            Some((false, Box::new(check_answered_set), 1)),
        );

        // answered should toggle back
        let toggle_res = super::toggle(
            Path((eid, secret.to_string(), qid_u, Property::Answered)),
            State(backend.clone()),
            String::from("off"),
        )
        .await
        .unwrap();
        check_answered_unset(&toggle_res);

        check(
            crate::list::list_all(Path((eid, secret.to_string())), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            Some((false, Box::new(check_answered_unset), 1)),
        );
        check(
            crate::list::list(Path(eid), State(backend.clone()))
                .await
                .1
                .unwrap()
                .0,
            Some((false, Box::new(check_answered_unset), 1)),
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
