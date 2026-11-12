#!/usr/bin/env bash
# Adversarial review loop between Claude Code and the kimi CLI.
#
#   adversarial-review.sh plan <task-spec.md> --author=claude|kimi [--reviewer=claude|kimi|gemini|codex|deepseek]
#   adversarial-review.sh code <changes.diff> --author=claude|kimi [--reviewer=claude|kimi|gemini|codex|deepseek]
#
# One agent authors, another reviews: the counterpart by default (whoever is not
# --author), or the reviewer named — never the author's own vendor. Exactly one
# rebuttal round. All outputs land in review/headless/, this driver's own subject
# directory (gitignored transients), which it clears at start and never looks
# outside of — other subjects under review/ belong to other sessions:
#
#   plan mode: plan.md -> review-1.md -> rebuttal-1.md [+ plan-v2.md if revised]
#   code mode: draft.diff (copy of input) -> review-1.md -> rebuttal-1.md
#
# This is the FULLY-HEADLESS variant (both roles run non-interactively). For
# the human-in-the-loop variant where your interactive session (Claude Code or
# kimi) authors and only the review step is headless, use the
# `adversarial-review` skill (.claude/skills/adversarial-review/), which calls
# scripts/request-review.sh for the review step — as does this driver.
#
# How each agent is actually invoked lives in scripts/lib/headless-agent.sh, which this and
# request-review.sh both source. They used to spell it out separately and drifted: this file went
# on passing --quiet and --work-dir long after request-review.sh had recorded that kimi-code
# dropped both, so a kimi-authored run died on any machine without a legacy kimi-cli install.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
REVIEW_DIR="$REPO_ROOT/review/headless"
REVISED_MARKER='=== REVISED PLAN ==='
# shellcheck source=lib/headless-agent.sh
. "$SCRIPT_DIR/lib/headless-agent.sh"

usage() {
    cat >&2 <<USAGE
Usage: $(basename "$0") <plan|code> <input-path> --author=claude|kimi [--reviewer=claude|kimi|gemini|codex|deepseek]

  plan mode: <input-path> is a task spec; the author drafts a plan first.
  code mode: <input-path> is a diff; the diff itself is the draft.
  --author    who authors/rebuts.
  --reviewer  who reviews; defaults to the other of claude/kimi, and may be
              gemini (the Antigravity CLI, agy), codex (Astra, the roster's
              artist and UI reviewer) or deepseek (Claude Code pointed at
              DeepSeek's endpoint). Never the author's own vendor.
USAGE
    exit 1
}

# --- argument parsing --------------------------------------------------------

MODE="${1:-}"; INPUT="${2:-}"; AUTHOR=""; REVIEWER=""
shift 2 2>/dev/null || usage
for arg in "$@"; do
    case "$arg" in
        --author=claude|--author=kimi) AUTHOR="${arg#--author=}" ;;
        --reviewer=claude|--reviewer=kimi|--reviewer=gemini|--reviewer=codex|--reviewer=deepseek) REVIEWER="${arg#--reviewer=}" ;;
        *) echo "error: unknown argument '$arg'" >&2; usage ;;
    esac
done

[[ "$MODE" == "plan" || "$MODE" == "code" ]] || usage
[[ -n "$AUTHOR" ]] || usage
[[ -f "$INPUT" ]] || { echo "error: input file not found: $INPUT" >&2; exit 1; }

if [[ -z "$REVIEWER" ]]; then
    if [[ "$AUTHOR" == "claude" ]]; then REVIEWER="kimi"; else REVIEWER="claude"; fi
fi
[[ "$REVIEWER" != "$AUTHOR" ]] || { echo "error: the reviewer must be a different vendor from the author ($AUTHOR)" >&2; exit 1; }

# Preflight only what will actually run.
preflight() {
    case "$1" in
        kimi)   [[ -n "$(headless_kimi_binary)" ]] || { echo "error: kimi not on PATH (set KIMI_BIN)" >&2; exit 1; } ;;
        gemini) [[ -n "$(headless_agy_binary)" ]] || { echo "error: agy not on PATH (set AGY_BIN)" >&2; exit 1; } ;;
        codex)  [[ -n "$(headless_codex_binary)" ]] || { echo "error: codex not on PATH (set CODEX_BIN)" >&2; exit 1; } ;;
        deepseek) command -v claude >/dev/null || { echo "error: 'claude' not on PATH; the deepseek arm runs Claude Code" >&2; exit 1; }
                  headless_deepseek_key >/dev/null || exit 1 ;;
        *)      command -v "$1" >/dev/null || { echo "error: '$1' not on PATH" >&2; exit 1; } ;;
    esac
}
preflight "$AUTHOR"
preflight "$REVIEWER"
mkdir -p "$REVIEW_DIR"

if compgen -G "$REVIEW_DIR/*.md" >/dev/null; then
    echo "note: overwriting previous run's files in review/headless/" >&2
fi
rm -f "$REVIEW_DIR"/plan.md "$REVIEW_DIR"/plan-v*.md "$REVIEW_DIR"/review-*.md \
      "$REVIEW_DIR"/rebuttal-*.md "$REVIEW_DIR"/draft.diff

# --- agent invocation --------------------------------------------------------

# invoke <claude|kimi>: prompt on stdin, response text on stdout. The prompt lands in a file on
# the way through, because past a certain size it can no longer be passed to kimi as an argv
# argument and the library hands it over on disk instead.
trap 'rm -f "$REVIEW_DIR"/.driver-prompt.$$' EXIT
invoke() {
    local prompt_tmp="$REVIEW_DIR/.driver-prompt.$$" status=0
    cat > "$prompt_tmp"
    if run_headless_agent "$1" "$prompt_tmp" "$REVIEW_DIR" "$REPO_ROOT"; then status=0; else status=$?; fi
    rm -f "$prompt_tmp"
    return "$status"
}

step() { echo "==> [$1] $2" >&2; }

# --- run ---------------------------------------------------------------------

if [[ "$MODE" == "plan" ]]; then
    DRAFT="$REVIEW_DIR/plan.md"
    DRAFT_LABEL="PLAN"

    step "1/3" "author ($AUTHOR) drafting plan from $INPUT"
    {
        cat <<'PROMPT'
Write a concrete implementation plan for the task below. Be specific about
steps, files, data flow, failure handling, and how the result is verified.
This plan will be adversarially reviewed, so make your assumptions explicit.
Respond with the plan in markdown only — do not modify any files.

=== TASK ===
PROMPT
        cat "$INPUT"
    } | invoke "$AUTHOR" > "$DRAFT"
else
    DRAFT="$REVIEW_DIR/draft.diff"
    DRAFT_LABEL="DIFF"

    step "1/3" "code mode: diff is the draft, copying $INPUT -> review/headless/draft.diff"
    cp "$INPUT" "$DRAFT"
fi

step "2/3" "reviewer ($REVIEWER) critiquing"
"$SCRIPT_DIR/request-review.sh" "$MODE" "$DRAFT" \
    --reviewer="$REVIEWER" --out="$REVIEW_DIR/review-1.md" >/dev/null

step "3/3" "author ($AUTHOR) rebutting"
{
    if [[ "$MODE" == "plan" ]]; then
        cat <<PROMPT
You wrote the plan below, which then received the adversarial review that
follows it. Address every numbered finding (F1, F2, ...) explicitly — for each
one, state either ACCEPT (and exactly how the plan changes) or REBUT (and why
the failure scenario does not hold). Do not skip or merge findings.

If you accept any finding, output the complete revised plan after a line
containing exactly:
$REVISED_MARKER
If you rebut everything, omit that marker and the revised plan.
Respond with text only — do not modify any files.
PROMPT
    else
        cat <<'PROMPT'
You authored the diff below, which then received the adversarial review that
follows it. Address every numbered finding (F1, F2, ...) explicitly — for each
one, state either ACCEPT (acknowledging the issue and what the fix would be) or
REBUT (and why the failure scenario does not hold). Do not skip or merge
findings. Do not produce a revised diff and do not modify any files — respond
with the rebuttal text only.
PROMPT
    fi
    printf '\n=== YOUR %s ===\n' "$DRAFT_LABEL"
    cat "$DRAFT"
    printf '\n=== REVIEW ===\n'
    cat "$REVIEW_DIR/review-1.md"
} | invoke "$AUTHOR" > "$REVIEW_DIR/rebuttal-1.raw"

# Split rebuttal from revised plan on the marker (plan mode only; marker absent
# means the author rebutted everything and there is no v2).
# The marker must be the WHOLE line, not merely present in one. Substring matching split the
# file on any line that mentioned the marker — and the rebuttal prompt above quotes it verbatim,
# so an author who rebutted everything while quoting the instruction back ("you asked for a
# revised plan after the marker line; I decline because...") had half its rebuttal filed as a
# revised plan. A marker that does not stand alone now leaves the rebuttal whole, which is the
# harmless direction: no plan-v2.md rather than a bogus one.
if [[ "$MODE" == "plan" ]] && grep -qxF "$REVISED_MARKER" "$REVIEW_DIR/rebuttal-1.raw"; then
    awk -v m="$REVISED_MARKER" '$0 == m { found=1; next } !found' \
        "$REVIEW_DIR/rebuttal-1.raw" > "$REVIEW_DIR/rebuttal-1.md"
    awk -v m="$REVISED_MARKER" 'found; $0 == m { found=1 }' \
        "$REVIEW_DIR/rebuttal-1.raw" > "$REVIEW_DIR/plan-v2.md"
else
    mv "$REVIEW_DIR/rebuttal-1.raw" "$REVIEW_DIR/rebuttal-1.md"
fi
rm -f "$REVIEW_DIR/rebuttal-1.raw"

echo >&2
echo "Done. Author: $AUTHOR, reviewer: $REVIEWER. Outputs:" >&2
ls -1 "$REVIEW_DIR" | grep -E '\.(md|diff)$' | sed 's|^|  review/headless/|' >&2
