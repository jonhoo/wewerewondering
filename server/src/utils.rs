use crate::{Backend, Local};
use aws_sdk_dynamodb::{error::SdkError, types::AttributeValue};
use aws_smithy_types::body::SdkBody;
use http::StatusCode;
use std::time::SystemTime;
use tracing::{error, warn};
use ulid::Ulid;

pub(crate) fn to_dynamo_timestamp(time: SystemTime) -> AttributeValue {
    AttributeValue::N(
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string(),
    )
}

pub fn mint_service_error<E>(e: E) -> SdkError<E> {
    SdkError::service_error(
        e,
        aws_smithy_runtime_api::http::Response::new(
            aws_smithy_runtime_api::http::StatusCode::try_from(200).unwrap(),
            SdkBody::empty(),
        ),
    )
}

pub async fn get_secret(dynamo: &Backend, eid: &Ulid) -> Result<String, StatusCode> {
    match dynamo {
        Backend::Dynamo(dynamo) => {
            match dynamo
                .get_item()
                .table_name("events")
                .key("id", AttributeValue::S(eid.to_string()))
                .projection_expression("secret")
                .send()
                .await
            {
                Ok(v) => {
                    if let Some(s) = v
                        .item()
                        .and_then(|e| e.get("secret"))
                        .and_then(|s| s.as_s().ok())
                    {
                        Ok(s.clone())
                    } else {
                        warn!(%eid, "attempted to access non-existing event");
                        Err(StatusCode::NOT_FOUND)
                    }
                }
                Err(e) => {
                    error!(%eid, error = %e, "dynamodb event request for secret verificaton failed");
                    Err(http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Backend::Local(local) => {
            let mut local = local.lock().unwrap();
            let Local { events, .. } = &mut *local;
            match events.get(eid) {
                Some(s) => Ok(s.clone()),
                None => Err(StatusCode::NOT_FOUND),
            }
        }
    }
}

pub async fn check_secret(dynamo: &Backend, eid: &Ulid, secret: &str) -> Result<(), StatusCode> {
    let s = get_secret(dynamo, eid).await?;
    if s == secret {
        Ok(())
    } else {
        warn!(%eid, secret, "attempted to access event with incorrect secret");
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Seed the database.
///
/// This will register a test event (with id `00000000000000000000000000`) and
/// a number of questions for it in the database, whether it's an in-memory [`Local`]
/// database or a local instance of DynamoDB. Note that in the latter case
/// we are checking if the test event is already there, and - if so - we are _not_ seeding
/// the questions. This is to avoid creating duplicated questions when re-running the app.
/// And this is not an issue of course when running against our in-memory [`Local`] database.
///
/// The returned vector contains IDs of the questions related to the test event.
#[cfg(debug_assertions)]
pub(crate) async fn seed(backend: &mut Backend) -> Vec<Ulid> {
    use crate::{ask, SEED};
    use std::sync::{Arc, Mutex};
    use tracing::{info, warn};

    #[derive(serde::Deserialize)]
    struct LiveAskQuestion {
        likes: usize,
        text: String,
        hidden: bool,
        answered: bool,
        #[serde(rename = "createTimeUnix")]
        created: usize,
    }

    let seed: Vec<LiveAskQuestion> = serde_json::from_str(SEED).unwrap();
    let seed_e = Ulid::from_string("00000000000000000000000000").unwrap();
    let seed_e_secret = "secret";

    info!("going to seed test event");
    match backend.event(&seed_e).await.unwrap() {
        output if output.item().is_some() => {
            warn!("test event is already there, skipping seeding questions");
        }
        _ => {
            backend.new(&seed_e, seed_e_secret).await.unwrap();
            info!("successfully registered test event, going to seed questions now");
            // first create questions ...
            let mut qs = Vec::new();
            for q in seed {
                let qid = ulid::Ulid::new();
                backend
                    .ask(
                        &seed_e,
                        &qid,
                        ask::Question {
                            body: q.text,
                            asker: None,
                        },
                    )
                    .await
                    .unwrap();
                qs.push((qid, q.created, q.likes, q.hidden, q.answered));
            }
            // ... then set the vote count + answered/hidden flags
            match backend {
                Backend::Dynamo(ref mut client) => {
                    use aws_sdk_dynamodb::types::BatchStatementRequest;
                    // DynamoDB supports batch operations using PartiQL syntax with `25` as max batch size
                    // https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_BatchExecuteStatement.html
                    for chunk in qs.chunks(25) {
                        let batch_update = chunk
                            .iter()
                            .map(|(qid, created, votes, hidden, answered)| {
                                let builder =  BatchStatementRequest::builder();
                                let builder = if *answered {
                                    builder.statement(
                                       // numerous words are reserved in the DynamoDB engine (e.g. Key, Id, When) and
                                       // should be qouted; we are quoting all of our attrs to avoid possible collisions
                                       r#"UPDATE "questions" SET "answered"=? SET "votes"=? SET "when"=? SET "hidden"=? WHERE "id"=?"#,
                                    )
                                    .parameters(to_dynamo_timestamp(SystemTime::now())) // answered
                                } else {
                                    builder.statement(
                                       r#"UPDATE "questions" SET "votes"=? SET "when"=? SET "hidden"=? WHERE "id"=?"#,
                                    )
                                };
                                builder
                                .parameters(AttributeValue::N(votes.to_string())) // votes
                                .parameters(AttributeValue::N(created.to_string())) // when
                                .parameters(AttributeValue::Bool(*hidden)) // hidden
                                .parameters(AttributeValue::S(qid.to_string())) // id
                                .build()
                                .unwrap()
                            })
                            .collect::<Vec<_>>();
                        client
                            .batch_execute_statement()
                            .set_statements(Some(batch_update))
                            .send()
                            .await
                            .expect("batch to have been written ok");
                    }
                }
                Backend::Local(ref mut state) => {
                    let state = Arc::get_mut(state).unwrap();
                    let state = Mutex::get_mut(state).unwrap();
                    for (qid, created, votes, hidden, answered) in qs {
                        let q = state.questions.get_mut(&qid).unwrap();
                        q.insert("votes", AttributeValue::N(votes.to_string()));
                        if answered {
                            q.insert("answered", to_dynamo_timestamp(SystemTime::now()));
                        }
                        q.insert("hidden", AttributeValue::Bool(hidden));
                        q.insert("when", AttributeValue::N(created.to_string()));
                    }
                }
            }
            info!("successfully registered questions");
        }
    }
    // let's collect ids of the questions related to the test event,
    // we can then use them to auto-generate user votes over time
    backend
        .list(&seed_e, true)
        .await
        .expect("scenned index ok")
        .items()
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .expect("id is in projection")
                .as_s()
                .expect("id is of type string");
            // NB! If you are creating entries manually via the DynamoDB Web UI (or CLI)
            // when developing and testing, make sure you are putting valid ulids as ids,
            // since the db server will only check that `id` respects the `S` type, and so
            // will not error back to you when you are saving a question (say, via Web UI)
            // with `id="string-that-is-not-valid-ulid"`.
            ulid::Ulid::from_string(id).expect("all ids to be valid ulids in the table")
        })
        .collect()
}
