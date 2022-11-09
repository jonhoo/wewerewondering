<script>
	import { onMount } from 'svelte';
	import List from './List.svelte';

	let event;

	async function hashchange() {
		// the poor man's router!
		const path = window.location.hash.slice(1);

		if (path.startsWith('/event')) {
			const id = path.slice(7);
			let i = id.indexOf("/");
			if (i !== -1) {
				event = {
					"id": id.substring(0, i),
					"secret": id.substring(i + 1)
				};
			} else {
				event = {
					"id": id
				};
			}
			window.scrollTo(0,0);
		} else {
			event = null;
		}
	}

	async function create() {
		let resp = await fetch(`http://localhost:3000/event`, {
			"method": "POST",
		}).then(r => r.json());
		window.location.hash = `/event/${resp.id}/${resp.secret}`;
	}

	onMount(hashchange);
</script>

<svelte:window on:hashchange={hashchange}/>

<main>
	{#if event}
		<List {event} />
	{:else}
		<button on:click={create}>Create new event</button>
	{/if}
</main>

<style>
	main {
		position: relative;
		max-width: 800px;
		margin: 0 auto;
		min-height: 101vh;
		padding: 1em;
	}

	main :global(.meta) {
		color: #999;
		font-size: 12px;
		margin: 0 0 1em 0;
	}

	main :global(a) {
		color: rgb(0,0,150);
	}
</style>
