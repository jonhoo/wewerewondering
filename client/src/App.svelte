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

<style global lang="postcss">
@tailwind utilities;
@tailwind components;
@tailwind base;
</style>

<svelte:window on:hashchange={hashchange}/>

{#if event}
	<main class="max-w-4xl mx-auto my-8 px-8">
		<List {event} />
	</main>
{:else}
	<div class="flex justify-center items-center h-screen">
		<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-500 hover:border-red-400" on:click={create}>Create new event</button>
	</div>
{/if}
