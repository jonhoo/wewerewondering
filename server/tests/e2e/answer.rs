use crate::utils::TestContext;
use fantoccini::Locator;

async fn guest_asks_question_and_host_answers(TestContext { host: h, .. }: TestContext) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let guest_url = h.create_event().await;
    let host_window = h.window().await.unwrap();

    // initially no questions
    assert!(h.wait_for_pending_questions().await.unwrap().is_empty());

    // ------------------------ guest window ---------------------------------
    // guest opens the link and ...
    let guest_window = h.new_window(false).await.unwrap();
    h.switch_to_window(guest_window.handle.clone())
        .await
        .unwrap();
    h.goto(guest_url.as_str()).await.unwrap();
    // ... asks a question, which ...
    h.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    let question_text = "Did you attend Rust Forge conference in Wellington in 2025?";
    h.send_alert_text(question_text).await.unwrap();
    h.accept_alert().await.unwrap();
    let question_author = "Tim";
    h.send_alert_text(question_author).await.unwrap();
    h.accept_alert().await.unwrap();
    // ... appears on the screen
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions article .question__text"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    // sanity: and it's the only one pending question
    assert_eq!(h.wait_for_pending_questions().await.unwrap().len(), 1);

    // ------------------------ host window ----------------------------------
    // host sees the newly asked question
    h.switch_to_window(host_window.clone()).await.unwrap();
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    // sanity: the host's screen also shows one single question
    assert_eq!(h.wait_for_pending_questions().await.unwrap().len(), 1);

    // the host also observes two action options: they can either mark
    // the question as answered or hide it
    let question_article = h
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap();
    let answer_button = question_article
        .find(Locator::Css(r#"button[data-action="mark_answered"]"#))
        .await
        .unwrap();
    let _hide_button = question_article
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
    // the host asks the question elsewhere (e.g. during the live stream) and
    // marks the question as answered
    answer_button.click().await.unwrap();

    // the host observes text "No unanswered questions." which might be
    // satisfying, but also can be sad depends on the context and mood
    let no_questions_text = h
        .wait_for_element(Locator::Css("#pending-questions h2"))
        .await
        .unwrap();
    assert!(no_questions_text
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // and Tim's question travelled to the "answered" container:
    let answered_section = h
        .wait_for_element(Locator::Id("answered-questions"))
        .await
        .unwrap();
    let host_answered_questions = answered_section
        .find_all(Locator::Css("article"))
        .await
        .unwrap();
    assert_eq!(host_answered_questions.len(), 1);
    // sanity: let's check that this is actually Tim's question
    assert!(host_answered_questions[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ------------------------ guest window ---------------------------------
    // let's switch to Tim's window for a sec and check they also observe their
    // question having been answered
    h.switch_to_window(guest_window.handle).await.unwrap();
    let guest_pending_questions = h
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
    let guest_answered_section = h
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
    // ok let's hop back to the host window to see what actions they have
    // now available for the answered question
    h.switch_to_window(host_window).await.unwrap();
    // they can either mark the already answered question as unanswered, or ...
    let _unanswer_button = host_answered_questions[0]
        .find(Locator::Css(r#"button[data-action="mark_not_answered"]"#))
        .await
        .unwrap();
    // ... hide the question (just like they could do with that question
    // when it was not answered)
    let _hide_answered_button = host_answered_questions[0]
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
}

async fn guest_asks_question_and_host_hides_it(TestContext { host: h, .. }: TestContext) {
    // ------------------------ host window ----------------------------------
    // host creates a new event
    let guest_url = h.create_event().await;
    let host_window = h.window().await.unwrap();

    // initially no questions
    assert!(h.wait_for_pending_questions().await.unwrap().is_empty());

    // ------------------------ guest window ---------------------------------
    // guest opens the link and ...
    let guest_window = h.new_window(false).await.unwrap();
    h.switch_to_window(guest_window.handle.clone())
        .await
        .unwrap();
    h.goto(guest_url.as_str()).await.unwrap();
    // ... asks a question ... no, instead they write some toxic stuff
    h.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    let question_text = "I hate your streams and everything about you";
    h.send_alert_text(question_text).await.unwrap();
    h.accept_alert().await.unwrap();
    let question_author = "Hater";
    h.send_alert_text(question_author).await.unwrap();
    h.accept_alert().await.unwrap();
    // ... and well - this toxic stuff also appear on the screen, but ...
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions article .question__text"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));

    // ------------------------ host window ----------------------------------
    // the host sees it and just decides to hide it: there is not much they
    // can do about it
    h.switch_to_window(host_window.clone()).await.unwrap();
    assert!(h
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    let question_article = h
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap();
    let hide_button = question_article
        .find(Locator::Css(r#"button[data-action="hide"]"#))
        .await
        .unwrap();
    hide_button.click().await.unwrap();

    // the host observes text "No unanswered questions"
    let no_questions_text = h
        .wait_for_element(Locator::Css("#pending-questions h2"))
        .await
        .unwrap();
    assert!(no_questions_text
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // this "question" has been moved to a special container (not the one for
    // ansered questions)
    let host_hidden_section = h
        .wait_for_element(Locator::Id("hidden-questions"))
        .await
        .unwrap();
    let guest_answered_questions = host_hidden_section
        .find_all(Locator::Css("article"))
        .await
        .unwrap();
    assert_eq!(guest_answered_questions.len(), 1);

    // ------------------------ guest window ---------------------------------
    // in the guest's window there are now no unanswered questions either
    h.switch_to_window(guest_window.handle).await.unwrap();
    let guest_pending_questions = h
        .wait_for_element(Locator::Css("#pending-questions"))
        .await
        .unwrap();
    assert!(guest_pending_questions
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains("no unanswered questions"));
    // actually no questions at all:
    assert!(h
        .wait_for_element(Locator::Id("answered-questions"))
        .await
        .is_err());
    assert!(h
        // NB: the host would see these questions as tested above
        .wait_for_element(Locator::Id("hidden-questions"))
        .await
        .is_err());
}

mod tests {
    crate::serial_test!(guest_asks_question_and_host_answers);
    crate::serial_test!(guest_asks_question_and_host_hides_it);
}
