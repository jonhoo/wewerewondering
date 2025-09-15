import { writable } from "svelte/store";

export const event = writable(null);
export const votedFor = writable(null);
export const localAdjustments = writable(null);
export const questionCache = writable(null);

/**
 * Initialize event store.
 *
 * Given the `eid`, this will costruct a local storage key for the current
 * event (e.g. `event::01K542Z5KKR8YJ5DX9GQN7VV1S` for guest whereas for host -
 * `event::01K542Z5KKR8YJ5DX9GQN7VV1S/mSgwnC12sDhlNLzzej38ClSrSWWYfN`) and read
 * existing event data (if any) from disk into the app's memory.
 *
 * Internally, we are also creating subscribtions per slice (e.g. `votedFor`,
 * `questions`, `localAdjustments`) and persisting any mutations of those slices
 * back onto disk.
 *
 * The event data has got the following shape:
 * ```json
 *  {
 *    "votedFor": {},
 *    "localAdjustments": { "newQuestions":[],"remap":{}},
 *    "questions": {
 *      "01K542ZQASKKGEEXV696D3X515":{"text":"new session","when":1757852720}
 *    }
 *  }
 * ```
 *
 * @param {string} eid
 */
export function initEventStore(eid) {
	const storedEventDataKey = `event::${eid}`;

	/**
	 * @param {import("svelte/store").Writable} storeSlice
	 * @param {"votedFor" | "localAdjustments" | "questions"} eventDataKey
	 */
	function subscribe(storeSlice, eventDataKey) {
		storeSlice.subscribe((value) => {
			const data = JSON.parse(localStorage.getItem(storedEventDataKey)) ?? {};
			if (value) {
				data[eventDataKey] = value;
			} else {
				data[eventDataKey] = undefined;
			}
			localStorage.setItem(storedEventDataKey, JSON.stringify(data));
		});
	}

	const storedEventData = JSON.parse(localStorage.getItem(storedEventDataKey)) ?? {};
	votedFor.set(storedEventData.votedFor ?? {});
	subscribe(votedFor, "votedFor");
	localAdjustments.set(
		storedEventData.localAdjustments ?? {
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
	);
	subscribe(localAdjustments, "localAdjustments");
	questionCache.set(storedEventData.questions ?? {});
	subscribe(questionCache, "questions");
}

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
