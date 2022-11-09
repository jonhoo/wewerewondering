import { writable } from "svelte/store";

// TODO: auto-update these in case local storage changes

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
