use crate::utils::TestContext;
use fantoccini::Locator;

async fn guest_asks_question_and_others_vote(
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
    let question_text = "Are we web yet?";
    g1.send_alert_text(question_text).await.unwrap();
    g1.accept_alert().await.unwrap();
    let question_author = "Steve";
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
    // note that the question will have one vote by default, meaning we are
    // upvoting our own question by default, and ...
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
    // ... we cannot upvote it twice; well, there are actually ways to "blow up"
    // your question to make the host see it (tweaking the data in the local
    // storage and clearing it altogether, or opening another session on the same
    // device or simply opening the link on another device), but fair play is
    // not something that the app guarantees and our next assertion is more for
    // demo purposes and we will show later on that other guest can upvote this
    // question without any hacks rather within the normal app flow
    assert!(question
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .is_err());

    // -------------------------- host window --------------------------
    // host can see the newly added question and also that there is one vote
    // for this question (the default one from the question's author)
    assert_eq!(
        h.wait_for_element(Locator::Css("#pending-questions article"))
            .await
            .unwrap()
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

    // we are giving the front-end some time to poll for question details changes
    // from the back-end, in this case the number of votes have changed; one could
    // argue that for the upvoting client we are using optimistic update and so
    // increase the counter in their instance of application immediately while
    // performing the mutation on the background, which is true, but we want to
    // avoid flakiness plus it's not the optimistic update that we are testing here
    g2.wait_for_polling().await;
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

    // ------------------------ first guest window -----------------------
    assert_eq!(
        g1.wait_for_element(Locator::Css("#pending-questions article"))
            .await
            .unwrap()
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "2" // NB used to "1" for the asking guest
    );

    // -------------------------- host window --------------------------
    // host can now also see there are 2 votes for this question
    let question = h
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
        "2"
    );
    // and btw the host can also upvote this question (maybe for their future
    // self to remember which questions they wanted to answer - even if this
    // is not the most popular one)
    let upvote_button = question
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .unwrap();
    upvote_button.click().await.unwrap();
    g2.wait_for_polling().await;
    assert_eq!(
        question
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "3"
    );

    // let's verify that both guests can now see that the votes count is
    // 3 (three) for this question: what we have is that every member of this
    // session voted for this question
    // ------------------------ first guest window -----------------------
    assert_eq!(
        g1.wait_for_element(Locator::Css("#pending-questions article"))
            .await
            .unwrap()
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "3"
    );
    // ------------------------ second guest window -----------------------
    assert_eq!(
        g2.wait_for_element(Locator::Css("#pending-questions article"))
            .await
            .unwrap()
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "3"
    );
}

mod tests {
    crate::serial_test!(guest_asks_question_and_others_vote);
}
