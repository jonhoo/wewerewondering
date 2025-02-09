<script>
	import { onMount } from "svelte";
	import { event } from "./store";
	import List from "./List.svelte";

	let problum = $state();

	async function popstate() {
		const path = window.location.pathname;

		if (path.startsWith("/event")) {
			const id = path.slice(7);
			let i = id.indexOf("/");
			let new_event;
			if (i !== -1) {
				new_event = {
					id: id.substring(0, i),
					secret: id.substring(i + 1)
				};
			} else {
				new_event = {
					id: id
				};
			}
			if (new_event != $event) {
				let r = await fetch(`/api/event/${new_event.id}`).catch((e) => {
					problum = e;
					// server is down, retry slowly
					setTimeout(popstate, 5000);
					throw e;
				});
				if (!r.ok) {
					if (r.status >= 400 && r.status < 500) {
						// our fault -- don't retry
						event.set(null);
					} else {
						// server is sad, retry slowly
						setTimeout(popstate, 5000);
					}
					problum = r;
					return;
				}
				problum = null;
				event.set(new_event);
			}
		} else {
			event.set(null);
		}
	}

	async function create() {
		let resp = await fetch(`/api/event`, {
			method: "POST"
		}).then((r) => r.json());
		// TODO: on failure
		history.pushState(resp, `Q&A ${resp.id} (host view)`, `/event/${resp.id}/${resp.secret}`);
		await popstate();
	}

	onMount(popstate);
</script>

<svelte:window onpopstate={popstate} />

{#if problum}
	<div class="fixed right-0 bottom-4 left-0">
		<p class="mx-auto max-w-4xl bg-red-500 px-4 py-2 font-bold text-white">
			{#if problum.status}
				{#if problum.status === 404}
					Event not found.
				{:else}
					The server is having issues; got {problum.status} {problum.statusText}.
				{/if}
			{:else}
				Lost connection to the server&hellip; retrying.
			{/if}
		</p>
	</div>
{/if}

{#if $event}
	<main class="mx-auto my-4 max-w-4xl px-4">
		<List />
		<div class="mt-4 text-center text-slate-400">
			Questions are ordered by <a
				class="hover:text-black"
				href="https://github.com/jonhoo/wewerewondering/blob/b87f660669d9323b7c2825d28b22c83792cd509e/server/src/list.rs#L192-L242"
				>votes over time</a
			>.
		</div>
		<div class="mt-4 text-center text-slate-400">
			( made on <a class="hover:text-black" href="https://github.com/jonhoo/wewerewondering"
				>github</a
			>
			by <a class="hover:text-black" href="https://thesquareplanet.com/">jonhoo</a>
			)
		</div>
	</main>
{:else}
	<div class="flex h-screen items-center justify-center">
		<button
			data-testid="create-event-button"
			class="border-2 border-red-500 bg-orange-700 p-4 px-8 font-bold text-white hover:border-red-400"
			onclick={create}>Open new Q&amp;A session</button
		>
	</div>
{/if}

<style global lang="postcss">
	@tailwind utilities;
	@tailwind components;
	@tailwind base;
</style>
