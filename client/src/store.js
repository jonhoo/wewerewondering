import { writable } from "svelte/store";

const storedVotedFor = JSON.parse(localStorage.getItem("votedFor"));
export const votedFor = writable(!storedVotedFor ? {} : storedVotedFor);
votedFor.subscribe(value => {
    if (value) {
        localStorage.setItem("votedFor", JSON.stringify(value));
    } else {
        localStorage.removeItem("votedFor");
    }
});

const storedQs = JSON.parse(localStorage.getItem("questions"));
export const questionCache = writable(!storedQs ? {} : storedQs);
questionCache.subscribe(value => {
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
		fetching = {"non": "empty", "overridden": "below"};
		await new Promise(resolve => {
			setTimeout(resolve, 50);
		});
	}

	console.info("fetching batch", qid, Object.keys(batch).length);

	// give the next batch a way to wait for us to complete
	let resolve;
	let reject;
	fetch_done = new Promise((resolve1, reject1) => {
		resolve = resolve1;
		reject = reject1;
	});

	// :ake the current batch of qids (and their promises).
	fetching = batch;
	batch = {};
	// sort to improve cache hit rate
	let qids = Object.entries(fetching).map(([qid, resolve, reject]) => qid);
	qids.sort();
	let arg = qids.join(",");
	// and go!
	let data = await fetch(`/api/questions/${arg}`);
	let json = await data.json();
	// store back to cache
	questionCache.update(qs => {
		for (const [qid, q] of Object.entries(json)) {
			qs[qid] = q;
		}
		return qs;
	});
	// resolve anyone who's waiting
	for (const [qid, [_, resolve, reject]] of Object.entries(fetching)) {
		resolve(json[qid]);
	}
	// and clear next batch to go
	fetching = {};
	resolve(true);

	return await promise;
}

window.addEventListener('storage', (e) => {
	if (e.key == "votedFor") {
		votedFor.set(JSON.parse(e.newValue));
	} else if (e.key == "questions") {
		questionCache.set(JSON.parse(e.newValue));
	}
});
