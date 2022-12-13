<script>
	import Question from "./Question.svelte";
	import { rawQuestions, localAdjustments, votedFor,  questions, event } from './store.js';
    import { poll_time, animationTime } from "./helpers";
	import { flip } from 'svelte/animate';
	import { scale } from 'svelte/transition';


	function visibilitychange() {
		// immediately refresh when we become visible
		if (!document.hidden) {
			event.set($event);
		}
	}

	let interval;
	let problum;
	async function loadQuestions(e) {
		if (interval) {
			clearTimeout(interval);
		}
		let next = poll_time(e);
		console.info("refresh; next in", next, "ms");
		// set early so we'll retry even if request fails
		interval = setTimeout(() => {event.set(e)}, next);
		let url = e.secret
			? `/api/event/${e.id}/questions/${e.secret}`
			: `/api/event/${e.id}/questions`;
		let r = await fetch(url);
		if (!r.ok) {
			console.error(r);
            problum = r;
            if (r.status === 404) {
                rawQuestions.set(null);
            }
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
        problum = null;
		if (interval) {
			clearTimeout(interval);
		}
		// re-set timeout so we count from when the reload actually happened
		interval = setTimeout(() => {event.set(e)}, next);
		rawQuestions.set(await r.json());
	}
    event.subscribe(loadQuestions);

    // Not sure how the $: syntax works with stores, so to be on the safe side
    // i'm doing an explicit subscribe
    let unanswered = [];
	let answered = []; 
	let hidden = [];
    let last = new Date();
    questions.subscribe((qs) => {
        // Noop means no there are no new updates
        if (qs === "noop") return;
        let now = new Date();
        console.log(`Redraw request after: ${now.getTime() - last.getTime()}ms`);
        last = now;
        unanswered = qs.filter((q) => !q.answered && !q.hidden)
	    answered = qs.filter((q) => q.answered && !q.hidden)
	    hidden = qs.filter((q) => q.hidden)
    })
    $: disableLastOut = unanswered.length === 1

	
	async function ask() {
		let q;
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
		let resp = await fetch(`/api/event/${$event.id}`, {
			"method": "POST",
			"headers": {
				'Content-Type': 'application/json',
			},
			"body": JSON.stringify({
				"body": q,
				"asker": who,
			}),
		});
		let json = await resp.json();
		votedFor.update(vf => {
			vf[json.id] = true;
			return vf;
		});
		localAdjustments.update(la => {
			la.newQuestions.push(json.id);
			return la;
		});
	}

	let original_share_text = "Share event";
	let share_text = original_share_text;
	let reset;
	async function share(e) {
		let url = window.location + "";
		url = url.substring(0, url.length - $event.secret.length - 1);
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

<svelte:window on:visibilitychange={visibilitychange}/>

{#if questions}
	<div class="text-center">
	{#if $event.secret}
		<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-100 hover:border-red-400" on:click={share}>{share_text}</button>
		<div class="text-slate-400 pt-4">
			The URL in your address bar shares the host view.<br />
			Use the button to get a shareable link to your clipboard.<br />
			Questions disappear after 30 days.
		</div>
	{:else}
		<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-100 hover:border-red-400" on:click={ask}>Ask another question</button>
	{/if}
	</div>

	{#if problum}
	<div class="fixed bottom-4 left-0 right-0">
	<p class="max-w-4xl mx-auto bg-red-500 py-2 px-4 font-bold text-white">
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
		    <div animate:flip={{duration: animationTime}} {...disableLastOut ? {} : {"out:scale":{ duration: animationTime }}}>
			<Question bind:question={question} />
			</div>
		{/each}
		</div>
	{:else}
		<h2 class="text-center text-slate-500 text-2xl my-12" in:scale={{ duration: animationTime }}>
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
	<h2 class="text-2xl text-center text-green-700 dark:text-lime-500 mt-8 mb-4">Answered
		<span class="text-lg float-right">({answered.length} / {answered.length + unanswered.length})</span>
	</h2>
	<div class="flex flex-col divide-y">
	{#each answered as question (question.qid)}
		<div animate:flip={{duration: animationTime}} out:scale={{ duration: animationTime }}>
		<Question bind:question={question} />
		</div>
	{/each}
	</div>
	</section>
	{/if}
	{#if $event.secret && hidden.length > 0}
	<section>
	<h2 class="text-2xl text-center text-slate-400 dark:text-slate-500 mt-8 mb-4">Hidden</h2>
	<div class="flex flex-col divide-y">
	{#each hidden as question (question.qid)}
		<div animate:flip={{duration: animationTime}} out:scale={{ duration: animationTime }}>
		<Question bind:question={question} />
		</div>
	{/each}
	</div>
	</section>
	{/if}
{:else if problum}
	<div class="fixed bottom-4 left-0 right-0">
	<p class="max-w-4xl mx-auto bg-red-500 py-2 px-4 font-bold text-white">
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
