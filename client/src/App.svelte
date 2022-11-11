<script>
	import { onMount } from 'svelte';
	import List from './List.svelte';

	let event;
	let problum;

	async function hashchange() {
		// the poor man's router!
		const path = window.location.hash.slice(1);

		if (path.startsWith('/event')) {
			const id = path.slice(7);
			let i = id.indexOf("/");
			let new_event;
			if (i !== -1) {
				new_event = {
					"id": id.substring(0, i),
					"secret": id.substring(i + 1)
				};
			} else {
				new_event = {
					"id": id
				};
			}
			if (new_event != event) {
				let r = await fetch(`http://localhost:3000/event/${new_event.id}`).catch((e) => {
					problum = e;
					setTimeout(hashchange, 5000);
					throw e;
				});
				if (!r.ok) {
					if (r.status === 404) {
						event = null;
					} else {
						setTimeout(hashchange, 5000);
					}
					problum = r;
					return;
				}
				problum = null;
				event = new_event;
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

{#if problum}
	<div class="fixed bottom-4 left-0 right-0">
	<p class="max-w-4xl mx-auto bg-red-500 py-2 px-4 font-bold text-white">
	{#if problum.status}
		{#if problum.status === 404}
		Event not found.
		{:else}
		Connection problems: {problum.status}
		{/if}
	{:else}
		Lost connection to the server&hellip; retrying.
	{/if}
	</p>
	</div>
{/if}

{#if event}
	<main class="max-w-4xl mx-auto my-8 px-8">
		<List {event} />
	</main>
{:else}
	<div class="flex justify-center items-center h-screen">
		<button class="border p-4 px-8 bg-orange-700 text-white font-bold border-2 border-red-500 hover:border-red-400" on:click={create}>Create new event</button>
	</div>
{/if}
