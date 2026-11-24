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
| 5 | spike prototype (diff) | codex | 3 | 3 | 0 | 0 | "In egui 0.35, `set_focus_lock_filter` ignores the initial call because the widget did not have focus last frame … ArrowLeft changes the selected note, then egui moves focus to a control to the left. … The added test masks this by inserting two empty frames before sending arrows." | 2:29 |
| 5 | spike prototype (diff) | deepseek | 9 | 9 | 0 | 0 | found codex's three and six more, among them "`last_outcome` survives external changes, so the roll's spoken name can report an outcome the user did not just trigger" | 12:08 |
| 6 | probe v3 (diff) | gemini | 6 | 4 | 2 | 0 | "`before_step()` samples focused name before `sender.wait_for_focus()`" — two refuted by the transcript: announcements are listened for, and `--press` does pump events | 8:21 |
| 6 | probe v3 (diff) | deepseek | 7 | 6 | 0 | 1 | "The final \"Clean overlaps\" gate edge passes even when Clean overlaps does nothing … every expectation passes and the script prints \"every sequence passed.\"" — the gate's one vacuous step; the complaint that step order changed was rebutted, the gate being its only caller | 9:56 |
| 7 | web harness (diff) | codex | 2 | 1 | 1 | 0 | "Tab from the transport through the Zoom slider to its numeric value … eframe then transfers DOM focus to its text-agent input. `quiet_text_agent` has marked that input `aria-hidden`" — the web mechanism's real hole; its other finding, a missing lockfile, was the planner's own omission from the review diff | 1:41 |
| 7 | web harness (diff) | deepseek | 7 | 6 | 1 | 0 | the same hole, with the transcript line that showed it (`focus=input egui=other`), and "`other_description` retains the previous control's text" | 13:46 |
| 8 | web probe (diff) | gemini | 8 | 5 | 3 | 0 | "the script continues to execute all probe sequences against an inactive port" — three refuted against the file or by the run: the `playwright/test` import works, the nvm glob is not quoted, the server is Python 3.12 | 6:07 |
| 8 | web probe (diff) | deepseek | 7 | 7 | 0 | 0 | "The web gate exits 0 while page focus is on the hidden, unnamed input, because `--expect` skips the canvas description check whenever the canvas is not the active element" — a false pass demonstrated from the gate's own transcript | 12:00 |
| 9 | web harness, fixed (diff) | codex | 3 | 2 | 1 | 0 | "type `2`, then press Up … `typed_text` remains `Some(\"2\")` … Current still reports \"editing 2\"" | 1:53 |
| 9 | web harness, fixed (diff) | deepseek | 4 | 1 | 1 | 2 | "`Focused::Readout` publishes `transport_readout::describe(transport)` afresh each pass … announced continuously while focused during playback" — two rebutted as a throwaway branch's own business (native clippy on the spike, a rename fallback) | 11:31 |
| 10 | web probe, fixed (diff) | gemini | 6 | 3 | 1 | 2 | "`DeadlineExceeded` exits with status 1, masking hung browsers as passed negative controls" | 10:28 |
| 10 | web probe, fixed (diff) | deepseek | 6 | 5 | 0 | 1 | "`--expect` asserts only that the focused element has *a* name, so a wrong (or constant) accessible name on the adopted text entry is certified" | 10:19 |
| 11 | W0 result (document) | codex | 3 | 3 | 0 | 0 | "A user focuses the transport while stopped, starts playback with Space, waits, and requests the focused element's description again … The user therefore hears \"stopped\" and the old position while playback is running." | 1:12 |
| 11 | W0 result (document) | deepseek | 15 | 15 | 0 | 0 | "A transient outcome is baked into the persistent name/description, and the gate asserts the stale string" — and the unload guard: Alt+Left is bound by the application and is the browser's Back | 8:23 |
| 11 | W0 result (document) | gemini | 7 | 5 | 1 | 1 | "If an operation produces an announcement with no subsequent event or animation running, frame 2 never executes, leaving the accessible name stale" — its claim that committing a value drops focus to the body was refuted by the gate; its verdict, that W0 passed on synthetic tests, rebutted: a person sat it | 2:42 |
| 12 | W-A1 section v1 | codex | 6 | 6 | 0 | 0 | "The notes are `[A, A]`, with the cursor on index 1. Undo removes that second note … the selected value now exists unchanged at exactly one other index, so the cursor follows it to index 0. Delete then removes the surviving note—the opposite of R-9's required result." | 2:01 |
| 12 | W-A1 section v1 | deepseek | 17 | 17 | 0 | 0 | the same contradiction, and "a 12-minute stereo f32 render is ~127 MB of PCM that crosses the worker boundary" among nine findings on later batches, recorded against them | 4:57 |
| 13 | W-A1 section v2 | codex | 3 | 3 | 0 | 0 | "Duplicate A, producing `[A, A-copy, B]`, select the note in A-copy, then undo the duplication. `selected_track` remains index 1, which now names B." — ended the attempt to recover identity by comparing lists | 2:24 |
| 13 | W-A1 section v2 | deepseek | 10 | 10 | 0 | 0 | "the toolbar's existing convention is `selected: Option<usize>` where `None` means every note … the second click either moves every note or does nothing" | 5:07 |
| 14 | W-A1 section v3 | codex | 3 | 3 | 0 | 0 | "Delete followed by Ctrl+Z arrives before the next frame … `StudioApp::ui` currently runs global shortcuts before drawing the roll, so Undo restores the earlier project and cursor first, then the roll processes Delete" | 3:03 |
| 14 | W-A1 section v3 | deepseek | 17 | 15 | 1 | 1 | "History is recorded at frame end by `finish_project_frame` … at a point where the only selection in reach is the twin" — one misread of a gate row's fixture, one rebutted on the plan's own order (W1 splits the crate) | 5:20 |

Totals so far: codex 20 findings, 18 real, two refuted (both the same lockfile the planner had left
out of the diff it was given); deepseek 102 findings, 95 real, three refuted, the rest rebutted on
policy; gemini 41 findings, 28 real, eight refuted, five rebutted on policy. The web mechanism's
one real hole was found by codex in under two minutes and by deepseek with the evidence for it;
deepseek alone saw that the gate had passed it. Codex stays the narrow, fast, accurate one, deepseek
the thorough one, and gemini the one whose claims need checking against the file before anything is
done about them — a third of its findings on the web probe did not survive that.
