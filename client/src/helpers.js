import { localAdjustments } from './store.js'

export const animationTime = 400;

// Debounce implementaion
// Implementation grabbed from: https://youmightnotneed.com/lodash#function
export const debounce = (func, delay, { leading } = { leading: true }) => {
    let timerId

    return (...args) => {
        if (!timerId && leading) {
            func(...args)
        }
        clearTimeout(timerId)

        timerId = setTimeout(() => func(...args), delay)
    }
}

// Thought I would test out ChatGPT for this, since it seems like it would 
// likely be in its dataset. There are some clear limitations/edge cases 
// this function doesn't handle, but in our case the objects/arrays won't
// encounter them
export function deepEqual(a, b) {
    if (a === b) return true;

    if (a == null || typeof a != "object" ||
        b == null || typeof b != "object") return false;

    let keysA = Object.keys(a), keysB = Object.keys(b);

    if (keysA.length != keysB.length) return false;

    for (let key of keysA) {
        if (!keysB.includes(key) || !deepEqual(a[key], b[key])) return false;
    }

    return true;
}


let inactive_hits = 0;
export function poll_time(e) {
    if (document.hidden) {
        // if the tab is hidden, no need to refresh so often
        // if it's been hidden for a while, even less so
        // we could stop refreshing altogether, and just
        // refresh when we're visible again (i.e., on
        // visibilitychange), but it's nice if things don't
        // jump around too much when that happens.
        inactive_hits += 1;
        if (inactive_hits <= 10 /* times 30s */) {
            // For the first 5 minutes, poll every 30s
            return 30 * 1000;
        } else if (inactive_hits <= 25 /* -10 times 60s */) {
            // For the next 15 minutes, poll every 60s
            return 60 * 1000;
        } else {
            // At this point, the user probably won't
            // return to the tab for a while, so we can
            // update _very_ rarely.
            return 20 * 60 * 1000;
        }
    }

    inactive_hits = 0;
    if (e.secret) {
        // hosts should get relatively frequent updates
        return 3000;
    } else {
        // guests can wait
        return 10000;
    }
}

// because of caching, we may receive a list of questions from the
// server that doesn't reflect changes we've made (voting, asking new
// questions, toggling hidden/answered). that's _very_ confusing.
// so, we keep track of every change we make and re-apply it onto what
// we get from the server until we observe the change in the server's
// response.
export function adjustQuestions(rq, la, vf) {
    if (rq === null || rq === undefined) {
        return rq;
    }
    // deep-ish clone so we don't modify rawQuestions
    let qs = rq.map((q) => Object.assign({}, q));

    let nowPresent = {};
    for (const q of qs) {
        for (const newQ of la.newQuestions) {
            if (q.qid === newQ) {
                console.debug("no longer need to add", newQ);
                nowPresent[newQ] = true;
            }
        }
    }
    if (la.newQuestions.length > 0 || Object.keys(la.remap).length > 0) {
        console.log("question list needs local adjustments");
        let changed = Object.keys(nowPresent).length > 0;
        la.newQuestions = la.newQuestions.filter((qid) => !(qid in nowPresent));
        for (const newQ of la.newQuestions) {
            console.info("add in", newQ);
            qs.push({
                "qid": newQ,
                "hidden": false,
                "answered": false,
                "votes": 1,
            });
        }
        for (let i = 0; i < qs.length; i++) {
            let q = qs[i];
            let qid = q.qid;
            let adj = la.remap[qid];
            if (!adj) {
                continue;
            }
            console.debug("augment", qid);
            if ("hidden" in adj) {
                if (q.hidden === adj.hidden) {
                    console.debug("no longer need to adjust hidden");
                    delete la.remap[qid]["hidden"];
                    changed = true;
                } else {
                    console.info("adjust hidden to", adj.hidden);
                    qs[i].hidden = adj.hidden;
                }
            }
            if ("answered" in adj) {
                if (q.answered === adj.answered) {
                    console.debug("no longer need to adjust answered");
                    delete la.remap[qid]["answered"];
                    changed = true;
                } else {
                    console.info("adjust answered to", adj.answered);
                    qs[i].answered = adj.answered;
                }
            }
            if ("voted_when" in adj) {
                if (q.votes === adj.voted_when) {
                    console.info("adjust vote count from", q.votes);
                    // our vote likely isn't represented
                    if (vf[qid]) {
                        console.debug("adjust up");
                        qs[i].votes += 1;
                    } else {
                        console.debug("adjust down");
                        qs[i].votes -= 1;
                    }
                } else {
                    console.debug("vote count has been updated from", adj.voted_when, "to", q.votes);
                    delete la.remap[qid]["voted_when"];
                    changed = true;
                }
            }
            if (Object.keys(la.remap[qid]).length === 0) {
                console.debug("no more adjustments");
                delete la.remap[qid];
                changed = true;
            }
        }
        if (changed) {
            console.log("local adjustments changed");
            localAdjustments.set(la);
        }
    }
    qs.sort((a, b) => { return b.votes - a.votes; });
    return qs;
}


