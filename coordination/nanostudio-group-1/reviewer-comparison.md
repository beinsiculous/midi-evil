# NanoStudio Group 1 — reviewer comparison

Kimi authored the plan. Claude was quality reviewer, Gemini executor reviewer and Codex UI reviewer.
Jesse adjudicated every finding; decisions were folded into the settled plan. Review transients live in
`review/nanostudio-group-1/` until implementation dispatch/reviews finish.

Wall time below is an **upper bound**, measured from each draft's mtime to the review file's mtime.
Separate dispatch timestamps were not captured, so these bounds can include user adjudication time and
must not be treated as model latency. This recording gap is corrected for subsequent code reviews by
capturing dispatch timestamps explicitly.

| Round | Reviewer | Findings | Real | False | Policy rebut | Notable catch | Upper bound |
| --- | --- | --- | ---: | ---: | ---: | --- | ---: |
| 1 | Claude | 10 | 9 | 0 | 1 | “performance `values` are a second source of truth beside the target parameters” | 247 s |
| 1 | Gemini | 7 | 6 | 1 | 0 | “Mutating patch fields in-place for performance controls destroys base preset data” | 478 s |
| 1 | Codex | 7 | 6 | 0 | 1 | “Most performance routes will not affect held notes” | 754 s |
| 2 | Claude | 8 | 8 | 0 | 0 | “per-frame offsets on time-shaped destinations break a voice engine that is closed-form in voice time” | 462 s |
| 2 | Gemini | 7 | 5 | 1 | 1 | “Serde `rc` does not deduplicate” | 728 s |
| 2 | Codex | 5 | 5 | 0 | 0 | “the batch scope omits the production handoff needed for offline effect routes” | 952 s |
| 3 | Claude | 10 | 10 | 0 | 0 | “the new load ceiling refuses legacy sliced patches that the shipped build wrote” | 313 s |
| 3 | Gemini | 6 | 5 | 0 | 1 | “Destructive editing on shared `sample_sources` silently corrupts or invalidates sibling zones” | 493 s |
| 3 | Codex | 4 | 4 | 0 | 0 | “Save & assign can mix preset versions within one export” | 693 s |
| 4 | Claude | 12 | 12 | 0 | 0 | “the plan leans on a Patch Lab undo that does not exist” | 405 s |
| 4 | Gemini | 7 | 5 | 2 | 0 | “Destructive delay-tail preprocessing produces un-loadable patches exceeding decoder duration limits” | 673 s |
| 4 | Codex | 5 | 5 | 0 | 0 | “Multiple routes to one destination have no defined combination rule” | 1,036 s |
| 5 | Claude | 11 | 11 | 0 | 0 | “Pinning instrument snapshots at creation moves loading onto the click” | 992 s |
| 5 | Gemini | 7 | 7 | 0 | 0 | “Destructive CoW edits on shared sources break deduplication” | 1,328 s |
| 5 | Codex | 2 | 2 | 0 | 0 | “The persisted model cannot distinguish derived legacy depths from authored depths” | 1,536 s |

## Batch 0 implementation reviews

Code-review wall time is dispatch timestamp to review-file mtime. Gemini and DeepSeek timed out without
verdicts in round 8 and are not counted as reviews. Gemini's round-9 small artifact produced three false
findings caused by stable-version/full-diff assumptions and one valid range-cadence catch. Jesse explicitly
ended further rounds after round 19; the final fixes were gated without another review cycle.

| Round | Reviewer | Findings | Real | False/deferred | Notable catch | Wall time |
| --- | --- | --- | ---: | ---: | --- | ---: |
| 6 | Claude | 5 | 4 | 1 deferred | unrelated missing instruments now block renders | 222 s |
| 6 | Codex | 3 | 3 | 0 | drum-bank saves bypass the resolution guard | 362 s |
| 7 | Claude | 7 | 6 | 1 deferred | queued render prematurely ends recording | 250 s |
| 7 | Codex | 6 | 6 | 0 | queued exports capture subsequent edits | 449 s |
| 7 | Gemini | 4 | 1 | 3 false | selected-part full-tail claim was rebutted | timed out later pass |
| 8 | Claude | 7 | 6 | 1 deferred | queued render cannot be withdrawn | 465 s |
| 8 | Codex | 4 | 4 | 0 | export omits just-committed recording | 629 s |
| 9 | Claude | 6 | 6 | 0 | cancelled drain overwrites document status | 294 s |
| 9 | Codex | 3 | 3 | 0 | temp cleanup deletes another process's export | 530 s |
| 9 | Gemini | 4 | 1 | 3 false | unaligned analysis ranges miss cancellation | 530 s |
| 10 | Claude | 6 | 5 | 1 deferred | stalled I/O locks transport and saving | 455 s |
| 10 | Codex | 3 | 3 | 0 | detached retirement needs shutdown ownership | 574 s |
| 11 | Claude | 6 | 5 | 1 deferred | temp recognizer does not match generated names | 400 s |
| 11 | Codex | 4 | 4 | 0 | canceled worker can overwrite newer export | 581 s |
| 12 | Claude | 6 | 4 | 1 deferred, 1 unmeasured | refused batch ends recording | 512 s |
| 12 | Codex | 3 | 3 | 0 | global locks let unrelated exports stall | 624 s |
| 13 | Claude | 5 | 4 | 1 deferred | Space can cancel an export while ending a take | 615 s |
| 13 | Codex | 4 | 4 | 0 | destination aliases bypass publication lock | 736 s |
| 14 | Claude | 3 | 3 | 0 | deferred outcomes can be announced twice | 571 s |
| 14 | Codex | 3 | 3 | 0 | private audio is exposed before permission copy | 563 s |
| 15 | Claude | 3 | 3 | 0 | transport stop misses deferred outcome clear | 570 s |
| 15 | Codex | 2 | 2 | 0 | dangling chains lose intermediate links | 638 s |
| 16 | Claude | 4 | 3 | 1 false | web-time was already a direct dependency | 488 s |
| 16 | Codex | 4 | 4 | 0 | metadata replacement bypasses effective access | 588 s |
| 17 | Claude | 4 | 4 | 0 | direct cancel can leave plausible truncated WAV | 506 s |
| 17 | Codex | 2 | 2 | 0 | dangling-chain retry bypasses locking | 670 s |
| 18 | Claude | 5 | 4 | 1 false | candidate worktree contains direct web-time dependency | 478 s |
| 18 | Codex | 2 | 2 | 0 | sparse drums skip all cancellation checkpoints | 664 s |
| 19 | Claude | 5 | 5 | 0 | failed Open withdraws a queued export | 729 s |
| 19 | Codex | 2 | 2 | 0 | new-file rename can overwrite concurrent data | 769 s |

The recurring high-value catches were serialization identity, active-voice semantics, exact-byte
verification and publication ownership. The plan was settled only after those became explicit contracts
and separate batches.
