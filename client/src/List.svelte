<script>
	import { onMount } from "svelte";
	import Question from "./Question.svelte";
	import { votedFor } from './store.js';
	import { flip } from 'svelte/animate';

	export let event;

	let interval;
	async function loadEvents(e) {
		if (interval) {
			clearTimeout(interval);
		}
		interval = setTimeout(() => {event = event;}, 5000);
		let url = e.secret
			? `http://localhost:3000/event/${e.id}/${e.secret}`
			: `http://localhost:3000/event/${e.id}`;
		let r = await fetch(url);
		if (!r.ok) {
			console.error(r);
			throw r;
		}
		if (interval) {
			clearTimeout(interval);
		}
		interval = setTimeout(() => {event = event;}, 5000);
		return await r.json();
	}

	let questions;
	let problum;
	// XXX: this ends up doing _two_ loads when the page initially opens
	//      not sure why...
	$: loadEvents(event).then((qs) => {
		questions = qs;
		problum = null;
	}).catch((r) => {
		if (r.status === 404) {
			questions = null;
			problum = r;
		} else {
			// leave questions and just highlight (hopefully
			// temporary) error.
			problum = r;
		}
	});
	$: unanswered = (questions || []).filter((q) => !q.answered && !q.hidden)
	$: answered = (questions || []).filter((q) => q.answered && !q.hidden)
	$: hidden = (questions || []).filter((q) => !q.hidden)

	const resort = () => {
		questions = questions.sort((a, b) => { return b.votes - a.votes; });
	}

	async function ask() {
		let q = prompt("Question:");
		// TODO: handle "cancel", reject URL-only early, handle error
		let resp = await fetch(`http://localhost:3000/event/${event.id}`, {
			"method": "POST",
			"body": q,
		}).then(r => r.json());
		votedFor.update(vf => {
			vf[resp.id] = true;
			return vf;
		});
		event = event;
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

{#if questions}
	<div class="text-center">
	{#if event.secret}
		<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-100 hover:border-red-400" on:click={share}>{share_text}</button>
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
	<div class="flex flex-col divide-y">
	{#each unanswered as question (question.qid)}
		<div animate:flip="{{duration: 500}}">
		<Question {event} bind:question={question} {resort} />
		</div>
	{/each}
	</div>
	</section>
	<section>
	<h2 class="text-2xl text-center text-green-700 mt-8 mb-4">Answered</h2>
	<div class="flex flex-col divide-y">
	{#each answered as question (question.qid)}
		<div animate:flip="{{duration: 500}}">
		<Question {event} bind:question={question} {resort} />
		</div>
	{/each}
	</div>
	</section>
	{#if event.secret}
	<section>
	<h2 class="text-2xl text-center text-slate-400 mt-8 mb-4">Hidden</h2>
	<div class="flex flex-col divide-y">
	{#each hidden as question (question.qid)}
		<div animate:flip="{{duration: 500}}">
		<Question {event} bind:question={question} {resort} />
		</div>
	{/each}
	</div>
	</section>
	{/if}
{:else if problum}
	<div class="fixed bottom-4 left-0 right-0">
	<p class="max-w-4xl mx-auto bg-red-500 py-2 px-4 font-bold text-white">
	{#if !problum.status}
		Could not establish connection to the server.
	{:else if problum.status == 404}
		Event not found.
	{:else}
		The server is having issues; got {problum.status} status code.
	{/if}
	</p>
	</div>
{:else}
	<p>Loading questions...</p>
{/if}
