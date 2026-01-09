import test from "node:test";
import { sameQuestions } from "./utils.js";

test(`[${sameQuestions.name}]`, async (t) => {
  await t.test("defferent array lengths", (t) => {
    const same = sameQuestions(
      [],
      [{ hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 }]
    );
    t.assert.equal(same, false);
  });

  await t.test("different questions' ordering", (t) => {
    const same = sameQuestions(
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0 }
      ],
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0 },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 }
      ]
    );
    t.assert.equal(same, false);
  });

  await t.test("different sets of keys", (t) => {
    const same = sameQuestions(
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0 }
      ],
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0, answered: 1767956971 }
      ]
    );
    t.assert.equal(same, false);
  });

  await t.test("different sets of keys (absent vs undefined)", (t) => {
    const same = sameQuestions(
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        // key absent
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0 }
      ],
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        // key present, but value `undefined`
        // NB: we consider such questions to also be defferent
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0, answered: undefined }
      ]
    );
    t.assert.equal(same, false);
  });

  await t.test("different values", (t) => {
    const same = sameQuestions(
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 1 },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0, answered: 1767956971 }
      ],
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 2 }, // NB: somebody upvoted this question
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0, answered: 1767956971 }
      ]
    );
    t.assert.equal(same, false);
  });

  await t.test("keys order does not matter", (t) => {
    const same = sameQuestions(
      [
        { qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 2, hidden: false },
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6", votes: 0, answered: 1767956971 }
      ],
      // NB: questions order still matters, but the keys can go in any order
      [
        { hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 2 },
        { hidden: false, votes: 0, answered: 1767956971, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW6" }
      ]
    );
    t.assert.equal(same, true);
  });

  await t.test("sanity: both arrays empty", (t) => {
    const same = sameQuestions([], []);
    t.assert.equal(same, true);
  });

  await t.test("sanity: same lengths, ordering, keys (and keys ordering), and values", (t) => {
    const same = sameQuestions(
      [{ hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 2 }],
      [{ hidden: false, qid: "01KEH74QPHZ9R3KJPK4JJM1ZW5", votes: 2 }]
    );
    t.assert.equal(same, true);
  });
});
