<script>
	import Question from "./Question.svelte";
	import { votedFor, localAdjustments } from "./store.js";
	import { flip } from "svelte/animate";

	export let event;

	let inactive_hits = 0;
	function poll_time(e) {
		if (document.hidden) {
			// if the tab is hidden, no need to refresh so often
			// if it's been hidden for a while, even less so
			// we could stop refreshing altogether, and just
			// refresh when we're visible again (i.e., on
			// visibilitychange), but it's nice if things don't
			// jump around too much when that happens.
			inactive_hits += 1;
			if (inactive_hits <= 10 /* times 30s */) {
				// For the first 5 minutes, poll every 30s
				return 30 * 1000;
			} else if (inactive_hits <= 25 /* -10 times 60s */) {
				// For the next 15 minutes, poll every 60s
				return 60 * 1000;
			} else {
				// At this point, the user probably won't
				// return to the tab for a while, so we can
				// update _very_ rarely.
				return 20 * 60 * 1000;
			}
		}

		inactive_hits = 0;
		if (e.secret) {
			// hosts should get relatively frequent updates
			return 3000;
		} else {
			// guests can wait
			return 10000;
		}
	}

	function visibilitychange() {
		// immediately refresh when we become visible
		if (!document.hidden) {
			event = event;
		}
	}

	let interval;
	async function loadQuestions(e) {
		if (interval) {
			clearTimeout(interval);
		}
		let next = poll_time(e);
		console.info("refresh; next in", next, "ms");
		// set early so we'll retry even if request fails
		interval = setTimeout(() => {
			event = event;
		}, next);
		let url = e.secret
			? `/api/event/${e.id}/questions/${e.secret}`
			: `/api/event/${e.id}/questions`;
		let r = await fetch(url);
		if (!r.ok) {
			console.error(r);
			if (r.status >= 400 && r.status < 500) {
				// it's our fault. most likely, the event
				// doesn't exist (or has since been deleted),
				// but could also be that we have the wrong
				// secret. regardless, no point in retrying.
				if (interval) {
					clearTimeout(interval);
				}
			}
			throw r;
		}
		if (interval) {
			clearTimeout(interval);
		}
		// re-set timeout so we count from when the reload actually happened
		interval = setTimeout(() => {
			event = event;
		}, next);
		return await r.json();
	}

	// XXX: this ends up doing _two_ loads when the page initially opens
	//      not sure why...
	let rawQuestions;
	$: loadQuestions(event)
		.then((qs) => {
			rawQuestions = qs;
			problum = null;
		})
		.catch((r) => {
			if (r.status === 404) {
				rawQuestions = null;
				problum = r;
			} else {
				// leave questions and just highlight (hopefully
				// temporary) error.
				problum = r;
			}
		});

	// because of caching, we may receive a list of questions from the
	// server that doesn't reflect changes we've made (voting, asking new
	// questions, toggling hidden/answered). that's _very_ confusing.
	// so, we keep track of every change we make and re-apply it onto what
	// we get from the server until we observe the change in the server's
	// response.
	function adjustQuestions(rq, la, vf) {
		if (rq === null || rq === undefined) {
			return rq;
		}
		// deep-ish clone so we don't modify rawQuestions
		let qs = rq.map((q) => Object.assign({}, q));

		let nowPresent = {};
		for (const q of qs) {
			for (const newQ of la.newQuestions) {
				if (q.qid === newQ) {
					console.debug("no longer need to add", newQ);
					nowPresent[newQ] = true;
				}
			}
		}
		if (la.newQuestions.length > 0 || Object.keys(la.remap).length > 0) {
			console.log("question list needs local adjustments");
			let changed = Object.keys(nowPresent).length > 0;
			la.newQuestions = la.newQuestions.filter((qid) => !(qid in nowPresent));
			for (const newQ of la.newQuestions) {
				console.info("add in", newQ);
				qs.push({
					qid: newQ,
					hidden: false,
					votes: 1
				});
			}
			for (let i = 0; i < qs.length; i++) {
				let q = qs[i];
				let qid = q.qid;
				let adj = la.remap[qid];
				if (!adj) {
					continue;
				}
				console.debug("augment", qid);
				if ("hidden" in adj) {
					if (q.hidden === adj.hidden) {
						console.debug("no longer need to adjust hidden");
						delete la.remap[qid]["hidden"];
						changed = true;
					} else {
						console.info("adjust hidden to", adj.hidden);
						qs[i].hidden = adj.hidden;
					}
				}
				if ("answered" in adj) {
					// Ohhh, how I wish Javascript had a match statement like rust
					const patch = adj.answered;
					if (patch.action === "unset" && "answered" in q) {
						console.info("remove answered property");
						delete qs[i].answered;
					} else if (patch.action === "set" && !("answered" in q)) {
						console.info("adjust answered to", patch.value);
						qs[i].answered = patch.value;
					} else {
						console.debug("no longer need to adjust answered");
						delete la.remap[qid]["answered"];
						changed = true;
					}
				}
				if ("voted_when" in adj) {
					if (q.votes === adj.voted_when) {
						console.info("adjust vote count from", q.votes);
						// our vote likely isn't represented
						if (vf[qid]) {
							console.debug("adjust up");
							qs[i].votes += 1;
						} else {
							console.debug("adjust down");
							qs[i].votes -= 1;
						}
					} else {
						console.debug("vote count has been updated from", adj.voted_when, "to", q.votes);
						delete la.remap[qid]["voted_when"];
						changed = true;
					}
				}
				if (Object.keys(la.remap[qid]).length === 0) {
					console.debug("no more adjustments");
					delete la.remap[qid];
					changed = true;
				}
			}
			if (changed) {
				console.log("local adjustments changed");
				localAdjustments.set(la);
			}
		}
		qs.sort((a, b) => {
			return b.votes - a.votes;
		});
		return qs;
	}

	$: questions = adjustQuestions(rawQuestions, $localAdjustments, $votedFor);
	let problum;
	$: unanswered = (questions || []).filter((q) => !q.answered && !q.hidden);
	$: answered = (questions || [])
		.filter((q) => q.answered && !q.hidden)
		.sort((a, b) => a.answered - b.answered);
	$: hidden = (questions || []).filter((q) => q.hidden);

	async function ask() {
		let q;
		/* eslint-disable no-constant-condition */
		while (true) {
			q = prompt("Question:", q || "");
			if (q === null) {
				return;
			}
			if (q.match(/^\s*\S*\s*$/)) {
				alert("Use at least two words in your question.");
				continue;
			}
			break;
		}
		let who = prompt("Want to leave a signature? (optional)");
		if (!who || who.match(/^\s*$/)) {
			who = null;
		}
		// TODO: handle error
		let resp = await fetch(`/api/event/${event.id}`, {
			method: "POST",
			headers: {
				"Content-Type": "application/json"
			},
			body: JSON.stringify({
				body: q,
				asker: who
			})
		});
		let json = await resp.json();
		votedFor.update((vf) => {
			vf[json.id] = true;
			return vf;
		});
		localAdjustments.update((la) => {
			la.newQuestions.push(json.id);
			return la;
		});
	}

	let original_share_text = "Share event";
	let share_text = original_share_text;
	let reset;
	async function share(e) {
		let url = window.location + "";
		url = url.substring(0, url.length - event.secret.length - 1);
		await navigator.clipboard.writeText(url);
		e.target.textContent = "📋 Link copied!";
		if (reset) {
			clearTimeout(reset);
		}
		reset = setTimeout(() => {
			e.target.textContent = original_share_text;
		}, 1500);
	}
</script>

<svelte:window on:visibilitychange={visibilitychange} />

{#if questions}
	<div class="text-center">
		{#if event.secret}
			<button
				class="border-2 border-red-100 bg-orange-700 p-4 px-8 font-bold text-white hover:border-red-400"
				on:click={share}>{share_text}</button
			>
			<div class="pt-4 text-slate-400">
				The URL in your address bar shares the host view.<br />
				Use the button to get a shareable link to your clipboard.<br />
				Questions disappear after 30 days.
			</div>
		{:else}
			<button
				class="border-2 border-red-100 bg-orange-700 p-4 px-8 font-bold text-white hover:border-red-400"
				on:click={ask}>Ask another question</button
			>
		{/if}
	</div>

	{#if problum}
		<div class="fixed bottom-4 left-0 right-0">
			<p class="mx-auto max-w-4xl bg-red-500 px-4 py-2 font-bold text-white">
				{#if problum.status}
					Connection problems: {problum.status}
				{:else}
					Lost connection to the server&hellip; retrying.
				{/if}
			</p>
		</div>
	{/if}

	<section class="pt-4">
		{#if unanswered.length > 0}
			<div class="flex flex-col divide-y">
				{#each unanswered as question (question.qid)}
					<div animate:flip={{ duration: 500 }}>
						<Question {event} bind:question />
					</div>
				{/each}
			</div>
		{:else}
			<h2 class="my-8 text-center text-2xl text-slate-500">
				{#if answered.length > 0}
					No unanswered questions.
				{:else}
					No unanswered questions (yet).
				{/if}
			</h2>
		{/if}
	</section>
	{#if answered.length > 0}
		<section>
			<h2 class="mb-4 mt-8 text-center text-2xl text-green-700 dark:text-lime-500">
				Answered
				<span class="float-right text-lg"
					>({answered.length} / {answered.length + unanswered.length})</span
				>
			</h2>
			<div class="flex flex-col divide-y">
				{#each answered as question (question.qid)}
					<div animate:flip={{ duration: 500 }}>
						<Question {event} bind:question />
					</div>
				{/each}
			</div>
		</section>
	{/if}
	{#if event.secret && hidden.length > 0}
		<section>
			<h2 class="mb-4 mt-8 text-center text-2xl text-slate-400 dark:text-slate-500">Hidden</h2>
			<div class="flex flex-col divide-y">
				{#each hidden as question (question.qid)}
					<div animate:flip={{ duration: 500 }}>
						<Question {event} bind:question />
					</div>
				{/each}
			</div>
		</section>
	{/if}
{:else if problum}
	<div class="fixed bottom-4 left-0 right-0">
		<p class="mx-auto max-w-4xl bg-red-500 px-4 py-2 font-bold text-white">
			{#if !problum.status}
				Lost connection to the server&hellip; retrying.
			{:else if problum.status == 404}
				Event not found.
			{:else if problum.status == 401}
				Permission denied.
			{:else}
				The server is having issues; got {problum.status} {problum.statusText}.
			{/if}
		</p>
	</div>
{:else}
	<p>Loading questions...</p>
{/if}
