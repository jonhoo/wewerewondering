use crate::utils::{GrantClipboardReadCmd, QuestionState, TestContext};
use aws_sdk_dynamodb::types::AttributeValue;
use fantoccini::Locator;
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
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());
    assert!(h.await_questions(QuestionState::Answered).await.is_empty());
    assert!(h.await_questions(QuestionState::Hidden).await.is_empty());

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
    let (eid, url) = h.create_event().await;

    // the host can see that nobody has asked
    // a question - at least not just yet
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
    assert_eq!(d.event_questions(&eid).await.unwrap().count, 0);

    // ------------------------ guest window ---------------------------------
    // a guest visits the event's page and ...
    g.goto(url.as_str()).await.unwrap();

    // they do not observe any questions either, so ...
    assert!(g.await_questions(QuestionState::Pending).await.is_empty());

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
    h.wait_for_polling().await;
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
    let questions = d.event_questions(eid).await.unwrap();
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

    // ----------------------- ANOTHER EVENT ----------------------------------
    // ----------------------- host window ------------------------------------
    // say, the host now creates another event ...
    let (new_eid, new_url) = h.create_event().await;
    // since this is a brand new event, there are no questions on the screen
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());

    // -------------------------- database ------------------------------------
    // ... nor in the database
    let questions = d.event_questions(new_eid).await.unwrap();
    assert_eq!(questions.count, 0); // NB

    // ------------------------ guest window ---------------------------------
    // same for the guest: they are not seeing the question they asked during
    // the earlier event
    g.goto(new_url.as_str()).await.unwrap();
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());

    // but if they decide to visit the earlier event again ...
    g.goto(url.as_str()).await.unwrap();
    // ... they will see their earlier question
    let pending_questions = g.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending_questions.len(), 1);
    assert!(pending_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&q_submitted.to_lowercase()));
}

/// This is checking that the user does not observe questions that are asked
/// while they (the user) have got updates paused, but this also serves as a
/// regression test for `https://github.com/jonhoo/wewerewondering/issues/290`.
async fn user_pauses_and_resumes_updates(
    TestContext {
        host: h, guest1: g, ..
    }: TestContext,
) {
    // ------------------------ host window -----------------------------------
    let (_eid, url) = h.create_event().await;
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());
    // ------------------------ guest window ----------------------------------
    // guest opens the link and asks a question ...
    g.goto(url.as_str()).await.unwrap();
    let (qtext, qauthor) = (
        "Why did not they try to keep the Zig community on GitHub?",
        "Andrew",
    );
    g.ask(qtext, Some(qauthor)).await.unwrap();
    // ... and it shows up (as the only one)
    let pending = g.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));
    // ------------------------ host window ----------------------------------
    // host sees this question...
    let pending = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));
    // ... and decides to pause updates (reminder: pausing updates is not the
    // host's privilege rather every user can do this)
    let toggle_updates_btn = h
        .find(Locator::Id("toggle-updates-button"))
        .await
        .expect("toggle updates button to be on the screen");
    assert!(toggle_updates_btn
        .text()
        .await
        .unwrap()
        // used to offer pause ...
        .eq_ignore_ascii_case("pause updates"));
    // ... but after click ...
    toggle_updates_btn.click().await.unwrap();
    assert!(toggle_updates_btn
        .text()
        .await
        .unwrap()
        // ... suggests to "resume"
        .eq_ignore_ascii_case("resume updates"));

    // the host now switches to another tab
    let host_app_tab = h.window().await.unwrap();
    let host_not_app_tab = h
        .new_window(true)
        .await
        .expect("new tab to have been created")
        .handle;
    h.switch_to_window(host_not_app_tab)
        .await
        .expect("to have switched to the new tab just fine");

    // ------------------------ guest window ----------------------------------
    // in the meantime, the guest asks another question
    // (it's actually same user, they are just using a different signature this time)
    let (next_qtext, next_qauthor) = ("Does the wind still blow over Savannah?", "Charles B.");
    g.ask(next_qtext, Some(next_qauthor)).await.unwrap();
    let pending_questions = g.await_questions(QuestionState::Pending).await;
    assert_eq!(pending_questions.len(), 2);

    // ------------------------ host window ----------------------------------
    // the host now switches back to the app's tab ...
    h.switch_to_window(host_app_tab)
        .await
        .expect("to have switched back to the app's tab");
    // ... and observes the question list, but a stale one,
    // becase they still got updates disabled
    let pending = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // let's now resume updates ...
    let toggle_updates_btn = h
        .find(Locator::Id("toggle-updates-button"))
        .await
        .expect("toggle updates button to be on the screen");
    assert!(toggle_updates_btn
        .text()
        .await
        .unwrap()
        // the updates are still paused
        .eq_ignore_ascii_case("resume updates"));
    toggle_updates_btn.click().await.unwrap();
    assert!(toggle_updates_btn
        .text()
        .await
        .unwrap()
        // back to initial button text
        .eq_ignore_ascii_case("pause updates"));

    // ... and verify thet the list is up-to-date
    let pending_quesions = h.await_questions(QuestionState::Pending).await;
    assert_eq!(pending_quesions.len(), 2);
}

mod tests {
    crate::serial_test!(host_starts_new_q_and_a_session);
    crate::serial_test!(guest_asks_question_and_it_shows_up);
    crate::serial_test!(user_pauses_and_resumes_updates);
}
