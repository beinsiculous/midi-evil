#!/usr/bin/env bash
# PostToolUse hook (ExitPlanMode matcher) for Claude Code and Kimi Code CLI:
# once a top-level plan is approved, route it through plan mode of the
# adversarial-review skill before implementation begins.
#
#   --harness=claude (default)  inject context via Claude's additionalContext JSON
#   --harness=kimi              print the instruction on stdout, exit 0 (kimi
#                               appends PostToolUse stdout to context)
#
# Silent in repos without the adversarial-review skill marker (kimi hooks live
# in the global config and fire in every project). Subagents implementing a
# section of an already-reviewed plan are exempt: the instruction says to skip.
set -euo pipefail

HARNESS=claude
for arg in "$@"; do
    case "$arg" in
        --harness=claude|--harness=kimi) HARNESS="${arg#--harness=}" ;;
        *) echo "error: unknown argument '$arg'" >&2; exit 1 ;;
    esac
done
REVIEWER=kimi
[ "$HARNESS" = "kimi" ] && REVIEWER=claude

# Repo guard: only repos that carry the adversarial-review skill participate.
top="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
[ -f "$top/.kimi-code/skills/adversarial-review/SKILL.md" ] && : || \
[ -f "$top/.claude/skills/adversarial-review/SKILL.md" ] || exit 0

message="Project convention: an approved top-level plan goes through PLAN mode of the adversarial-review skill before implementation. Claim a subject directory with mkdir review/<subject> (no -p: an existing one belongs to another session), write the approved plan to review/<subject>/plan.md, run scripts/request-review.sh plan review/<subject>/plan.md --reviewer=${REVIEWER} (or --reviewer=gemini if ${REVIEWER} is out of credits or not on PATH; add --reviewer=codex, the artist, for a plan or diff with a screen or an asset in it), present every finding faithfully, adjudicate ACCEPT/REBUT with the user, write review/<subject>/rebuttal-N.md (and plan-vN+1.md if anything changed), and ask whether another round is wanted. Skip this only if you are a subagent implementing a section of a plan that was already reviewed, or the user explicitly opts out. If neither ${REVIEWER} nor gemini is available, tell the user and ask whether to proceed unreviewed."

if [ "$HARNESS" = "kimi" ]; then
    printf '%s\n' "$message"
    exit 0
fi
command -v jq >/dev/null 2>&1 || exit 0
jq -n --arg ctx "$message" '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $ctx}}'
