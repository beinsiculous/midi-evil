# Reviewer comparison — MIDI Evil Web

One row per review. *Real* is a finding verified against the tree and accepted (in part counts as
real); *false* is one the tree refuted; *policy* is a rebuttal on house rules rather than facts. Wall
time is the review file's mtime minus the dispatch timestamp recorded in `review/web/dispatch-N.txt`
(deepseek's from its own later dispatch, after kimi failed); the review files themselves are
transient, so the notable catch is quoted here.

Kimi, the default quality reviewer, produced no review in either round: it has no credential on
bob-ross ("The provided authorization grant is invalid", then "provider managed:kimi-code has no
credential configured"). Deepseek stood in, as the `roles` skill allows.

| round | subject | reviewer | findings | real | false | policy | notable catch (quoted) | wall |
|---|---|---|---|---|---|---|---|---|
| 1 | plan v1 | codex | 5 | 5 | 0 | 0 | "The current `RenderJob` invokes one synchronous closure; `evil-render` then invokes synchronous synthesis and mixing loops over complete audio buffers. Running that closure inline, or scheduling it for a later browser turn, still blocks until it returns." — overturned the record's W4; became D10, the Web Worker | 1:46 |
| 1 | plan v1 | deepseek | 15 | 15 | 0 | 0 | "`transport.rs:1-35` is only section start/end/repeat; tempo is `app/chrome.rs:191-204` … The gate that decides whether the whole effort happens fails on file-scope and window-width plumbing rather than on accessibility, and the failure is written up as an accessibility verdict." — and the polite-live-region finding that became D11 | 4:40 |
| 2 | plan v3 | codex | 4 | 4 | 0 | 0 | "The cursor selects C at index 0. The user activates \"Clean overlaps\"; `evil_project::clean_overlaps` sorts the vector into time order. Existing `RollState` stores selection as an index, so index 0 now identifies D." — became D6's identity rule | 1:25 |
| 2 | plan v3 | deepseek | 17 | 17 (1 in part) | 0 | 1 | "Playwright dispatches keys through CDP into the page, not through the browser UI. Browser-owned shortcuts … never reach a real renderer, but the synthetic dispatch goes to the page, so the table records \"the application receives Ctrl+N\"." — and the Delete that fires in two editors at once; the WebKit gate was rebutted on policy, its declaration accepted | 3:35 |
| 3 | probe v1 (diff) | deepseek | 8 | 8 (1 in part) | 0 | 0 | "The recipe can attach to (and then unlink the socket of) a live instance, and its cleanup runs unconditionally" — its modifier finding was right to demand proof and wrong on the mechanism: winit reads modifiers from the event's own state | 7:43 |
| 3 | probe v1 (diff) | gemini | 6 | 4 (1 in part) | 1 | 1 | "`chord.split(\"+\")` splits on every plus character" — its Critical, that a background window never takes focus, was refuted by six runs; the window-title loosening was rebutted on policy | 6:13 |
| 4 | probe v2 (diff) | deepseek | 7 | 6 | 1 | 0 | "give the private session its own runtime directory … the app can never be a secondary, and the `pgrep`/socket refusal and both `rm`s in `cleanup` stop being load-bearing" — one change that closed three findings; the pointer-payload guess was refuted by a run | 8:58 |
| 4 | probe v2 (diff) | gemini | 8 | 7 | 0 | 1 | "The patterns `Activat`, `systemd1`, and `GetItems` are unanchored substrings … the error line matches `Activat` … and is silently dropped" — forcing window activation rebutted again | 8:32 |

Totals so far: codex 9 findings, 9 real; deepseek 47 findings, 46 real, one refuted by a run, one
half rebutted on policy; gemini 14 findings, 11 real, one refuted, two rebutted on policy. Codex is
narrower and faster and found both of the plan's structural errors (W4, the cursor); deepseek finds
the most and almost nothing false, and proposed the probe's best fix; gemini is the one that reads
shell — the trap race and the stderr filter were both its — and twice asked for the window to be
forced into focus, which is the one thing this probe must not do.
