use crate::utils::TestContext;
use fantoccini::Locator;

async fn guest_asks_question_and_another_guest_upvotes_it(
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
    assert!(h.wait_for_pending_questions().await.unwrap().is_empty());

    // ------------------------ first guest window -----------------------
    // first guest opens the link and ...
    g1.goto(guest_url.as_str()).await.unwrap();
    // ... asks a question, which ...
    g1.wait_for_element(Locator::Id("ask-question-button"))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    let question_text = "What are your thoughts on memory safety in systems programming?";
    g1.send_alert_text(question_text).await.unwrap();
    g1.accept_alert().await.unwrap();
    let question_author = "Sarah";
    g1.send_alert_text(question_author).await.unwrap();
    g1.accept_alert().await.unwrap();
    // ... appears on the screen
    assert!(g1
        .wait_for_element(Locator::Css("#pending-questions article .question__text"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    // we now know that question details have all been fetched
    let question = g1
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap();
    assert_eq!(
        question
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "1"
    );

    // ------------------------ second guest window ----------------------
    // second guest sees the newly asked question and ...
    g2.goto(guest_url.as_str()).await.unwrap();
    let question = g2
        .wait_for_element(Locator::Css("#pending-questions article"))
        .await
        .unwrap();
    let vote_count = question
        .find(Locator::Css("[data-votes]"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(vote_count.contains("1"));
    assert!(question
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&question_text.to_lowercase()));
    // .. they upvote it
    let upvote_button = question
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .unwrap();
    upvote_button.click().await.unwrap();
    assert_eq!(
        question
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "2"
    );
}

mod tests {
    crate::serial_test!(guest_asks_question_and_another_guest_upvotes_it);
}
