/**
 * Install an intersection observer for `target` element.
 *
 * If `target` is a string, it is considered to be a CSS selector, otherwise
 * the utility treats it as the target element. If the element is not found,
 * this will return `null`, otherwise an observer is being created with the
 * provied `callback` and `options`, and set to observe the `target` element.
 * The utility will return a cleanup function calling which will unobserve
 * the target element and let the observer itself to be garbage collected.
 *
 *
 * If no options get passed, intersection with the device's viewport will be
 * being observed and the `callback` will be fired as soon as the `target`
 * element touches the viewport's boundary even if pixels of the target element
 * are _not_ visible just yet. So if you want to run some logic when half of
 * the element's bounding rectangle gets within the viewport, you can go with:
 *
 * ```js
 * const unobserveFn = installIntersectionObserver(
 *  "#target-elem-id",
 *  (_entries, _observer) => { console.log("half of the target in viewport"); },
 *  { threshold: 0.5 }
 * );
 * if (unobserveFn === null) { ... }
 * ```
 *
 * @param {string | HTMLElement} target
 * @param {IntersectionObserverCallback} callback
 * @param {IntersectionObserverInit | undefined} options
 * @returns {() => void | null}
 */
export function installIntersectionObserver(target, callback, options = undefined) {
	const targetElem = typeof target === "string" ? document.querySelector(target) : target;
	if (!target) return null;
	let observer = new IntersectionObserver(callback, options);
	observer.observe(targetElem);
	return () => {
		observer.unobserve(targetElem);
		observer = null;
	};
}

/**
 * Emits `debug` log if we are in `development` mode.
 *
 * @param {...*} args
 * @returns {void}
 */
export function dbg(...args) {
	if (import.meta.env.MODE === "development") {
		console.debug(...args);
	}
}

/**
 * Compares two arrays of questions and returns `true` if
 * length, ordering and each question's properties are equal.
 *
 * @param {import("./types").Question[]} a
 * @param {import("./types").Question[]} b
 * @returns {boolean}
 */
export function sameQuestions(a, b) {
	// these should be arrays of the same length ...
	if (a.length !== b.length) return false;
	// ... and the order of elements matters
	for (let i = 0; i < a.length; i++) {
		const aQuest = a[i];
		const aQuestKeys = Object.keys(a[i]).toSorted();
		const bQuest = b[i];
		const bQuestKeys = Object.keys(b[i]).toSorted();
		// the number of keys in questions from the same question pair
		// should be same (their ordering is ignored)
		if (aQuestKeys.length !== bQuestKeys.length) return false;
		// finally, we are checking that both the keys and values
		// associted with those keys are the same
		for (let j = 0; j < aQuestKeys.length; j++) {
			const aQuestKey = aQuestKeys[j];
			const bQuestKey = bQuestKeys[j];
			if (aQuestKey !== bQuestKey) return false;
			if (aQuest[aQuestKey] !== bQuest[bQuestKey]) return false;
		}
	}
	return true;
}
