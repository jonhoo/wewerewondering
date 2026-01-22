<script>
	import { onMount } from "svelte";
	import { installIntersectionObserver } from "./utils";

	/**
	 * @typedef Props
	 * @property {boolean} paused
	 * @property {import("svelte/elements").EventHandler} onclick
	 * @property {(isIntersecting: boolean) => void} onviewportintersection
	 */

	/** @type {Props} */
	let { onviewportintersection, onclick, paused } = $props();

	onMount(() => {
		const uninstall = installIntersectionObserver(
			"#toggle-updates-button",
			([{ isIntersecting }]) => onviewportintersection(isIntersecting)
		);
		return () => uninstall?.();
	});
</script>

<button
	id="toggle-updates-button"
	class="cursor-pointer text-slate-300 underline hover:text-slate-400"
	{onclick}
>
	{paused ? "Resume" : "Pause"} Updates
</button>
