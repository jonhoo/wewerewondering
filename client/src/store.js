import { writable } from "svelte/store";

export const event = writable(null);

const storedVotedFor = JSON.parse(localStorage.getItem("votedFor"));
export const votedFor = writable(!storedVotedFor ? {} : storedVotedFor);
votedFor.subscribe((value) => {
	if (value) {
		localStorage.setItem("votedFor", JSON.stringify(value));
	} else {
		localStorage.removeItem("votedFor");
	}
});

const storedLocalAdjustments = JSON.parse(localStorage.getItem("localAdjustments"));
export const localAdjustments = writable(
	!storedLocalAdjustments
		? {
				newQuestions: [
					// qid
				],
				remap: {
					// qid => {
					//   hidden: bool,
					//   answered: {action: "unset"} | {action: "set", value: number},
					//   voted_when: int
					// }
				}
			}
		: storedLocalAdjustments
);
localAdjustments.subscribe((value) => {
	if (value) {
		localStorage.setItem("localAdjustments", JSON.stringify(value));
	} else {
		localStorage.removeItem("localAdjustments");
	}
});

const storedQs = JSON.parse(localStorage.getItem("questions"));
export const questionCache = writable(!storedQs ? {} : storedQs);
questionCache.subscribe((value) => {
	if (value) {
		localStorage.setItem("questions", JSON.stringify(value));
	} else {
		localStorage.removeItem("questions");
	}
});

let batch = {};
let fetching = {};
let fetch_done;
export async function questionData(qid, qs) {
	if (qs[qid]) {
		console.debug("already in cache", qid);
		return qs[qid];
	}

	if (fetching[qid]) {
		console.debug("already fetching", qid);
		return await fetching[qid][0];
	}

	if (batch[qid]) {
		console.debug("already batched", qid);
		return await batch[qid][0];
	}

	console.debug("adding to batch", qid);

	let resolve_p;
	let reject_p;
	let promise = new Promise((resolve1, reject1) => {
		resolve_p = resolve1;
		reject_p = reject1;
	});
	let first = Object.keys(batch).length === 0;
	batch[qid] = [promise, resolve_p, reject_p];

	if (Object.keys(fetching).length > 0) {
		if (first) {
			// it's our job to do the next fetch
			console.debug("fetch already ongoing, need to wait then fetch");
			await fetch_done;
		} else {
			console.debug("fetch already ongoing, just need to wait");
			return await promise;
		}
	} else if (first) {
		console.info("single-question batch; waiting 50ms");
		fetching = { non: "empty", overridden: "below" };
		await new Promise((resolve) => {
			setTimeout(resolve, 50);
		});
	}

	console.info("fetching batch", qid, Object.keys(batch).length);

	// give the next batch a way to wait for us to complete
	let resolve;
	let _reject;
	fetch_done = new Promise((resolve1, reject1) => {
		resolve = resolve1;
		_reject = reject1;
	});

	while (true) {
		// make the current batch of qids (and their promises).
		fetching = batch;
		batch = {};
		// sort to improve cache hit rate
		let qids = Object.entries(fetching).map(([qid]) => qid);
		// dynamodb can fetch at most 100 keys, and at most 16MB,
		// whichever is smaller. for 16MB to be smaller, entries would
		// need to be >160k. we project id, text, who, and when. text
		// and who are the only free-form fields, and they're
		// collectively limited to 1k by the max request body size, so
		// 100 will always be limit we care about. we want to make sure
		// we don't _send_ more than 100 keys, because then the whole
		// query will fail.
		//
		// separately, by keeping batches smaller, we increase the
		// chances of cache hits becaues it's more likely two clients
		// will request the same set of questions. so, we pick a number
		// that's small-ish.
		//
		// ref https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_BatchGetItem.html
		qids.sort();
		if (qids.length > 25) {
			qids = qids.slice(0, 25);
		}
		let arg = qids.join(",");
		// and go!
		// TODO: handle failure
		let data = await fetch(`/api/questions/${arg}`);
		let json = await data.json();
		// store back to cache
		questionCache.update((qs) => {
			for (const [qid, q] of Object.entries(json)) {
				qs[qid] = q;
			}
			return qs;
		});
		// resolve anyone who's waiting
		let added_back_to_batch = 0;
		for (const [qid, [pr, res, rej]] of Object.entries(fetching)) {
			// dynamodb does not guarantee that we get responses for all
			// the keys, so we need to let them leak over into the next
			// batch.
			if (qid in json) {
				res(json[qid]);
			} else {
				added_back_to_batch += 1;
				batch[qid] = [pr, res, rej];
			}
		}
		fetching = {};
		if (added_back_to_batch > 0 && Object.keys(batch).length === added_back_to_batch) {
			// we created a new batch and there's no-one else to
			// pick it up, so it's on us.
			continue;
		}
		// clear next batch to go
		resolve(true);
		break;
	}

	return await promise;
}

window.addEventListener("storage", (e) => {
	if (e.key == "votedFor") {
		votedFor.set(JSON.parse(e.newValue));
	} else if (e.key == "questions") {
		questionCache.set(JSON.parse(e.newValue));
	} else if (e.key == "localAdjustments") {
		localAdjustments.set(JSON.parse(e.newValue));
	}
});
