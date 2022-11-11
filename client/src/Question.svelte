<script>
	import { onMount } from 'svelte';
	import {votedFor, questionCache, questionData} from './store.js';

	export let question;
	export let event;
	export let resort;

	let now = new Date();
	onMount(() => {
		const interval = setInterval(() => {
			now = new Date();
		}, 3000);

		return () => {
			clearInterval(interval);
		};
	});

	$: liked = question.qid in $votedFor;
	$: q = questionData(question.qid, $questionCache);

	async function vote() {
		let dir;
		if (liked) {
			dir = "down";
		} else {
			dir = "up";
		}
		let resp = await fetch(`http://localhost:3000/vote/${question.qid}/${dir}`, {
			"method": "POST",
		}).then(r => r.json());
		votedFor.update(vf => {
			if (liked) {
				delete vf[question.qid];
			} else {
				vf[question.qid] = true;
			}
			return vf;
		});
		question.votes = resp.votes;
		question = question;

		resort();
	}

	async function toggle(what) {
		await fetch(`http://localhost:3000/event/${event.id}/${event.secret}/${question.qid}/toggle/${what}`, {
			"method": "POST",
		});
		question[what] = !question[what];
		question = question;
	}

	async function answered() {
		toggle("answered");
	}
	async function hidden() {
		toggle("hidden");
	}

	function qclass(q) {
		if (q.hidden && q.answered) {
			return "p-4 bg-white text-lime-500";
		} else if (q.hidden) {
			return "p-4 bg-white text-slate-400";
		} else if (q.answered) {
			return "p-4 bg-white text-green-700";
		} else {
			return "p-4 bg-white";
		}
	}

	function since(q, now) {
		let when = new Date(q.when * 1000);
		let dur = (now - when) / 1000;
		if (dur > 24 * 60 * 60) {
			return parseInt(dur / (24 * 60 * 60)) + "d ago";
		} else if (dur > 60*60) {
			return parseInt(dur / (60 * 60)) + "h ago";
		} else if (dur > 60) {
			return parseInt(dur / 60) + "m ago";
		} else if (dur < 10) {
			return "just now";
		} else {
			return parseInt(dur) + "s";
		}
	}
</script>

<article class={qclass(question)}>
	<div class="flex items-center">
	<div class="mr-4 w-8 grow-0 shrink-0 text-center">
		{#if liked}
		<button class="hover:opacity-50" title="Retract vote" on:click={vote}>▲</button>
		{:else}
		<button class="opacity-30 hover:opacity-100" title="Vote" on:click={vote}>△</button>
		{/if}
		<div class="font-bold text-black">{question.votes}</div>
	</div>
	<div class="pr-4 flex-1">
		{#await q}
		<p class="text-xl">loading...</p>
		{:then q}
		<p class="text-xl">{q.text}</p>
		<div class="text-slate-400 pt-1 text-right">
		<span>{since(q, now)}</span>
		{#if event.secret}
			—
			{#if question.answered}
				<button on:click={answered}>Mark as not answered</button>
			{:else}
				<button on:click={answered}>Mark as answered</button>
			{/if}
			|
			{#if question.hidden}
				<button on:click={hidden}>Unhide</button>
			{:else}
				<button on:click={hidden}>Hide</button>
			{/if}
		{/if}
		</div>
		{/await}
	</div>
	</div>
</article>
