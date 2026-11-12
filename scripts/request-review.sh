#!/usr/bin/env bash
# One-shot adversarial review of an artifact by a headless counterpart CLI.
# Used by the interactive skill (.claude/skills/adversarial-review/) and by
# the fully-headless driver (adversarial-review.sh).
#
#   request-review.sh plan review/<subject>/plan.md   --reviewer=kimi
#   request-review.sh code review/<subject>/draft.diff --reviewer=gemini [--out=path]
#
# Writes the review to --out (default: review-N.md beside the artifact, N
# auto-incremented) and prints the output path on stdout so callers can capture it.
#
# How the reviewer is actually invoked lives in scripts/lib/headless-agent.sh, which this and
# adversarial-review.sh both source — they used to spell it out separately and drifted apart.
# The reviewer runs in the artifact's directory and may READ this whole repo (read-only; what each
# CLI enforces is in lib/headless-agent.sh's header). The prompt still carries the whole artifact.
#
# Subjects. The artifact must live under review/<subject>/ — one directory per review
# conversation, claimed by its author with a plain `mkdir` — and everything this script writes
# (the review, its prompt temp file, a reviewer's gate) lands beside the artifact. Two sessions
# reviewing at once therefore never claim each other's numbers or overwrite each other's files,
# which they did when review/ was flat.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
REVIEW_DIR="$REPO_ROOT/review"
PROMPTS_DIR="$REPO_ROOT/prompts"
# shellcheck source=lib/headless-agent.sh
. "$SCRIPT_DIR/lib/headless-agent.sh"

usage() {
    cat >&2 <<EOF
Usage: $(basename "$0") <plan|code> <artifact-path> --reviewer=claude|kimi|gemini|codex|deepseek [--out=path] [--image=path ...]

  plan mode: artifact is a plan document (uses prompts/adversarial-plan-review.md)
  code mode: artifact is a diff        (uses prompts/adversarial-code-review.md)
  --reviewer  which CLI critiques the artifact (gemini = the Antigravity CLI, agy; codex = Astra, the
              artist and UI reviewer — for a plan or diff with a screen or an asset in it; deepseek =
              Claude Code pointed at DeepSeek's endpoint, a second quality reviewer, cheap enough for
              every diff)
  --image     a screenshot the reviewer should look at (codex only; repeatable) — the UI finding is
              about what is on the screen, so hand it the screen
  --out       output file (default: review-N.md beside the artifact, N auto-incremented)

  The artifact must be under review/<subject>/ — see the header.
EOF
    exit 1
}

MODE="${1:-}"; ARTIFACT="${2:-}"; REVIEWER=""; OUT=""; IMAGES=()
shift 2 2>/dev/null || usage
for arg in "$@"; do
    case "$arg" in
        --reviewer=claude|--reviewer=kimi|--reviewer=gemini|--reviewer=codex|--reviewer=deepseek) REVIEWER="${arg#--reviewer=}" ;;
        --out=*) OUT="${arg#--out=}" ;;
        --image=*) IMAGES+=("${arg#--image=}") ;;
        *) echo "error: unknown argument '$arg'" >&2; usage ;;
    esac
done

[[ "$MODE" == "plan" || "$MODE" == "code" ]] || usage
[[ -n "$REVIEWER" ]] || usage
[[ -f "$ARTIFACT" ]] || { echo "error: artifact not found: $ARTIFACT" >&2; exit 1; }
for image in "${IMAGES[@]}"; do
    [[ -f "$image" ]] || { echo "error: image not found: $image" >&2; exit 1; }
done
if [[ ${#IMAGES[@]} -gt 0 && "$REVIEWER" != "codex" ]]; then
    echo "error: --image is for --reviewer=codex; the other reviewers take no images" >&2; exit 1
fi
case "$REVIEWER" in
    kimi)   [[ -n "$(headless_kimi_binary)" ]] || { echo "error: kimi not on PATH (set KIMI_BIN)" >&2; exit 1; } ;;
    gemini) [[ -n "$(headless_agy_binary)" ]] || { echo "error: agy not on PATH (set AGY_BIN)" >&2; exit 1; } ;;
    codex)  [[ -n "$(headless_codex_binary)" ]] || { echo "error: codex not on PATH (set CODEX_BIN)" >&2; exit 1; } ;;
    deepseek) command -v claude >/dev/null || { echo "error: 'claude' not on PATH; the deepseek arm runs Claude Code" >&2; exit 1; }
              headless_deepseek_key >/dev/null || exit 1 ;;
    *)      command -v "$REVIEWER" >/dev/null || { echo "error: '$REVIEWER' not on PATH" >&2; exit 1; } ;;
esac

# Refuse an artifact outside review/: the review and the prompt temp file are written beside the
# artifact, and beside docs/feature-plan.md they would be untracked files in a tracked directory.
SUBJECT_DIR="$(cd "$(dirname "$ARTIFACT")" && pwd)"
case "$SUBJECT_DIR" in
    "$REVIEW_DIR"/*) ;;
    *) echo "error: artifacts live in review/<subject>/ (got $ARTIFACT); mkdir review/<subject> and put it there" >&2; exit 1 ;;
esac

# Sweep what a killed run left behind — prompt temp files, kimi's handover, agy's raw output, codex's output and log —
# but only when the owning process is dead AND the file is over an hour old: two reviewers may
# share a subject directory at once, and a PID can be reused by something long-lived.
for leftover in "$SUBJECT_DIR"/.request-prompt.* "$SUBJECT_DIR"/.headless-prompt.* "$SUBJECT_DIR"/.agy-output.* "$SUBJECT_DIR"/.codex-output.* "$SUBJECT_DIR"/.codex-log.* "$SUBJECT_DIR"/.driver-prompt.*; do
    [[ -e "$leftover" ]] || continue
    owner="${leftover##*/.}"; owner="${owner#*.}"; owner="${owner%%.*}"
    if [[ "$owner" =~ ^[0-9]+$ ]] && ! kill -0 "$owner" 2>/dev/null && [[ -n "$(find "$leftover" -mmin +60 2>/dev/null)" ]]; then
        rm -f "$leftover"
    fi
done

if [[ "$MODE" == "plan" ]]; then
    PROMPT_FILE="$PROMPTS_DIR/adversarial-plan-review.md"
    LABEL="PLAN"
else
    PROMPT_FILE="$PROMPTS_DIR/adversarial-code-review.md"
    LABEL="DIFF"
fi

# Claim the name by CREATING it, not by looking and then writing. Two sessions reviewing at
# once — which is the intended setup, one per harness — both saw the same lowest free N and the
# second overwrote the first, leaving one of them adjudicating a review of someone else's diff.
# noclobber makes the create-or-fail atomic.
if [[ -z "$OUT" ]]; then
    n=1
    while true; do
        candidate="$SUBJECT_DIR/review-$n.md"
        if (set -o noclobber; : > "$candidate") 2>/dev/null; then OUT="$candidate"; break; fi
        n=$((n+1))
        if [[ "$n" -gt 999 ]]; then
            echo "error: $SUBJECT_DIR already holds 999 reviews; this subject is surely settled" >&2
            exit 1
        fi
    done
else
    # An explicit --out gets the same claim, and must stay inside the subject: a reused name would
    # otherwise clobber a review someone is reading, and a path elsewhere would escape the one
    # directory this script may write to.
    case "$(cd "$(dirname "$OUT")" 2>/dev/null && pwd)" in
        "$SUBJECT_DIR") ;;
        *) echo "error: --out must be inside the artifact's subject directory ($SUBJECT_DIR)" >&2; exit 1 ;;
    esac
    (set -o noclobber; : > "$OUT") 2>/dev/null || { echo "error: $OUT already exists; pick another name" >&2; exit 1; }
fi

echo "==> reviewer ($REVIEWER) critiquing $ARTIFACT -> $OUT" >&2

# The prompt goes to a file rather than a variable: past a certain size it can no longer be
# passed to kimi as an argv argument, and the library needs it on disk to hand over instead.
PROMPT_TMP="$SUBJECT_DIR/.request-prompt.$$"
trap 'rm -f "$PROMPT_TMP"' EXIT
{
    cat "$PROMPT_FILE"
    printf '\n=== %s UNDER REVIEW ===\n' "$LABEL"
    cat "$ARTIFACT"
} > "$PROMPT_TMP"

# The name above was claimed by creating the file, so a reviewer that dies leaves an empty
# review behind and burns that N for good. An empty one is cleared; a partial one is kept and
# said out loud, because a truncated review must never be mistaken for a settled one.
# `if ! cmd; then ... $?` reads the status of the NEGATION, which is 0 exactly when cmd failed —
# so this used to exit 0 on every reviewer failure, telling the gate and the skill that a review
# had succeeded while writing none. Fail-open at the process level, at the one point that exists
# to make failure visible. The status is captured in the else branch, where it is the real one.
HEADLESS_IMAGES=("${IMAGES[@]}")
if run_headless_agent "$REVIEWER" "$PROMPT_TMP" "$SUBJECT_DIR" "$REPO_ROOT" > "$OUT"; then
    # Exit 0 with nothing to say is not a review either: a CLI that printed a blank line and
    # exited clean would otherwise be reported as a settled, finding-free review, and the empty
    # file would burn its N for good (gemini's code review, round 5, F1).
    if [[ -z "$(tr -d '[:space:]' < "$OUT")" ]]; then
        rm -f "$OUT"
        echo "error: $REVIEWER exited 0 but wrote no review; no review was written" >&2
        exit 1
    fi
else
    status=$?
    if [[ -s "$OUT" ]]; then
        echo "error: $REVIEWER failed after writing $(wc -c < "$OUT") bytes; $OUT is INCOMPLETE" >&2
    else
        rm -f "$OUT"
        echo "error: $REVIEWER produced nothing; no review was written" >&2
    fi
    exit "$status"
fi

echo "$OUT"
