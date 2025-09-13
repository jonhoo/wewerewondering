use crate::utils::{QuestionState, TestContext};
use fantoccini::Locator;

async fn guest_asks_question_and_host_answers(
    TestContext {
        host: h,
        guest1: g1,
        guest2: g2,
        ..
    }: TestContext,
) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let guest_url = h.create_event().await;
    // initially no questions
    assert!(h.expect_questions(QuestionState::Pending).await.is_err());

    // ------------------------ guest window ---------------------------------
    // guest opens the link and ...
    g1.goto(guest_url.as_str()).await.unwrap();
    // ... asks a question, which ...
    g1.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    let question_text = "Did you attend Rust Forge conference in Wellington in 2025?";
    g1.send_alert_text(question_text).await.unwrap();
    g1.accept_alert().await.unwrap();
    let question_author = "Tim";
    g1.send_alert_text(question_author).await.unwrap();
    g1.accept_alert().await.unwrap();
    // ... appears on the screen
    let questions = g1.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(questions.len(), 1);
    assert!(questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ------------------------ second guest window ----------------------
    // second guest can also see the question
    g2.goto(guest_url.as_str()).await.unwrap();
    let questions = g2.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(questions.len(), 1);
    assert!(questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ------------------------ host window ----------------------------------
    // host sees the newly (and the only) asked question
    let questions = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(questions.len(), 1);
    assert!(questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // the host also observes two action options: they can either mark
    // the question as answered or hide it
    let answer_button = questions[0]
        .find(Locator::Css(r#"button[data-action="mark_answered"]"#))
        .await
        .unwrap();
    let _hide_button = questions[0]
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
    let answered_questions = h.expect_questions(QuestionState::Answered).await.unwrap();
    assert_eq!(answered_questions.len(), 1);
    // sanity: let's check that this is actually Tim's question
    assert!(answered_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ------------------------ guest window ---------------------------------
    // let's switch to Tim's window for a sec and check they also observe their
    // question having been answered
    g1.wait_for_polling().await;
    let guest_pending_questions = g1
        .wait_for_element(Locator::Css("#pending-questions"))
        .await
        .unwrap();
    assert!(guest_pending_questions
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // sanity: probably obvious but let's actually check that the guest cannot
    // mark the question as unanswered, neither hide it; in fact they cannot do
    // much about the question they asked:
    let guest_answered_section = g1
        .wait_for_element(Locator::Id("answered-questions"))
        .await
        .unwrap();
    let guest_answered_questions = guest_answered_section
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
    let _unanswer_button = answered_questions[0]
        .find(Locator::Css(r#"button[data-action="mark_not_answered"]"#))
        .await
        .unwrap();
    // ... or hide the question (just like they could do with that question
    // when it was not answered)
    let _hide_answered_button = answered_questions[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
}

async fn guest_asks_question_and_host_hides_it(
    TestContext {
        host: h,
        guest1: g1,
        guest2: g2,
        ..
    }: TestContext,
) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let guest_url = h.create_event().await;
    // initially no questions
    assert!(h.expect_questions(QuestionState::Pending).await.is_err());

    // ---------------- first guest (not a gentle person) window -------------
    // guest opens the link and ...
    g1.goto(guest_url.as_str()).await.unwrap();
    // ... asks a question ... no, instead they write some toxic stuff
    g1.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    let question_text = "I hate your streams and everything about you";
    g1.send_alert_text(question_text).await.unwrap();
    g1.accept_alert().await.unwrap();
    let question_author = "Hater";
    g1.send_alert_text(question_author).await.unwrap();
    g1.accept_alert().await.unwrap();
    // ... and well - this toxic stuff also appears on the screen, but ...
    assert!(
        g1.expect_questions(QuestionState::Pending).await.unwrap()[0]
            .text()
            .await
            .unwrap()
            .to_lowercase()
            .contains(&question_text.to_lowercase())
    );

    // ------------------------ second guest window ----------------------
    // second guest sees the "question" ...
    g2.goto(guest_url.as_str()).await.unwrap();
    assert!(
        g2.expect_questions(QuestionState::Pending).await.unwrap()[0]
            .text()
            .await
            .unwrap()
            .to_lowercase()
            .contains(&question_text.to_lowercase())
    );

    // ------------------------ host window ----------------------------------
    // the host sees it and just decides to hide it: there is not much they
    // can do about it
    let pending_questions = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert!(pending_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    let hide_button = pending_questions[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
    hide_button.click().await.unwrap();

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
    let hidden_questions = h.expect_questions(QuestionState::Hidden).await.unwrap();
    assert_eq!(hidden_questions.len(), 1);
    assert!(hidden_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ---------------- first guest (not a gentle person) window -------------
    // just like in the "answer" scenario, let's wait until the changes
    // are sent to the server and synced back
    g1.wait_for_polling().await;
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
}

mod tests {
    crate::serial_test!(guest_asks_question_and_host_answers);
    crate::serial_test!(guest_asks_question_and_host_hides_it);
}
