---
name: file-issue
description: File unfinished work, discovered debt, or a follow-up as a GitHub issue on the Insiculous Studio Board, in the house shape. Use whenever you are about to defer, drop, or note work you did not finish - and before reporting a task done. Also use when asked to "file that", "open an issue", or "put it on the board".
---

# File an issue

Open work lives on the **Studio Board**
(https://github.com/orgs/beinsiculous/projects/1), as issues in the repo the work
belongs to. Nothing tracked lives in a markdown file — the per-crate `TECH_DEBT.md`
files were retired Aug 28 2026 and must never be recreated.

**The rule this skill serves:** anything you don't finish — work you deferred, debt you
created, a follow-up you spotted — is filed before you report done. Never buried in a
doc, never left as a bare `TODO:`, never dropped.

## 0. Name the repo first — every time

`gh` resolves a bare command against the **session's working directory**, not the code
you are reasoning about. From the working-set root that is `beinsiculous/insiculous`, so
a bare `gh issue create` while working on a game files the issue into the admin repo.
The harness also resets the working directory between tool calls, so a `cd` does not
survive. This is the same hazard the working set legislates for git with `git -C <repo>`.

**Pass `-R beinsiculous/<repo>` on every call.** If the target is not obvious, ask git:

```sh
git -C <path-to-repo> remote get-url origin     # -> https://github.com/beinsiculous/<repo>.git
```

Work belongs to the repo that owns the code: engine → `insiculous_2d`; a game → that
game's own repo (`pong`, `snake`, `breakout`, `frogger`, `asteroids`, `space_invaders`);
the site and the web face → `insiculous_web`; the Fort Knight app → `fortknight`;
Deion art → `deion_assets`; the working set's own docs, prompts and skills →
`insiculous`.

## 1. Search before you file

```sh
gh issue list -R beinsiculous/<repo> --search "<keywords>" --state all
gh issue list -R beinsiculous/<repo> --label tech-debt
```

If it is already tracked, **add a comment to that issue** with the new evidence and cite
its number in your report. Do not open a duplicate. If an existing issue is resolved by
current code, say so — it gets closed, with reusable lessons appended to `log_archive.md`
where the repo keeps one.

## 2. Title

```
[<scope>][<label>] <ID> — <what is wrong, concretely>
```

`scope` is the crate, area or game (`ecs`, `renderer`, `games`, `deploy`, `profile`).
`label` is usually `tech-debt`. `ID` is a `[CATEGORY-NNN]`-style id (`DRY`, `SRP`,
`KISS`, `ARCH`, `GAP`, `BUILD`, `UX`, `DOC`) — include one only where a stable id already
exists or the area already numbers its findings; a plain summary is fine otherwise.

Real examples:

```
[games][tech-debt] BUILD-001 — my_platformer does not compile (player_box arity)
[common][tech-debt] DRY-002 — volume clamping duplicated across audio and ecs
[deploy] I0 — AI-asset purge gate wired into paid publish paths
```

## 3. Body — evidence, not vibes

Four things, in this order. A finding that cannot name a concrete failure is not a
finding — the same bar the adversarial-review convention sets.

1. **What and where**, with `file:line` pointers that a reader can chase.
2. **The evidence, and when it was last checked.** Say the date plainly:
   "Re-verified broken Aug 28 2026 — `cargo check` still fails with the same error."
   A stale claim nobody re-checked is worse than no issue.
3. **What "done" looks like** — the acceptance condition, so whoever picks it up knows
   when to stop.
4. **Fix or retire**, where the item may not be worth doing at all. Say so explicitly
   rather than leaving a decision implied.

Include effort (Small/Medium/Large) and priority when you know them.

## 4. One item per issue — except low-priority backlogs

High and Medium findings each get their own issue.

**Low findings collect.** Each crate or area keeps a single
`[<area>][tech-debt] Low-priority backlog (<IDs>)` issue holding a checkbox list, one
line per item with its id and a code pointer. Extend that issue; do not open a second
backlog for the same area. Create one only if the area has none.

```sh
gh issue list -R beinsiculous/<repo> --label tech-debt --search "Low-priority backlog"
```

Backlog issues stay **out of sprint milestones** on purpose — they are worked
opportunistically when you are already in that crate, never taken as "the next task".

## 5. Label it

```sh
gh issue create -R beinsiculous/<repo> --label tech-debt --title "..." --body "..."
```

`gh` fails if the label does not exist in that repo. Check and create once:

```sh
gh label list -R beinsiculous/<repo> | grep -q '^tech-debt' \
  || gh label create tech-debt -R beinsiculous/<repo> --color D93F0B \
       --description "Technical debt: known compromise or deferred work"
```

## 6. Put it on the board and set its fields

`.github/workflows/add-to-project.yml` adds new issues automatically in every repo that
has it — which is every repo **except the root `insiculous` repo**, tracked as
`beinsiculous/insiculous#7`. Add those by hand:

```sh
gh project item-add 1 --owner beinsiculous --url <issue-url>
```

`fortknight` has the workflow but it fails until its `ADD_TO_PROJECT_PAT` secret is set.
Check whether the issue landed on the board before adding it by hand — adding one that the
workflow already added creates a duplicate row.

Then set the board fields. One call gives you every row with its item id:

```sh
gh project item-list 1 --owner beinsiculous --limit 200 --format json
gh project field-list 1 --owner beinsiculous
```

- **Priority** — `P0 urgent` · `P1 next` · `P2 soon` · `P3 someday`
- **Phase** — `E Asset Pipeline` · `F Deion Art` · `G Re-skins` · `H Web Port` ·
  `I Deploy` · `J Arcade` · `Editor` · `Tech Debt` · `Ops`
- **Sprint** — only if it belongs to a live batch; see the `sprint-planning` skill.
  Setting `Sprint` means also setting the matching repo milestone, and
  `scripts/check-sprint-sync.sh` in the working set reports any pair that disagrees.

## 7. Closing it later

Reference the issue in the commit that resolves it:

```
fixes beinsiculous/<repo>#N
```

If the resolution carries a lesson worth keeping, append it to that repo's
`log_archive.md` before closing.

## What is NOT an issue

- Work the user explicitly declined. Their "no" is the record.
- Speculative "might be nice" notes with no failure scenario attached.
- Anything you can finish now in the time it takes to write the issue. Do it instead.

## Report honestly

Your final summary names every issue you filed, by number, and every follow-up you chose
**not** to file, with the reason. "Nothing deferred" is a claim — only make it if it is
true.
