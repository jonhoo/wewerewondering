use crate::utils::{GrantClipboardReadCmd, QuestionState, TestContext};
use aws_sdk_dynamodb::types::AttributeValue;
use fantoccini::Locator;
use std::collections::HashMap;
use url::Url;

async fn host_starts_new_q_and_a_session(
    TestContext {
        host: h, dynamo, ..
    }: TestContext,
) {
    // the host navigates to the app's welcome page
    h.goto_homepage().await;

    assert_eq!(h.title().await.unwrap(), "Q&A");
    let create_event_btn = h
        .wait_for_element(Locator::Id("create-event-button"))
        .await
        .unwrap();

    // starts a new Q&A session and ...
    create_event_btn.click().await.unwrap();

    // ... gets redirected to the event's host view where they can ...
    let share_event_btn = h
        .wait_for_element(Locator::Id("share-event-button"))
        .await
        .unwrap();
    let event_url_for_host = h.current_url().await.unwrap();
    let mut params = event_url_for_host.path_segments().unwrap();
    assert_eq!(params.next().unwrap(), "event");
    let event_id = params.next().unwrap();
    let event_secret = params.next().unwrap();
    assert!(params.next().is_none());

    // ... grab the event's guest url to share it later with folks
    share_event_btn.click().await.unwrap();
    h.issue_cmd(GrantClipboardReadCmd).await.unwrap();
    let event_url_for_guest: Url = h
        .execute_async(
            r#"
                const [callback] = arguments;
                navigator.clipboard.readText().then((text) => callback(text));
            "#,
            vec![],
        )
        .await
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let mut params = event_url_for_guest.path_segments().unwrap();
    assert_eq!(params.next().unwrap(), "event");
    let event_id_for_guest = params.next().unwrap();

    assert_eq!(event_id_for_guest, event_id); // same event id
    assert!(params.next().is_none()); // but no secret

    // and there are currently no pending, answered, or hidden questions
    // related to the newly created event
    h.expect_questions(QuestionState::Pending)
        .await
        .unwrap_err()
        .is_timeout();
    h.expect_questions(QuestionState::Answered)
        .await
        .unwrap_err()
        .is_timeout();
    h.expect_questions(QuestionState::Hidden)
        .await
        .unwrap_err()
        .is_timeout();

    // let's make sure we are persisting the event...
    let event = dynamo
        .get_item()
        .table_name("events")
        .key("id", AttributeValue::S(event_id.into()))
        .send()
        .await
        .unwrap()
        .item
        .unwrap();
    assert_eq!(
        event.get("id").unwrap(),
        &AttributeValue::S(event_id.into())
    );
    assert_eq!(
        event.get("secret").unwrap(),
        &AttributeValue::S(event_secret.into())
    );
    // ... and there are actually no questions associated
    // with that event
    assert_eq!(
        dynamo
            .query()
            .table_name("questions")
            .index_name("top")
            .key_condition_expression("eid = :eid")
            .expression_attribute_values(":eid", AttributeValue::S(event_id.into()))
            .send()
            .await
            .unwrap()
            .count,
        0
    )
}

async fn guest_asks_question_and_it_shows_up(
    TestContext {
        host: h,
        guest1: g,
        dynamo: d,
        ..
    }: TestContext,
) {
    // ------------------------ host window ----------------------------------
    // we've got a new event
    let guest_url = h.create_event().await;
    let event_id = guest_url.path_segments().unwrap().next_back().unwrap();

    // the host can see that nobody has asked
    // a question - at least not just yet
    h.expect_questions(QuestionState::Pending)
        .await
        .unwrap_err()
        .is_timeout();

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
    assert_eq!(d.event_questions(event_id).await.unwrap().count, 0);

    // ------------------------ guest window ---------------------------------
    // a guest visits the event's page and ...
    g.goto(guest_url.as_str()).await.unwrap();

    // they do not observe any questions either, so ...
    g.expect_questions(QuestionState::Pending)
        .await
        .unwrap_err()
        .is_timeout();

    // ... they click the "Ask another question" button ...
    g.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // they can see a prompt
    let alert = g.get_alert_text().await.unwrap();
    assert!(alert.to_lowercase().contains("question"));

    // ... and they decide to enter a single word
    g.send_alert_text("What?").await.unwrap();
    g.accept_alert().await.unwrap();

    // but the app asks them to enter at least a couple of words
    assert!(g
        .get_alert_text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("at least two words"));

    // and they say ok ...
    g.accept_alert().await.unwrap();

    // and the app shows them the prompt again
    assert!(g
        .get_alert_text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("question"));

    // this time they enter a _few_ words ...
    let q_submitted = "What is this life, if, full of care, we have no time to stand and stare?";
    g.send_alert_text(q_submitted).await.unwrap();
    g.accept_alert().await.unwrap();

    // ... and then they also leave their signature (which is optional btw)
    let name = "William Henry Davies";
    let alert = g.get_alert_text().await.unwrap();
    assert!(alert.to_ascii_lowercase().contains("signature"));
    g.send_alert_text(name).await.unwrap();
    g.accept_alert().await.unwrap();

    // and we they see their (and the only one) question on the screen
    let pending_questions = g.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending_questions.len(), 1);
    assert!(pending_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&q_submitted.to_lowercase()));
    // sanity: let's make sure the question is attributed to them
    assert!(pending_questions[0]
        .find(Locator::Css(".question__by"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&name.to_lowercase()));

    // ------------------------ host window ----------------------------------
    // let's check that the host can also see this question
    let pending_questions = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending_questions.len(), 1);
    assert!(pending_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&q_submitted.to_lowercase()));

    // --------------------------- database ----------------------------------
    // finally, let's verify that the question has been persisted
    let questions = d.event_questions(event_id).await.unwrap();
    assert_eq!(questions.count, 1); // NB
    let qid = questions.items().first().unwrap().get("id").unwrap();
    let q_stored = d.question_by_id(qid.to_owned()).await.unwrap();

    // For reference. The GetItem output will have the following shape:
    //
    // GetItemOutput {
    //   item: Some({
    //      "id": S("01JR92BQRZ9SJ8GBA0XK3NMMAJ"),
    //      "who": S("William Henry Davies"),
    //      "eid": S("01JR92BQ3BR02VPN5KM89H1KDK"),
    //      "text": S("What ... stare?"),
    //      "when": N("1744061194"),
    //      "expire": N("1746653194"),
    //      "hidden": Bool(false),
    //      "votes": N("1")}),
    //   consumed_capacity: None,
    //   _request_id: Some("376e203c-8e88-456a-861e-a76b7ca8bc25")
    // }

    // this is _their_ question (at least this is their signature)
    let who = q_stored.item().unwrap().get("who").unwrap().as_s().unwrap();
    assert_eq!(who, name);

    // and this _is_ the question they've just asked
    let text = q_stored
        .item()
        .unwrap()
        .get("text")
        .unwrap()
        .as_s()
        .unwrap();
    assert_eq!(text, q_submitted);
}

mod tests {
    crate::serial_test!(host_starts_new_q_and_a_session);
    crate::serial_test!(guest_asks_question_and_it_shows_up);
}
