use crate::utils::{QuestionState, TestContext};
use fantoccini::Locator;

async fn guest_asks_question_and_host_answers(
    TestContext {
        host: h,
        guest1: g1,
        guest2: g2,
        dynamo: d,
    }: TestContext,
) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let (eid, url) = h.create_event().await;
    // initially no questions
    assert!(h.expect_questions(QuestionState::Pending).await.is_err());

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
    assert_eq!(d.event_questions(&eid).await.unwrap().count, 0);

    // ------------------------ guest window ---------------------------------
    // guest opens the link and ...
    g1.goto(url.as_str()).await.unwrap();
    // ... asks a question, which ...
    let (qtext, qauthor) = (
        "Did you attend Rust Forge conference in Wellington in 2025?",
        "Tim",
    );
    g1.ask(qtext, Some(qauthor)).await.unwrap();

    // ... appears on the screen
    let pending = g1.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // ------------------------ second guest window ----------------------
    // second guest can also see the question
    g2.goto(url.as_str()).await.unwrap();
    let pending = g2.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // ------------------------ host window ----------------------------------
    // host sees the newly (and the only) asked question
    let pending = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // the host also observes two action options: they can either mark
    // the question as answered or hide it
    let answer_button = pending[0]
        .find(Locator::Css(r#"button[data-action="mark_answered"]"#))
        .await
        .unwrap();
    let _hide_button = pending[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
    // the host asks the question elsewhere (e.g. during the live stream) and
    // marks the question as answered
    answer_button.click().await.unwrap();

    // the host observes the text "No unanswered questions." which might be
    // satisfying, but also can be sad depending on the context and mood
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions h2"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // and Tim's question travelled to the "answered" container:
    let answered = h.expect_questions(QuestionState::Answered).await.unwrap();
    assert_eq!(answered.len(), 1);
    // sanity: let's check that this is actually Tim's question
    assert!(answered[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // ------------------------ guest window ---------------------------------
    // let's switch to Tim's window for a sec and check they also observe their
    // question having been answered
    g1.wait_for_polling().await;
    let guest_pending = g1
        .wait_for_element(Locator::Css("#pending-questions"))
        .await
        .unwrap();
    assert!(guest_pending
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // sanity: probably obvious but let's actually check that the guest cannot
    // mark the question as unanswered, neither hide it; in fact they cannot do
    // much about the question they asked:
    let guest_answered = g1
        .wait_for_element(Locator::Id("answered-questions"))
        .await
        .unwrap();
    let guest_answered_questions = guest_answered
        .find_all(Locator::Css("article"))
        .await
        .unwrap();
    assert_eq!(guest_answered_questions.len(), 1);
    assert!(guest_answered_questions[0]
        .find(Locator::Css(r#"button[data-action="mark_not_answered"]"#))
        .await
        .is_err());
    assert!(guest_answered_questions[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .is_err());

    // ------------------------ host window ----------------------------------
    // ok let's hop back to the host window to see what actions they now have
    // available for the answered question; they can either mark the already answered
    // question as unanswered ...
    let _unanswer_button = answered[0]
        .find(Locator::Css(r#"button[data-action="mark_not_answered"]"#))
        .await
        .unwrap();
    // ... or hide the question (just like they could do with that question
    // when it was not answered)
    let _hide_answered_button = answered[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();

    // --------------------------- database ----------------------------------
    let questions = d.event_questions(eid).await.unwrap();
    assert_eq!(questions.count, 1);
    let qid = questions.items().first().unwrap().get("id").unwrap();
    let q = d.question_by_id(qid.to_owned()).await.unwrap();
    let q = q.item().unwrap();
    assert_eq!(q.get("who").unwrap().as_s().unwrap(), qauthor);
    assert_eq!(q.get("text").unwrap().as_s().unwrap(), qtext);
    assert_eq!(q.get("votes").unwrap().as_n().unwrap(), "1");
    assert!(q.get("answered").is_some());
    assert!(!q.get("hidden").unwrap().as_bool().unwrap());
}

async fn guest_asks_question_and_host_hides_it(
    TestContext {
        host: h,
        guest1: g1,
        guest2: g2,
        dynamo: d,
    }: TestContext,
) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let (eid, url) = h.create_event().await;
    // initially no questions
    assert!(h.expect_questions(QuestionState::Pending).await.is_err());

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
    assert_eq!(d.event_questions(&eid).await.unwrap().count, 0);

    // ---------------- first guest (not a gentle person) window -------------
    // guest opens the link and ...
    g1.goto(url.as_str()).await.unwrap();
    // ... asks a question ... no, instead they write some toxic stuff
    let qtext = "I hate your streams and everything about you";
    g1.ask(qtext, None).await.unwrap();

    // ... and well - this toxic stuff also appears on the screen, but ...
    assert!(
        g1.expect_questions(QuestionState::Pending).await.unwrap()[0]
            .text()
            .await
            .unwrap()
            .to_lowercase()
            .contains(&qtext.to_lowercase())
    );

    // ------------------------ second guest window ----------------------
    // second guest sees the "question" ...
    g2.goto(url.as_str()).await.unwrap();
    assert!(
        g2.expect_questions(QuestionState::Pending).await.unwrap()[0]
            .text()
            .await
            .unwrap()
            .to_lowercase()
            .contains(&qtext.to_lowercase())
    );

    // ------------------------ host window ----------------------------------
    // the host sees it and just decides to hide it: there is not much they
    // can do about it
    h.wait_for_polling().await;
    let pending = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));
    pending[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // the host observes text "No unanswered questions"
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions h2"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // this "question" has been moved to a special container (not the one for
    // answered questions)
    let hidden = h.expect_questions(QuestionState::Hidden).await.unwrap();
    assert_eq!(hidden.len(), 1);
    assert!(hidden[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));

    // ---------------- first guest (not a gentle person) window -------------
    // just like in the "answer" scenario, let's wait until the changes
    // are sent to the server and synced back
    g1.wait_for_polling().await;
    // they still see their "question" on the screen even though it has been
    // hidden by the host (later in this test we are demonstrating that the
    // question gets hidden from _other_ guests)
    let pending = h.expect_questions(QuestionState::Hidden).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));
    // NB: the host would see these questions in the hidden as tested above
    assert!(g1.expect_questions(QuestionState::Hidden).await.is_err());

    // ------------------------ second guest window ----------------------
    // the "question" disappears for the second guest
    assert!(g2
        .wait_for_element(Locator::Css("#pending-questions h2"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));

    // NB: same as in the bad guy's case, the good guy will not see that
    // the question got to the hidden section
    assert!(g2.expect_questions(QuestionState::Hidden).await.is_err());

    // --------------------------- database ----------------------------------
    let questions = d.event_questions(eid).await.unwrap();
    assert_eq!(questions.count, 1);
    let qid = questions.items().first().unwrap().get("id").unwrap();
    let q = d.question_by_id(qid.to_owned()).await.unwrap();
    let q = q.item().unwrap();
    assert_eq!(q.get("text").unwrap().as_s().unwrap(), qtext);
    assert_eq!(q.get("votes").unwrap().as_n().unwrap(), "1");
    // Notice how skipped providing a signature when we were "asking a question"
    // (reminder: proding your name/nickname is optional)
    assert!(q.get("who").is_none());
    assert!(q.get("hidden").unwrap().as_bool().unwrap()); // NB
    assert!(q.get("answered").is_none());
}

mod tests {
    crate::serial_test!(guest_asks_question_and_host_answers);
    crate::serial_test!(guest_asks_question_and_host_hides_it);
}
