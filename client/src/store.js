import { writable } from "svelte/store";

const storedVotedFor = JSON.parse(localStorage.getItem("votedFor"));
export const votedFor = writable(!storedVotedFor ? {} : storedVotedFor);
votedFor.subscribe(value => {
    if (value) {
        localStorage.setItem("votedFor", JSON.stringify(value));
    } else {
        localStorage.removeItem("votedFor");
    }
});

const storedQs = JSON.parse(localStorage.getItem("questions"));
export const questionTexts = writable(!storedQs ? {} : storedQs);
questionTexts.subscribe(value => {
    if (value) {
        localStorage.setItem("questions", JSON.stringify(value));
    } else {
        localStorage.removeItem("questions");
    }
});

window.addEventListener('storage', (e) => {
	if (e.key == "votedFor") {
		votedFor.set(JSON.parse(e.newValue));
	} else if (e.key == "questions") {
		questionTexts.set(JSON.parse(e.newValue));
	}
});
