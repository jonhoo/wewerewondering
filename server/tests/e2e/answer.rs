use crate::utils::TestContext;

async fn guest_asks_question_and_host_answers(TestContext { client: c, dynamo }: TestContext) {
    // TODO
}

async fn guest_asks_question_and_host_hides_it(TestContext { client: c, dynamo }: TestContext) {
    // TODO
}

mod tests {
    crate::serial_test!(guest_asks_question_and_host_answers);
    crate::serial_test!(guest_asks_question_and_host_hides_it);
}
