<script>
	import { onMount } from "svelte";
	import Question from "./Question.svelte";
	import { votedFor } from './store.js';

	export let event;

	let questions;
	let interval;
	async function refresh(e) {
		if (interval) {
			clearTimeout(interval);
		}
		// TODO: on error. esp 404 == show "no such event"
		let url = e.secret
			? `http://localhost:3000/event/${e.id}/${e.secret}`
			: `http://localhost:3000/event/${e.id}`;
		let r = await fetch(url);
		questions = await r.json();
		interval = setTimeout(() => {event = event;}, 5000);
	}
	$: refresh(event);

	async function ask() {
		let q = prompt("Question:");
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

<div class="text-center">
{#if event.secret}
	<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-100 hover:border-red-400" on:click={share}>{share_text}</button>
{:else}
	<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-100 hover:border-red-400" on:click={ask}>Ask another question</button>
{/if}
</div>

{#if questions}
	<section class="pt-4">
	<div class="flex flex-col divide-y">
	{#each questions as question}
		{#if question.hidden}
		{:else if question.answered}
		{:else}
			<Question {event} bind:question={question}/>
		{/if}
	{/each}
	</div>
	</section>
	<section>
	<h2 class="text-2xl text-center text-green-700 mt-8 mb-4">Answered</h2>
	<div class="flex flex-col divide-y">
	{#each questions as question}
		{#if question.hidden}
		{:else if question.answered}
			<Question {event} bind:question={question}/>
		{:else}
		{/if}
	{/each}
	</div>
	</section>
	{#if event.secret}
	<section>
	<h2 class="text-2xl text-center text-slate-400 mt-8 mb-4">Hidden</h2>
	<div class="flex flex-col divide-y">
	{#each questions as question}
		{#if question.hidden}
			<Question {event} bind:question={question}/>
		{:else if question.answered}
		{:else}
		{/if}
	{/each}
	</div>
	</section>
	{/if}
{:else}
	<p>loading...</p>
{/if}
