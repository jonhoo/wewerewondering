<script>
	import {votedFor, questionTexts} from './store.js';

	export let question;
	export let event;

	async function questionText(question, qs) {
		if (qs[question.qid]) {
			return qs[question.qid];
		}

		let t = await fetch(`http://localhost:3000/question/${question.qid}`)
			.then(r => r.text())
			.then(data => {
				questionTexts.update(qs => {
					qs[question.qid] = data;
					return qs;
				});
				return data;
			});
		return t;
	}

	$: liked = question.qid in $votedFor;

	let text;
	$: questionText(question, $questionTexts).then(data => text = data);

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
</script>

<article>
	<p>{text} {question.votes}</p>
	{#if liked}
		<button on:click={vote}>Unvote</button>
	{:else}
		<button on:click={vote}>Vote</button>
	{/if}
	{#if event.secret}
		{#if question.answered}
			<button on:click={answered}>Answered</button>
		{:else}
			<button on:click={answered}>Mark answered</button>
		{/if}
		{#if question.hidden}
			<button on:click={hidden}>Hidden</button>
		{:else}
			<button on:click={hidden}>Mark hidden</button>
		{/if}
	{:else}
		{#if question.answered}
			Answered
		{/if}
	{/if}
</article>

<style>
	article {
		position: relative;
		padding: 0 0 0 2em;
		border-bottom: 1px solid #eee;
	}

	h2 {
		font-size: 1em;
		margin: 0.5em 0;
	}

	span {
		position: absolute;
		left: 0;
	}

	a {
		color: #333;
	}
</style>
