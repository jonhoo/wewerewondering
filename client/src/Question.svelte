<script>
	import { onMount } from "svelte";
	import { votedFor, questionCache, questionData, localAdjustments } from "./store.js";

	export let question;
	export let event;

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
		// TODO: Handle API response in case of concurrent operations resulting
		// in vote direction opposite of client expectation
		await fetch(`/api/vote/${question.qid}/${dir}`, {
			method: "POST"
		}).then((r) => r.json());
		votedFor.update((vf) => {
			if (liked) {
				delete vf[question.qid];
			} else {
				vf[question.qid] = true;
			}
			return vf;
		});
		localAdjustments.update((la) => {
			let q = la.remap[question.qid] || {};
			q["voted_when"] = question.votes;
			la.remap[question.qid] = q;
			return la;
		});
		// NOTE: we don't update the vote count here because it would
		// mean we'd have an updated count for this question, but not
		// for any others.
		// question.votes = resp.votes;
	}

	async function toggle(what) {
		const res = await fetch(
			`/api/event/${event.id}/questions/${event.secret}/${question.qid}/toggle/${what}`,
			{
				method: "POST",
				body: question[what] ? "off" : "on"
			}
		).then((r) => r.json());
		localAdjustments.update((la) => {
			let q = la.remap[question.qid] || {};
			if (what === "answered") {
				q[what] = what in res ? { action: "set", value: res[what] } : { action: "unset" };
			} else {
				q[what] = res[what];
			}
			la.remap[question.qid] = q;
			return la;
		});
	}

	async function answered() {
		toggle("answered");
	}
	async function hidden() {
		toggle("hidden");
	}

	function qclass(q) {
		if (q.hidden && q.answered) {
			return "p-4 bg-white dark:bg-slate-800 text-lime-500 dark:text-green-700";
		} else if (q.hidden) {
			return "p-4 bg-white dark:bg-slate-800 text-slate-400 dark:text-slate-500";
		} else if (q.answered) {
			return "p-4 bg-white dark:bg-slate-800 text-green-700 dark:text-lime-500";
		} else {
			return "p-4 bg-white dark:bg-slate-800 dark:text-slate-300";
		}
	}

	function since(q, now) {
		let when = new Date(q.when * 1000);
		let dur = (now - when) / 1000;
		if (dur > 24 * 60 * 60) {
			return parseInt(dur / (24 * 60 * 60)) + "d ago";
		} else if (dur > 60 * 60) {
			return parseInt(dur / (60 * 60)) + "h ago";
		} else if (dur > 60) {
			return parseInt(dur / 60) + "m ago";
		} else if (dur < 10) {
			return "just now";
		} else {
			return parseInt(dur) + "s ago";
		}
	}
</script>

<article class={qclass(question)}>
	<div class="flex items-center">
		<div class="mr-4 w-8 shrink-0 grow-0 text-center">
			{#if liked}
				<button class="hover:opacity-50" title="Retract vote" on:click={vote}>▲</button>
			{:else}
				<button class="opacity-30 hover:opacity-100" title="Vote" on:click={vote}>△</button>
			{/if}
			<div class="font-bold text-black dark:text-slate-300">{question.votes}</div>
		</div>
		<div class="flex-1 pr-4">
			{#await q}
				<p class="text-xl">loading...</p>
			{:then q}
				<p class="text-xl">{q.text}</p>
				<div class="pt-1 text-right text-slate-400">
					<span>{since(q, now)}</span>
					{#if q.who}
						<span>by {q.who}</span>
					{/if}
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
