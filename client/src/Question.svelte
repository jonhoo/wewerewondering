<script>
	import {votedFor, questionTexts} from './store.js';

	export let question;
	export let event;

	async function questionText(question, qs) {
		if (qs[question.qid]) {
			return qs[question.qid];
		}

        // TODO: rate-limit how many we do of these at once
        //       or at least batch the initial fetch.
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
		<p class="text-xl">{text}</p>
		{#if event.secret}
			<div class="text-slate-400 pt-1 text-right">
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
			</div>
		{/if}
	</div>
	</div>
</article>
