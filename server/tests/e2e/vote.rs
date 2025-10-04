use crate::utils::{QuestionState, TestContext};
use fantoccini::Locator;

async fn guest_asks_question_and_others_vote(
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
    assert!(h.await_questions(QuestionState::Pending).await.is_empty());

    // -------------------------- database -----------------------------------
    // sanity check: we do not have any questions for this event in db
    assert_eq!(d.event_questions(&eid).await.unwrap().count, 0);

    // ------------------------ first guest window -----------------------
    // first guest opens the link and asks a question, which ...
    g1.goto(url.as_str()).await.unwrap();
    let (qtext, qauthor) = ("Are we web yet?", "Steve");
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

    // note that the question will have one vote by default, meaning we are
    // upvoting our own question by default, and ...
    assert_eq!(
        pending[0]
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
    assert!(pending[0]
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .is_err());

    // -------------------------- host window --------------------------
    // host can see the newly added question and also that there is one vote
    // for this question (the default one from the question's author)
    assert_eq!(
        h.expect_questions(QuestionState::Pending).await.unwrap()[0]
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
    g2.goto(url.as_str()).await.unwrap();
    let pending = g2.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(pending[0]
        .text()
        .await
        .unwrap()
        .to_lowercase()
        .contains(&qtext.to_lowercase()));
    assert_eq!(
        pending[0]
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "1"
    );

    // .. they upvote it
    pending[0]
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // we are giving the front-end some time to poll for question details changes
    // from the back-end, in this case the number of votes have changed; one could
    // argue that for the upvoting client we are using optimistic update and so
    // increase the counter in their instance of application immediately while
    // performing the mutation on the background, which is true, but we want to
    // avoid flakiness plus it's not the optimistic update that we are testing here
    g2.wait_for_polling().await;
    assert_eq!(
        pending[0]
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
        g1.expect_questions(QuestionState::Pending).await.unwrap()[0]
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
    let pending = h.expect_questions(QuestionState::Pending).await.unwrap();
    assert_eq!(
        pending[0]
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
    pending[0]
        .find(Locator::Css(r#"button[data-action="upvote"]"#))
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    g2.wait_for_polling().await;
    assert_eq!(
        pending[0]
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
        g1.expect_questions(QuestionState::Pending).await.unwrap()[0]
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
        g2.expect_questions(QuestionState::Pending).await.unwrap()[0]
            .find(Locator::Css("[data-votes]"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "3"
    );

    // --------------------------- database ----------------------------------
    let questions = d.event_questions(eid).await.unwrap();
    assert_eq!(questions.count, 1);
    let qid = questions.items().first().unwrap().get("id").unwrap();
    let q = d.question_by_id(qid.to_owned()).await.unwrap();
    let q = q.item().unwrap();
    assert!(!q.get("hidden").unwrap().as_bool().unwrap());
    assert!(q.get("answered").is_none());
    assert_eq!(q.get("who").unwrap().as_s().unwrap(), qauthor);
    assert_eq!(q.get("text").unwrap().as_s().unwrap(), qtext);
    assert_eq!(q.get("votes").unwrap().as_n().unwrap(), "3"); // NB
}

mod tests {
    crate::serial_test!(guest_asks_question_and_others_vote);
}
