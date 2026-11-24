#!/bin/bash
# Runs the native screen-reader gate's mechanical half (plan § W0-native, Gate): the main
# sequence, then each edge, every one in a fresh application through run-probe.sh, and prints
# the transcript. Every step that the gate names is checked with --expect, so a run that exits 0
# is a run in which each key did what the gate says it must. The person's half — the same keys
# with Orca — is not this script's.
#
#   coordination/web/probes/native-gate.sh > native-gate-transcript.txt
#
# MIDI_EVIL_BINARY chooses the build, as for run-probe.sh. The sequences assume the tracked Bach
# fixture, which opens on its first track: its first note is C 5 at bar 1 beat 1.5, a sixteenth
# long, and it is in 4/4, so a new one-bar pattern holds four one-beat notes.
set -u

probes=$(cd "$(dirname "$0")" && pwd)
failed=0

run() {
    local title=$1
    shift
    echo
    echo "################ $title"
    if ! "$probes/run-probe.sh" --heading "$title" "$@"; then
        echo "################ FAILED: $title"
        failed=1
    fi
}

# F6 from nothing focused: the toolbar's stop (the transport readout), the side panel's (the
# selected track), then the editor's (the roll). No --focus anywhere: the roll is reached by keys.
reach_roll=(--key F6 --expect "Transport, " --key F6 --expect "01 " --key F6 --expect "Piano roll")

run "main sequence: focus the roll, read a note, change its pitch and start, delete it, read the transport" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 5, bar 1 beat 1.5, 0.25 beats, velocity 127" \
    --key Up --expect "Piano roll, C sharp 5, bar 1 beat 1.5," \
    --key ctrl+Right --expect "Piano roll, C sharp 5, bar 1 beat 1.75," \
    --key shift+Right --expect "C sharp 5, bar 1 beat 1.75, 0.5 beats" \
    --key shift+Left --expect "C sharp 5, bar 1 beat 1.75, 0.25 beats" \
    --key Delete --expect "Piano roll, deleted C sharp 5 at bar 1 beat 1.75, " \
    --expect-announcement "Removed." \
    --key Tab --expect-not "Piano roll" \
    --key shift+Tab --expect "Piano roll" \
    --key F6 --expect "Transport, stopped, tempo 64 beats per minute, position 0 minutes, 0 seconds" \
    --key shift+F6 --expect "Piano roll"

run "edge: Up on a note at pitch 127" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "C 5, bar 1 beat 1.5" \
    --key shift+Up --key shift+Up --key shift+Up --key shift+Up --expect "C 9," \
    --key shift+Up --expect "Piano roll, G 9, bar 1 beat 1.5" \
    --key Up --expect "Piano roll, already at the top, G 9," \
    --expect-announcement "Cannot go higher." \
    --key Up --expect-same-name --expect-announcement "Cannot go higher."

run "edge: Ctrl+Left on a note at tick 0" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "C 5, bar 1 beat 1.5" \
    --key ctrl+Left --key ctrl+Left --expect "Piano roll, C 5, bar 1 beat 1, " \
    --key ctrl+Left --expect "Piano roll, already at the start, C 5, bar 1 beat 1, " \
    --expect-announcement "Cannot move earlier." \
    --key ctrl+Left --expect-same-name --expect-announcement "Cannot move earlier."

run "edge: Enter then Delete in an empty pattern" \
    --press "+ Pattern part" --press "PIANO ROLL" "${reach_roll[@]}" \
    --expect "Piano roll, empty" \
    --key Enter --expect "Piano roll, added, C 4, pattern bar 1 beat 1, 1 beat, velocity 100; used by 1 part" \
    --key Delete --expect "Piano roll, deleted C 4 at pattern bar 1 beat 1; used by 1 part, empty" \
    --expect-announcement "Removed." \
    --key Delete --expect "Piano roll, empty" --expect-announcement "Nothing here yet." \
    --key Delete --expect-same-name --expect-announcement "Nothing here yet."

run "edge: Enter after the last note of a full shared pattern" \
    --press "+ Pattern part" --press "Duplicate loop" --press "PIANO ROLL" "${reach_roll[@]}" \
    --expect "Piano roll, empty" \
    --key Enter --key Enter --key Enter \
    --key Enter --expect "Piano roll, added, C 4, pattern bar 1 beat 4, 1 beat, velocity 100; shared by 2 parts" \
    --key Enter --expect "Piano roll, no room, C 4, pattern bar 1 beat 4, " \
    --expect-announcement "Nothing added: the pattern ends here." \
    --key Enter --expect-same-name --expect-announcement "Nothing added: the pattern ends here."

# Moving C 5 onto B 4's onset makes Clean overlaps re-sort the two, so the selected note's index
# changes under the cursor: the delete must still take C 5. The status line saying the notes were
# edited is what shows the clean changed something; without it this edge would pass on a no-op.
run "edge: select a note, Clean overlaps, return to the roll, delete the same note" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --key ctrl+Right --expect "Piano roll, C 5, bar 1 beat 1.75, " \
    --press "Clean overlaps" --expect-node "Unique timeline notes edited" \
    --key F6 --key F6 --key F6 --expect "Piano roll, C 5, bar 1 beat 1.75, " \
    --key Delete --expect "Piano roll, deleted C 5 at bar 1 beat 1.75, "

echo
if [ "$failed" -ne 0 ]; then
    echo "################ native gate: FAILED"
    exit 1
fi
echo "################ native gate: every sequence passed"
