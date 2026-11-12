#!/usr/bin/env bash
# PreToolUse hook (Bash matcher) for Claude Code and Kimi Code CLI: before a
# `git commit`, if the pending diff is big, require an adversarial code review
# (scripts/request-review.sh code ...) before committing.
#
#   --harness=claude (default)  deny via Claude's permissionDecision JSON
#   --harness=kimi              deny via stderr + exit 2 (kimi's block protocol)
#
# Silent (exit 0, no output) for: non-commit commands, non-repo dirs, repos
# without the adversarial-review skill marker (kimi hooks live in the GLOBAL
# config, so they fire in every project — the marker keeps them project-scoped),
# diffs under THRESHOLD changed lines, and the two doors past the gate below.
# Otherwise DENIES the commit with instructions — informing alone can't stop
# the triggering command.
#
# The two doors, and they mean different things:
#   ADV_REVIEWED=1 prefix   the review HAPPENED — code mode ran and every
#                           finding was adjudicated with the user.
#   a signed skip trailer   the review did NOT happen and a developer said so
#                           in writing, in the commit message (see below).
# ADV_REVIEWED=1 used to cover both, which let an agent skip a review on its
# own reading of a conversation and leave no trace that it had. It no longer
# does: skipping now costs a sentence and a name, written into the history it
# skipped review for.
#
# Be clear about what that buys (review-1 F1): friction and a paper trail, not
# proof of authorship. Nothing here can verify that a human typed the trailers
# — an agent that writes them forges a person's name into the record, which is
# worse than the old silent self-skip, not better. The only thing standing in
# that spot is the instruction, in both SKILL.md files and in the denial below,
# that an agent never writes them. The mechanism makes a skip visible and
# attributable; it does not make it honest.
set -euo pipefail

THRESHOLD=100
HARNESS=claude
for arg in "$@"; do
    case "$arg" in
        --harness=claude|--harness=kimi) HARNESS="${arg#--harness=}" ;;
        *) echo "error: unknown argument '$arg'" >&2; exit 1 ;;
    esac
done
# The counterpart CLI that reviews this harness's work.
REVIEWER=kimi
[ "$HARNESS" = "kimi" ] && REVIEWER=claude

# Degrade gracefully where jq is missing (review-3 F1): a reminder gate must
# never break basic shell use on a machine without the dependency.
command -v jq >/dev/null 2>&1 || exit 0

input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""')
# All matching runs against the command with quoted segments removed, so
# commit-message text can neither smuggle the ADV_REVIEWED=1 bypass
# (deion_assets review-1 F3) nor trigger the staging fallback below
# (review-1 F2).
stripped=$(printf '%s' "$cmd" | sed "s/'[^']*'//g; s/\"[^\"]*\"//g")
# A second view, for deciding whether a path is named (see names_paths). There, deleting a
# quoted span is exactly wrong: the question is where a message ENDS, and `-m "x" file.txt`
# stripped down to `-m  file.txt` cannot answer it. Whether the value is consumed or not, one
# of the two readings is wrong — either a real path is missed, or `-m x` reads as a path and
# an ordinary commit with a dirty tree is denied. Holding each quoted span open as a single
# opaque token answers it: `-m @Q@ file.txt` has a message AND a path, and both are visible.
placeheld=$(printf '%s' "$cmd" | sed "s/'[^']*'/@Q@/g; s/\"[^\"]*\"/@Q@/g")
# Detect the commit itself on the RAW command so `bash -c "git commit ..."` /
# `eval "git commit ..."` cannot slip past by hiding it inside quotes
# (FortKnight review-1 F2). A commit message that merely mentions "git commit"
# trips the gate too — a harmless false positive.
# `git commit` as a literal substring missed every redirected commit — the
# whole class this gate exists for in a nested working set, since
# `git -C <repo> commit` does not contain it.
#
# Between `git` and `commit`, allow git's own flags and each flag's value
# (`-C nested`, `-c k=v`, `-C "with space"`). A bare token that is NOT a flag
# value ends the match, so `git log --grep=commit` is still not a commit: `log`
# is neither a flag nor a flag's value.
q="'"
commit_pattern="\\bgit\\b([[:space:]]+(-[^[:space:]]+|\"[^\"]*\"|$q[^$q]*$q)([[:space:]]+(\"[^\"]*\"|$q[^$q]*$q|[^-[:space:]][^[:space:]]*))?)*[[:space:]]+commit([[:space:]]|$)"
printf '%s' "$cmd" | grep -qE "$commit_pattern" || exit 0
# Door 1: the review happened. The token counts only as a leading env
# assignment (or one right after && / ;) — never as free text inside a commit
# message (review-3 F3).
case "$stripped" in
    "ADV_REVIEWED=1 "* | *"&& ADV_REVIEWED=1 "* | *"; ADV_REVIEWED=1 "*) exit 0 ;;
esac

# Door 2: a signed skip. The only way to land a big diff with no review, and
# it lives in the commit message rather than in a shell variable nobody will
# ever read again — a skip belongs in the history it skipped review for:
#
#     Adversarial-Review-Skipped: <reason, more than 10 characters>
#     Skip-Signed-Off-By: <the developer's name>
#
# Any reason over ten characters is accepted — "just because" is a perfectly
# good reason, and this hook is not the judge of it. What is not optional is
# that a person signs it. Both trailers are required and case-sensitive.
SKIP_REASON_MINIMUM=10
# Git keeps trailers in the last block of a message and so do we: only the tail
# is scanned. Prose that quotes the trailer format — this repo's own docs, or
# the commit that lands them — then reads as prose instead of as a skip nobody
# asked for (review-1 F2, which fired on the very command that wrote the docs).
SKIP_TRAILER_TAIL_LINES=12

# Where a -F/--file message lives, in every spelling git accepts: -F path,
# -Fpath, --file=path, --file path, quoted or not (review-1 F3). `-F -` yields
# "-", which is not a file, so a stdin heredoc falls through to the command
# string, where its body already sits.
#
# Only the part of the command BEFORE any -m/--message is searched (review-2
# F2). git takes its message from one place or the other, never both, so a
# -F that appears after -m is inside the message text — and the denial below
# hands out the string "-F .git/COMMIT_EDITMSG", which a commit describing
# this convention will quote. Reading that file would be the worst possible
# miss: it holds the PREVIOUS attempt's message, the one file on disk likely
# to carry skip trailers, so a stale signature could wave a fresh commit
# through. The earlier comment here claimed no such file would exist. It was
# wrong, and this hook created it.
message_file() {
    file_argument=''
    before_message="$cmd"
    for message_flag in ' -m' ' --message'; do
        case "$before_message" in
            *"$message_flag"*) before_message="${before_message%%$message_flag*}" ;;
        esac
    done
    if [[ "$before_message" =~ (--file=|--file[[:space:]]+|-F[[:space:]]*)(\"[^\"]+\"|\'[^\']+\'|[^[:space:]]+) ]]; then
        file_argument="${BASH_REMATCH[2]}"
        file_argument="${file_argument%\"}"; file_argument="${file_argument#\"}"
        file_argument="${file_argument%\'}"; file_argument="${file_argument#\'}"
    fi
    printf '%s' "$file_argument"
}

# The message as *this command* carries it. -m/--message text and heredoc
# bodies are already inside the command string; a message file is read from
# disk (relative to the shell's cwd, which is why this runs before the cd).
# A message composed in $EDITOR cannot be seen from here, so a skip that goes
# through the editor is denied rather than assumed — the denial says how to
# retry. Paths are read relative to THIS process's working directory, so
# `git -C elsewhere commit -F msg.txt` cannot be seen either (review-2 F3);
# that fails closed, and the denial says which directory it looked in.
message_tail() {
    path=$(message_file)
    {
        printf '%s\n' "$cmd"
        if [ -n "${path:-}" ] && [ -f "$path" ]; then cat -- "$path"; fi
    } | tail -n "$SKIP_TRAILER_TAIL_LINES"
}

# The LAST occurrence wins, as it does in git's own trailer handling: a message
# that quotes the format and then signs for real is judged by the signature,
# not by the quotation (review-2 F1).
trailer_value() {
    message_tail | sed -n "s/^[[:space:]]*$1:[[:space:]]*//p" | tail -1 | sed 's/[[:space:]]*$//'
}

# A value that opens with '<' is the documented template — `<reason, more than
# 10 characters>` is thirty-three characters and would otherwise sail through
# the length check, signing a skip in the name of a placeholder (review-2 F1).
# Only the opening bracket disqualifies, so `Skip-Signed-Off-By: M <m@x.com>`
# is still a person signing their name.
is_placeholder() {
    case "$1" in '<'*) return 0 ;; *) return 1 ;; esac
}

skip_reason=$(trailer_value 'Adversarial-Review-Skipped')
skip_signer=$(trailer_value 'Skip-Signed-Off-By')
skip_problem=''
if [ -n "$skip_reason" ] || [ -n "$skip_signer" ]; then
    if is_placeholder "$skip_reason" || is_placeholder "$skip_signer"; then
        skip_problem="the trailers still hold the template ('${skip_reason}' / '${skip_signer}'), not a reason and a name. Quoting the format is not skipping a review."
    elif [ -z "$skip_reason" ]; then
        skip_problem="signed by '${skip_signer}' but no 'Adversarial-Review-Skipped:' trailer — sign a reason, not a blank."
    elif [ "${#skip_reason}" -le "$SKIP_REASON_MINIMUM" ]; then
        skip_problem="the reason '${skip_reason}' is ${#skip_reason} characters; more than ${SKIP_REASON_MINIMUM} are needed. Any reason over ${SKIP_REASON_MINIMUM} characters is accepted — even 'just because' — so say something."
    elif [ -z "$skip_signer" ]; then
        skip_problem="reason given, nobody signed it. Add 'Skip-Signed-Off-By: <your name>' — a skip goes on the record under a name."
    else
        exit 0
    fi
fi

deny() {
    if [ "$HARNESS" = "kimi" ]; then
        printf '%s\n' "$1" >&2
        exit 2
    fi
    jq -n --arg r "$1" '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $r}}'
    exit 0
}

participates() {
    [ -f "$1/.kimi-code/skills/adversarial-review/SKILL.md" ] || [ -f "$1/.claude/skills/adversarial-review/SKILL.md" ]
}

# Which repository is this commit actually for?
#
# In a nested working set (an admin repo with the project repos cloned inside
# it) the commit is usually issued from the parent, so the repo being committed
# to is NOT the one this hook stands in. Sizing the parent's diff there finds 0
# changed lines and waves a 500-line commit through, silently — worse than no
# gate at all.
#
# Resolution is per SEGMENT. The command is split on && || ; | and the segments
# are walked in order: a `cd` segment moves the working directory for everything
# after it, and only the segment that actually commits owns the -C that counts.
# Taking any -C in the command was a real bypass — `git -C small status; git
# commit` sized `small`, found nothing, and let the real commit land unreviewed
# (code review F1, reproduced before this was written).
#
# The gate never interprets a path: it hands one directory to git and denies
# anything it cannot reduce to exactly one repository. Failing closed is the only
# safe direction for a gate whose failure is invisible. Matching runs on
# $stripped (quoted segments removed), so a commit message cannot redirect it.
redirect_problem=""
target_dir=""
commit_segments=0
walking_dir=""
commit_segment=""
while IFS= read -r segment; do
    case "$segment" in *[![:space:]]*) ;; *) continue ;; esac
    segment=" $segment"          # so a segment-leading `cd`/`-C` still matches

    segment_cd=""
    case "$segment" in
        *[[:space:]]cd[[:space:]]*)
            segment_cd=$(printf '%s' "$segment" | sed -n 's/.*[[:space:]]cd[[:space:]]\{1,\}\([^[:space:]]\{1,\}\).*/\1/p') ;;
    esac

    if printf '%s' "$segment" | grep -qE "$commit_pattern"; then
        commit_segments=$((commit_segments + 1))
        case "$segment" in
            *--git-dir*|*--work-tree*|*GIT_DIR=*)
                redirect_problem="it points git at a repository through --git-dir, --work-tree or GIT_DIR=" ;;
        esac
        segment_c=$(printf '%s' "$segment" | sed -n 's/.*[[:space:]]-C[[:space:]]*\([^[:space:]]\{1,\}\).*/\1/p')
        if [ -n "$segment_c" ]; then
            # A relative -C is relative to where the walked `cd`s have got to, not to
            # this hook's own working directory. Resolving it here rather than joining
            # it was a silent bypass: from the admin repo, `cd nested && git -C . commit`
            # sized the ADMIN repo (nothing staged), found 0 lines, and waved a commit of
            # any size through. Reproduced at 9,675 staged lines before this was written.
            case "$segment_c" in
                /*) target_dir="$segment_c" ;;
                *)  target_dir="${walking_dir:+$walking_dir/}$segment_c" ;;
            esac
        else
            target_dir="$walking_dir"
            case "$segment" in
                *[[:space:]]-C*)
                    redirect_problem="its target directory is quoted or empty, so this gate cannot resolve it" ;;
            esac
        fi
    fi

    if [ -n "$segment_cd" ]; then
        case "$segment_cd" in
            /*) walking_dir="$segment_cd" ;;
            *)  walking_dir="${walking_dir:+$walking_dir/}$segment_cd" ;;
        esac
    fi
done <<SEGMENTS
$(printf '%s' "$stripped" | awk '{gsub(/&&|\|\||;|\|/, "\n"); print}')
SEGMENTS

# The same segment, as $placeheld sees it — the view names_paths needs. Walked separately
# rather than threaded through the loop above, because that loop resolves the TARGET and this
# one reads the ARGUMENTS: one question each, and neither has to know about the other.
while IFS= read -r segment; do
    case "$segment" in *[![:space:]]*) ;; *) continue ;; esac
    if printf '%s' " $segment" | grep -qE "$commit_pattern"; then
        commit_segment=" $segment"
    fi
done <<PLACEHELD_SEGMENTS
$(printf '%s' "$placeheld" | awk '{gsub(/&&|\|\||;|\|/, "\n"); print}')
PLACEHELD_SEGMENTS

# A subshell's working directory cannot be followed from out here, and two
# commits in one command cannot both be sized. Both are denied, not guessed.
case "$stripped" in
    *"("*) redirect_problem="it runs the commit inside a subshell, whose working directory this gate cannot follow" ;;
esac
if [ "$commit_segments" -gt 1 ]; then
    redirect_problem="it commits to more than one repository in one command — run one commit per command so each is sized against its own repo"
fi
case "$target_dir" in
    *'$'*) redirect_problem="its target directory is a shell variable this gate cannot expand" ;;
esac

if [ -z "$redirect_problem" ] && [ -n "$target_dir" ]; then
    git -C "$target_dir" rev-parse --show-toplevel >/dev/null 2>&1 || \
        redirect_problem="'$target_dir' is not inside a git repository"
fi
if [ -n "$redirect_problem" ]; then
    # Only marked repos get an opinion. With the target unresolvable, judge
    # participation by where we stand — an unmarked working directory stays
    # silent, which is what keeps kimi's globally-registered hooks scoped.
    # Accepted residual risk, code review F2: an unmarked cwd committing into a
    # marked repo through an unresolvable path is not gated. Bounded, because in
    # the intended working set the cwd is the admin repo, which is marked.
    stand="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
    participates "$stand" || exit 0
    deny "Blocked: this gate must size the diff of the repository being committed to, and $redirect_problem. Commit through 'git -C <repo> commit ...' with an unquoted path, or run the commit from inside <repo>. This is deliberate: a redirect the gate cannot resolve would otherwise measure the wrong repository, find nothing, and let an unreviewed commit land silently."
fi

top="$(git -C "${target_dir:-.}" rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$top"
# Repo guard: only repos that carry the adversarial-review skill participate.
[ -f .kimi-code/skills/adversarial-review/SKILL.md ] && : || \
[ -f .claude/skills/adversarial-review/SKILL.md ] || exit 0

changed_lines() {
    # numstat: insertions<TAB>deletions<TAB>path; "-" for binary counts as 0
    git diff ${1:-} --numstat 2>/dev/null | awk '{a += $1 + $2} END {print a + 0}'
}

# Does this command name paths? A commit that names paths takes the WORKING TREE version of
# them, whatever the index holds — so sizing the index finds 0 and lets any amount of unstaged
# work land. Reproduced at 800 changed lines before this was written, and naming a path is a
# routine invocation, not an exotic one. `--only`/`-o` is the same story.
#
# Tokens are walked rather than pattern-matched, because a flag's VALUE must not be mistaken
# for a path — `-F msg.txt` names no path, and reading it as one would size the whole working
# tree and deny an ordinary commit. The walk runs on $placeheld, where a quoted message is one
# opaque token, so a value can be consumed without swallowing a path that follows it.
names_paths() {
    saw_verb=0
    skip_next=0
    # Unquoted expansion is what splits the segment into tokens, but it also globs: a `*` in the
    # command would expand against this hook's own directory and the walk would no longer be
    # reading the command. Only ever a spurious deny, never a bypass — but the reasoning about
    # `--` and flag values is worth keeping true.
    set -f
    for token in $commit_segment; do
        if [ "$skip_next" -eq 1 ]; then skip_next=0; continue; fi
        if [ "$saw_verb" -eq 0 ]; then
            [ "$token" = "commit" ] && saw_verb=1
            continue
        fi
        case "$token" in
            --) return 0 ;;                 # everything after is a pathspec, by definition
            # --pathspec-from-file names paths just as surely as writing them out, in EVERY
            # spelling: attached (=FILE), separate (FILE), and from stdin (=-). Listing it among
            # the value-taking flags was worse than leaving it out — it made the separate form
            # swallow its own filename and report no paths at all. All three forms passed
            # silently at 400 unstaged lines. It must be answered before --*=* is reached.
            --pathspec-from-file|--pathspec-from-file=*) return 0 ;;
            --*=*) ;;                       # value is attached; consumes no later token
            -m|--message|\
            -F|--file|-c|--reedit-message|-C|--reuse-message|--author|--date|-t|--template|\
            --fixup|--squash|--cleanup) skip_next=1 ;;
            # A cluster of short flags whose LAST one takes a value: -qm, -sm, -vm, -am. Only
            # the last position can take one, and only a single-dash token can be a cluster —
            # matching long flags here would arm the skip for `--edit` and swallow the path
            # after it, which fails open. Without this, `git commit -qm "msg"` on a dirty tree
            # read the message as a path and denied an ordinary commit.
            -[!-]*[mFcCt]) skip_next=1 ;;
            -*) ;;                          # a flag that takes no value
            *) set +f; return 0 ;;          # a bare token: a path
        esac
    done
    set +f
    return 1
}

lines=$(changed_lines --cached)
# When this command also stages (`commit -a` or a `git add` in the same
# compound), what lands is the whole working tree vs HEAD — so size exactly
# that (`git diff HEAD`), not the max of staged-vs-HEAD and unstaged-vs-index,
# which undercounts when both exist (deion_assets review-1 F1, FortKnight
# review-1 F1). Without such staging, a pathspec/--only commit with unrelated
# local changes must not be gated by working-tree size (review-3 F2) — if we
# can't size what is being committed, pass silently.
case "$stripped" in
    *"git add"* | *" -a"* | *"--all"*)
        wt=$(changed_lines HEAD)
        [ "${wt:-0}" -gt "${lines:-0}" ] && lines=$wt
        ;;
esac
# A named path is sized against the whole working tree, not just that path. Picking the paths
# out of the command would mean trusting this walker's guess about which bare tokens are paths
# — and a wrong guess there sizes less than what lands, which is the direction that fails
# open. Over-counting only ever denies something that could have gone through, and a denial is
# visible and has two doors.
if [ -n "$commit_segment" ] && names_paths; then
    wt=$(changed_lines HEAD)
    [ "${wt:-0}" -gt "${lines:-0}" ] && lines=$wt
fi
[ "${lines:-0}" -eq 0 ] && exit 0
[ "${lines:-0}" -lt "$THRESHOLD" ] && exit 0

skip_door="A developer can skip the review, in writing, in the last lines of the commit message: 'Adversarial-Review-Skipped: <reason>' (more than ${SKIP_REASON_MINIMUM} characters — any reason qualifies, even 'just because') and 'Skip-Signed-Off-By: <their name>', passed with -m or -F <file> so this hook can read them (already typed them in an editor? retry with -F .git/COMMIT_EDITMSG; message files are read from $(pwd), so run the commit from there rather than through git -C). This hook cannot tell who typed those lines, which is exactly why you must not type them: writing them yourself forges a person's name into the permanent record of a review that never happened. Ask the user, use their words and their name, or run the review."

if [ -n "$skip_problem" ]; then
    reason="Skip rejected: ${skip_problem} The pending diff is ${lines} changed lines (threshold ${THRESHOLD}). ${skip_door} Or run the review: scripts/request-review.sh code review/<subject>/draft.diff --reviewer=${REVIEWER} (or --reviewer=gemini if ${REVIEWER} is out of credits or not on PATH; add --reviewer=codex, the artist, for a plan or diff with a screen or an asset in it), adjudicate every finding with the user, then retry prefixed ADV_REVIEWED=1."
else
    reason="Blocked by project convention: big commits get an adversarial CODE review before landing. The pending diff is ${lines} changed lines (threshold ${THRESHOLD}). Claim a subject directory — one review conversation — with mkdir review/<subject> (no -p: an existing one belongs to another session and is never touched), write the diff (git diff --cached > review/<subject>/draft.diff; use git diff if staging happens in the same command), run scripts/request-review.sh code review/<subject>/draft.diff --reviewer=${REVIEWER} (or --reviewer=gemini if ${REVIEWER} is out of credits or not on PATH; add --reviewer=codex, the artist, for a plan or diff with a screen or an asset in it), present and adjudicate every finding with the user, apply accepted fixes — then retry the commit with the command prefixed ADV_REVIEWED=1, which asserts the review HAPPENED and nothing else (if this exact diff was already reviewed this session, that counts). ${skip_door}"
fi

deny "$reason"
