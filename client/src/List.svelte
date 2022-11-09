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
		// TODO: on error
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

</script>

<button on:click={ask}>Ask a new question</button>

{#if questions}
	<h2>Unanswered</h2>
	{#each questions as question}
		{#if question.hidden}
		{:else if question.answered}
		{:else}
			<Question {event} bind:question={question}/>
		{/if}
	{/each}
	<h2>Answered</h2>
	{#each questions as question}
		{#if question.hidden}
		{:else if question.answered}
			<Question {event} bind:question={question}/>
		{:else}
		{/if}
	{/each}
	{#if event.secret}
	<h2>Hidden</h2>
	{#each questions as question}
		{#if question.hidden}
			<Question {event} bind:question={question}/>
		{:else if question.answered}
		{:else}
		{/if}
	{/each}
	{/if}
{:else}
	<p class="loading">loading...</p>
{/if}

<style>
	a {
		padding: 2em;
		display: block;
	}

	.loading {
		opacity: 0;
		animation: 0.4s 0.8s forwards fade-in;
	}

	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}
</style>
