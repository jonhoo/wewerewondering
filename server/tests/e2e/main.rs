#![cfg(feature = "e2e-test")]

mod utils;

use aws_sdk_dynamodb::types::AttributeValue;
use fantoccini::Locator;
use std::collections::HashMap;
use url::Url;
use utils::{GrantClipboardReadCmd, TestContext};

async fn host_starts_new_q_and_a_session(TestContext { client: c, dynamo }: TestContext) {
    // the host navigates to the app's welcome page
    c.goto_homepage().await;

    assert_eq!(c.title().await.unwrap(), "Q&A");
    let create_event_btn = c
        .wait_for_element(Locator::Id("create-event-button"))
        .await
        .unwrap();

    // starts a new Q&A session and ...
    create_event_btn.click().await.unwrap();

    // ... gets redirected to the event's host view where they can ...
    let share_event_btn = c
        .wait_for_element(Locator::Id("share-event-button"))
        .await
        .unwrap();
    let event_url_for_host = c.current_url().await.unwrap();
    let mut params = event_url_for_host.path_segments().unwrap();
    assert_eq!(params.next().unwrap(), "event");
    let event_id = params.next().unwrap();
    let event_secret = params.next().unwrap();
    assert!(params.next().is_none());

    // ... grab the event's guest url to share it later with folks
    share_event_btn.click().await.unwrap();
    c.issue_cmd(GrantClipboardReadCmd).await.unwrap();
    let event_url_for_guest: Url = c
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
    let pending_questions = c.wait_for_pending_questions().await.unwrap();
    assert!(pending_questions.is_empty());
    assert!(c
        .find(Locator::Id("answered-questions"))
        .await
        .unwrap_err()
        .is_no_such_element());
    assert!(c
        .find(Locator::Id("hidden-questions"))
        .await
        .unwrap_err()
        .is_no_such_element());

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

async fn guest_asks_question_and_it_shows_up(TestContext { client: c, dynamo }: TestContext) {
    // ------------------------ host window ----------------------------------
    // we've got a new event
    let guest_url = c.create_event().await;
    let event_id = guest_url.path_segments().unwrap().last().unwrap();

    // the host can see that nobody has asked
    // a question - at least not just yet
    let host_window = c.window().await.unwrap();
    assert!(c.wait_for_pending_questions().await.unwrap().is_empty());

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
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
    );

    // ------------------------ guest window ---------------------------------
    // a guest visits the event's page and ...
    let guest_window = c.new_window(false).await.unwrap();
    c.switch_to_window(guest_window.handle).await.unwrap();
    c.goto(&guest_url.as_str()).await.unwrap();

    // they do not observe any questions either, so ...
    assert!(c.wait_for_pending_questions().await.unwrap().is_empty());

    // ... they click the "Ask another question" button ...
    c.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // they can see a prompt
    let alert = c.get_alert_text().await.unwrap();
    assert!(alert.to_lowercase().contains("question"));

    // ... and they decide to enter a single word
    c.send_alert_text("What?").await.unwrap();
    c.accept_alert().await.unwrap();

    // but the app asks them to enter at least a couple of words
    assert!(c
        .get_alert_text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("at least two words"));

    // and they say ok ...
    c.accept_alert().await.unwrap();

    // and the app show them the prompt again
    assert!(c
        .get_alert_text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("question"));

    // this time they enter a _few_ words ...
    let q_submitted = "What is this life, if, full of care, we have no time to stand and stare?";
    c.send_alert_text(q_submitted).await.unwrap();
    c.accept_alert().await.unwrap();

    // ... and then they also leave they signature (which is optional btw)
    let name = "William Henry Davies";
    let alert = c.get_alert_text().await.unwrap();
    assert!(alert.to_ascii_lowercase().contains("signature"));
    c.send_alert_text(name).await.unwrap();
    c.accept_alert().await.unwrap();

    // let's make sure to await till question's details, such as text, creation
    // time, author have been fetched;, and check this is the question they've
    // entered into the prompt
    assert!(c
        .wait_for_element(Locator::Css("#pending-questions article .question__text"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&q_submitted.to_lowercase()));

    // and also that it's attributed to them
    assert!(c
        .wait_for_element(Locator::Css("#pending-questions article .question__by"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&name.to_lowercase()));

    // let's also check how many questions have been added to the
    // unanswered questions container, we can see one single question
    assert_eq!(c.wait_for_pending_questions().await.unwrap().len(), 1);

    // ------------------------ host window ----------------------------------
    // let's check that the host can also see this question
    c.switch_to_window(host_window).await.unwrap();
    assert!(c
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&q_submitted.to_lowercase()));
    // again, it's one single question
    assert_eq!(c.wait_for_pending_questions().await.unwrap().len(), 1);

    // --------------------------- database ----------------------------------
    // finally, let's verify that the question has been persisted
    let questions = dynamo
        .query()
        .table_name("questions")
        .index_name("top")
        .key_condition_expression("eid = :eid")
        .expression_attribute_values(":eid", AttributeValue::S(event_id.into()))
        .projection_expression("id,answered,#hidden")
        .expression_attribute_names("#hidden", "hidden")
        .send()
        .await
        .unwrap();
    assert_eq!(questions.count, 1); // NB
    let qid = questions.items().first().unwrap().get("id").unwrap();

    let q_stored = dynamo
        .get_item()
        .table_name("questions")
        .set_key(Some(HashMap::from([(String::from("id"), qid.to_owned())])))
        .send()
        .await
        .unwrap();

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
