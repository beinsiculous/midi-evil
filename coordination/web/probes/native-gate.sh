#!/bin/bash
# Runs the native screen-reader gate's mechanical half for W-A1a and W-A1b (plan § W-A1a and
# § W-A1b, Gate rows): the refusal rows first, then W0's whole sequence, the transport's step
# included, and its five edges, then R-1 to R-22, every sequence in a fresh application through
# run-probe.sh, and prints the transcript. Every
# step a row names carries an expectation, so a run that exits 0 is a run in which each key did
# what the row says. What keys cannot show from outside — which of two identical notes a Delete
# took, a lane point clicked with a pointer — the crate's unit and integration tests prove; those
# rows here show everything the keys can. Each row says which configuration it ran in: XWayland
# on a private bus. The person's half — the same keys with Orca, Wayland-native — is not this
# script's.
#
#   coordination/web/probes/native-gate.sh > native-gate-transcript.txt
#
# MIDI_EVIL_BINARY chooses the build, as for run-probe.sh. Rows without a fixture of their own
# open the tracked Bach invention, which starts on its first track: its first note is C 5 at bar 1
# beat 1.5, a sixteenth long, then B 4 at beat 1.75, in 4/4 at 64 beats per minute.
set -u

probes=$(cd "$(dirname "$0")" && pwd)
bach=$(cd "$probes/../../.." && pwd)/examples/public-domain/bach-invention-02/bach-invention-02.mid
failed=0
# A row opens the Bach invention unless run_with names a fixture for that row alone, whatever
# MIDI_EVIL_FILE the caller's environment holds.
row_fixture=
fixtures=$(mktemp -d /tmp/midi-evil-gate.XXXXXX) || exit 1
trap 'rm -rf -- "$fixtures"' EXIT
stand_in_runtime=

run() {
    local title=$1
    shift
    echo
    echo "################ $title  [XWayland, private bus]"
    if ! MIDI_EVIL_FILE=${row_fixture:-$bach} "$probes/run-probe.sh" --heading "$title" "$@"; then
        echo "################ FAILED: $title"
        failed=1
    fi
}

run_with() {
    local fixture=$1
    shift
    row_fixture=$fixtures/$fixture.evil run "$@"
}

# A project in 4/4 at 480 ticks a quarter; each argument after the name is one track, written
# "name|pitch,start,length;pitch,start,length".
fixture() {
    local name=$1
    shift
    local tracks=() instruments=() notes=() note_list=() index=0 track note pitch start length
    for track in "$@"; do
        notes=()
        IFS=';' read -ra note_list <<< "${track#*|}"
        for note in "${note_list[@]}"; do
            IFS=',' read -r pitch start length <<< "$note"
            notes+=("{\"pitch\":$pitch,\"velocity\":100,\"start_tick\":$start,\"duration_ticks\":$length}")
        done
        instruments+=("{\"name\":\"${track%%|*}\",\"source\":{\"type\":\"synth\",\"patch\":\"lead\"}}")
        tracks+=("{\"name\":\"${track%%|*}\",\"instrument\":$index,\"notes\":[$(IFS=,; echo "${notes[*]}")]}")
        index=$((index + 1))
    done
    cat > "$fixtures/$name.evil" << JSON
{"format":"midi-evil-project","version":2,"name":"$name","tempo_bpm":120,"beats_per_bar":4,
 "beat_unit":4,"ticks_per_quarter":480,"sample_rate":44100,
 "instruments":[$(IFS=,; echo "${instruments[*]}")],"tracks":[$(IFS=,; echo "${tracks[*]}")]}
JSON
}

fixture twins "Twins|60,0,480;60,0,480"
fixture bass "Bass|36,0,480;38,480,480;40,960,480;31,1440,480"
fixture two-tracks "one|60,0,480" "two|64,0,480"
fixture three-notes "Three|60,0,240;62,60,240;64,960,240"
fixture same-pitch-overlap "Overlap|60,0,960;60,480,480"
# Sixty-four one-beat notes at 120 beats a minute: half a minute of song, long enough to read the
# transport twice while it plays.
long_notes=$(for beat in $(seq 0 63); do echo -n "$((60 + beat % 12)),$((beat * 480)),480;"; done)
fixture long "Long|${long_notes%;}"
# A clean project at 90 beats a minute, so a tempo change and a new project each read differently.
fixture tempo "Tempo|60,0,480"
sed -i 's/"tempo_bpm":120/"tempo_bpm":90/' "$fixtures/tempo.evil"
# Two notes and one pitch-bend point, for the roll's Delete beside an automation lane.
cat > "$fixtures/lane-point.evil" << 'JSON'
{"format":"midi-evil-project","version":2,"name":"Lane point","tempo_bpm":120,"beats_per_bar":4,
 "beat_unit":4,"ticks_per_quarter":480,"sample_rate":44100,
 "instruments":[{"name":"Lead","source":{"type":"synth","patch":"lead"}}],
 "tracks":[{"name":"Lead","instrument":0,
  "notes":[{"pitch":60,"velocity":100,"start_tick":0,"duration_ticks":480},
           {"pitch":62,"velocity":100,"start_tick":480,"duration_ticks":480}],
  "pitch_bends":[{"tick":480,"value":0}]}]}
JSON

# F6 from nothing focused: the toolbar region, the side panel's stop (the selected track), then
# the editor's (the roll). No --focus anywhere: the roll is reached by keys, as a person would.
# The toolbar's stop is the transport readout.
reach_roll=(--key F6 --expect-region Toolbar --expect "Transport, stopped, "
    --key F6 --expect-region "Side panel" --key F6 --expect "Piano roll" --expect-region Editor)

################ the refusal rows
# The launcher's inner entry refuses before its first bus call unless every clause of its check
# holds. Each clause gets a row of its own in which every other clause is satisfied, so only the
# clause under test can refuse; a control row satisfies them all and must get past the check. A
# row for the probe's own check calls it without the launcher. The desktop's screen-reader flag
# and its settings files must be untouched by all of them. The stand-ins can reach nothing: the
# private bus has no display (:99), the application and registry are /bin/false, and the session
# bus named in the parent-clause row does not exist.
settings_state() {
    gsettings get org.gnome.desktop.a11y.applications screen-reader-enabled
    stat -c '%n %Y' "$HOME"/.config/dconf/* 2> /dev/null
}

refusal_row() {
    local title=$1 expected=$2 wanted_status=$3
    shift 3
    echo
    echo "################ $title  [desktop session; nothing reaches it]"
    local before after output status
    before=$(settings_state)
    output=$("$@" 2>&1 3>&1)
    status=$?
    after=$(settings_state)
    echo "$output" | grep -v "^ *event \|dbind-WARNING" | tail -3
    echo "exit status $status; the desktop's flag and settings files, before and after:"
    echo "$before"
    echo "$after"
    if [ "$status" -ne "$wanted_status" ] || [[ $output != *"$expected"* ]]; then
        echo "################ FAILED: $title (wanted status $wanted_status and '$expected')"
        failed=1
    elif [ "$before" != "$after" ]; then
        echo "################ FAILED: $title: the desktop's settings changed"
        failed=1
    fi
}

stand_in_runtime=$(mktemp -d /tmp/midi-evil-probe.XXXXXX) || exit 1
trap 'rm -rf -- "$fixtures" "$stand_in_runtime"' EXIT
echo stand-in-token > "$stand_in_runtime/isolation-token"
# Every clause satisfied, as run-probe.sh's outer half would set it, around a stand-in run.
isolated=(env GSETTINGS_BACKEND=memory DISPLAY=:99 XDG_RUNTIME_DIR="$stand_in_runtime"
    MIDI_EVIL_PRIVATE_RUNTIME="$stand_in_runtime" MIDI_EVIL_PROBE_TOKEN=stand-in-token
    MIDI_EVIL_DESKTOP_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS-}" MIDI_EVIL_X_DISPLAY=:99
    MIDI_EVIL_REGISTRY=/bin/false MIDI_EVIL_BINARY=/bin/false MIDI_EVIL_FILE=/dev/null)
inner=("$probes/run-probe.sh" --inside-private-bus --dump)

refusal_row "refusal: the inner entry with the desktop's own environment" \
    "refuses to run" 2 "${inner[@]}"
refusal_row "refusal control: every clause satisfied, so the check lets the run through" \
    "the application exited before probing" 1 "${isolated[@]}" dbus-run-session -- "${inner[@]}"
refusal_row "refusal clause: the settings backend is not the in-memory one" \
    "the settings backend is not the in-memory one" 2 \
    "${isolated[@]}" dbus-run-session -- env -u GSETTINGS_BACKEND "${inner[@]}"
refusal_row "refusal clause: the shell was not started by dbus-run-session" \
    "was not started by dbus-run-session" 2 \
    "${isolated[@]}" DBUS_SESSION_BUS_ADDRESS="unix:path=$stand_in_runtime/no-bus" "${inner[@]}"
# exec keeps the shell's process, so the inner entry's parent is still dbus-run-session.
refusal_row "refusal clause: the session bus is the desktop's" \
    "the session bus is not one dbus-run-session made" 2 \
    "${isolated[@]}" dbus-run-session -- sh -c \
    'MIDI_EVIL_DESKTOP_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS exec "$@"' sh "${inner[@]}"
refusal_row "refusal clause: the runtime directory is not this run's private one" \
    "the runtime directory is not this run's private one" 2 \
    "${isolated[@]}" dbus-run-session -- env MIDI_EVIL_PROBE_TOKEN=another-token "${inner[@]}"
refusal_row "refusal: the probe's own check, called without the launcher" \
    "atspi-probe: refuses to run" 1 python3 "$probes/atspi-probe.py" --dump

################ W0's sequence, the transport's step now included, and its five edges
run "W0 main sequence, with R-5: read the transport, read a note, change its pitch, start and length, delete it, leave and return" \
    --press "PIANO ROLL" \
    --key F6 --expect-region Toolbar \
    --expect "Transport, stopped, tempo 64 beats per minute, 4 4 time, bar 1 beat 1, position 0 minutes, 0 seconds" \
    --expect-description "Enter reads the transport again" \
    --key F6 --expect-region "Side panel" --key F6 --expect "Piano roll" --expect-region Editor \
    --key Right --expect "Piano roll, C 5, bar 1 beat 1.5, 0.25 beats, velocity 127" \
    --key Up --expect "Piano roll, C sharp 5, bar 1 beat 1.5," \
    --key ctrl+Right --expect "Piano roll, C sharp 5, bar 1 beat 1.75," \
    --key shift+Right --expect "C sharp 5, bar 1 beat 1.75, 0.5 beats" \
    --key shift+Left --expect "C sharp 5, bar 1 beat 1.75, 0.25 beats" \
    --key Delete --expect "Piano roll, C 5, bar 1 beat 2, " --expect-not "deleted" \
    --expect-announcement "Removed." \
    --key Tab --expect-not "Piano roll" \
    --key shift+Tab --expect "Piano roll, C 5, bar 1 beat 2, " --expect-not "deleted" \
    --expect-not "Removed" \
    --key i --expect-same-name \
    --expect-announcement "Transport, stopped, tempo 64 beats per minute, 4 4 time, bar 1 beat 1, position 0 minutes, 0 seconds"

run "W0 edge: Up on a note at pitch 127" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "C 5, bar 1 beat 1.5" \
    --key shift+Up --key shift+Up --key shift+Up --key shift+Up --expect "C 9," \
    --key shift+Up --expect "Piano roll, G 9, bar 1 beat 1.5" \
    --key Up --expect-same-name --expect-announcement "Cannot go higher." \
    --key Up --expect-same-name --expect-announcement "Cannot go higher."

run "W0 edge: Control+Left on a note at tick 0" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "C 5, bar 1 beat 1.5" \
    --key ctrl+Left --key ctrl+Left --expect "Piano roll, C 5, bar 1 beat 1, " \
    --key ctrl+Left --expect-same-name --expect-announcement "Cannot move earlier." \
    --key ctrl+Left --expect-same-name --expect-announcement "Cannot move earlier."

run "W0 edge: Enter then Delete in an empty pattern" \
    --press "+ Pattern part" --press "PIANO ROLL" "${reach_roll[@]}" \
    --expect "Piano roll, empty" \
    --key Enter --expect "Piano roll, C 4, pattern bar 1 beat 1, 1 beat, velocity 100" \
    --expect-announcement "Added." \
    --key Delete --expect "Piano roll, empty" --expect-announcement "Removed." \
    --key Delete --expect-same-name --expect-announcement "Nothing here yet." \
    --key Delete --expect-same-name --expect-announcement "Nothing here yet."

run "W0 edge: Enter after the last note of a full shared pattern" \
    --press "+ Pattern part" --press "Duplicate loop" --press "PIANO ROLL" "${reach_roll[@]}" \
    --expect "Piano roll, empty" \
    --key Enter --key Enter --key Enter \
    --key Enter --expect "Piano roll, C 4, pattern bar 1 beat 4, 1 beat, velocity 100" \
    --expect-announcement "Added to 2 parts." \
    --key Enter --expect-same-name --expect-announcement "Nothing added: the pattern ends here." \
    --key Enter --expect-same-name --expect-announcement "Nothing added: the pattern ends here."

# Clean overlaps sorts by start and then by the notes' old order, so it re-sorts only a note that
# moved before one stored ahead of it: B 4, stored after C 5, moved two snaps earlier lands before
# C 5's start, and the clean moves it to the front of the list. The cursor must follow it, and the
# delete must take B 4, which leaves C 5 as the first note. The status line saying the notes were
# edited shows the clean changed the list.
run "W0 edge, and R-17's first fixture: select a note, Clean overlaps re-sorts it, return, delete the same note" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --key Right --expect "Piano roll, B 4, bar 1 beat 1.75, " \
    --key ctrl+Left --key ctrl+Left --expect "Piano roll, B 4, bar 1 beat 1.25, " \
    --press "Clean overlaps" --expect-node "Unique timeline notes edited" \
    --key F6 --key F6 --key F6 --expect "Piano roll, B 4, bar 1 beat 1.25, " \
    --key Delete --expect-announcement "Removed." --expect "Piano roll, C 5, bar 1 beat 1.5, " \
    --key Left --expect-same-name --expect-announcement "Nothing before this one."

################ the rows found by the review of W0
run_with twins "R-1: two identical notes; the inspector changes one's velocity over several frames; the cursor never moves to the twin" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100" \
    --set-value Velocity:99 --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 99" \
    --set-value Velocity:98 --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 98" \
    --set-value Velocity:97 --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 97" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100" \
    --key Left --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 97"

run "R-2: F6 to the roll and an arrow right after it (the very next frame: the crate's tests)" \
    --press "PIANO ROLL" --burst "F6 F6 F6 Right" \
    --expect "Piano roll, C 5, bar 1 beat 1.5" --expect-region Editor \
    --key Right --expect "Piano roll, B 4, bar 1 beat 1.75"

run_with bass "R-3: a bass part; F6 from the roll reaches the toolbar and Shift+F6 returns" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 2, bar 1 beat 1" \
    --key F6 --expect-region Toolbar --expect-not "Piano roll" \
    --key shift+F6 --expect "Piano roll, C 2, bar 1 beat 1" --expect-region Editor

# Two controls are named Automation: the Notes view's toggle and the side panel's button, which
# always shows the lanes; the row presses the side panel's by its region. Whether a clicked lane
# point is what Delete takes is the crate's tests'
# (the_automation_lane_takes_delete_only_with_focus_which_a_click_gives_it and its mixer twin).
run_with lane-point "R-4: the roll focused beside an automation lane with a point; Delete takes the note, the point stays" \
    --press "PIANO ROLL" --press-in "Side panel|Automation" "${reach_roll[@]}" \
    --expect-node "Automation lane, Pitch bend, 1 point" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1" \
    --key Delete --expect "Piano roll, D 4, bar 1 beat 2" --expect-announcement "Removed." \
    --expect-node "Automation lane, Pitch bend, 1 point"

run "R-6: in an empty pattern, add a note and undo the add: the selection was reset" \
    --press "+ Pattern part" --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Enter --expect "Piano roll, C 4, pattern bar 1 beat 1" \
    --key ctrl+z --expect "Piano roll, empty" --expect-announcement "The selection was reset."

run "R-7: Right held across twenty notes, and Up held at the top" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --hold Right:19 --expect-hold-name-changes 0 --expect-release-name-changes 1 \
    --expect "Piano roll, D 5, bar 2 beat 2.25" \
    --key shift+Up --key shift+Up --key shift+Up --key shift+Up --key shift+Up \
    --expect "Piano roll, G 9, " \
    --hold Up:5 --expect-announcement-count "Cannot go higher.:1" \
    --expect-hold-name-changes 0 --expect-release-name-changes 0 \
    --hold Up:5 --expect-announcement-count "Cannot go higher.:1"

run "R-8: delete the last track from the Notes view; F6's editor stop is the add-a-track button" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --press "Track actions" --press "Delete track" \
    --press "Track actions" --press "Delete track" \
    --key shift+F6 --expect "+ Add a track to begin composing" --expect-region Editor

run_with twins "R-9: identical notes; select the first, add after it, undo: back on the first (which twin Delete takes: the crate's tests)" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 1 beat" \
    --key Enter --expect "Piano roll, C 4, bar 1 beat 2, 1 beat" --expect-announcement "Added." \
    --key ctrl+z --expect "Piano roll, C 4, bar 1 beat 1, 1 beat" \
    --key Delete --expect "Piano roll, C 4, bar 1 beat 1, 1 beat" --expect-announcement "Removed." \
    --key Delete --expect "Piano roll, empty" --expect-announcement "Removed."

run "R-10: the roll's description names every chord of the model" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --expect-description "Left previous note" --expect-description "Right next note" \
    --expect-description "Home first note" --expect-description "End last note" \
    --expect-description "Up a semitone higher" --expect-description "Down a semitone lower" \
    --expect-description "Shift+Up an octave higher" \
    --expect-description "Shift+Down an octave lower" \
    --expect-description "Control+Left moves the note earlier" \
    --expect-description "Control+Right moves the note later" \
    --expect-description "Shift+Left shorter" --expect-description "Shift+Right longer" \
    --expect-description "Enter adds a note" --expect-description "Delete removes the note" \
    --expect-description "Escape clears the selection" \
    --expect-description "I reads the transport" --expect-description "F6 and Shift+F6"

# AccessKit's AT-SPI adapter reports no disabled state for a button (it maps disabled only for
# roles that can be read-only), so the disabled one-note buttons are shown by pressing them and
# getting nothing; their disabled state itself is asserted in the crate's tests
# (the_one_note_buttons_edit_the_cursors_note_and_are_disabled_without_one).
run "R-14: every one-note edit from its toolbar button, with the roll focused" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 5, bar 1 beat 1.5" \
    --press "Later" --expect "Piano roll, C 5, bar 1 beat 1.75" \
    --press "Later" --expect "Piano roll, C 5, bar 1 beat 2, " \
    --key Left --expect "Piano roll, B 4, bar 1 beat 1.75" \
    --key Right --expect "Piano roll, C 5, bar 1 beat 2, " \
    --press "Earlier" --expect "Piano roll, C 5, bar 1 beat 1.75" \
    --press "Longer" --expect "Piano roll, C 5, bar 1 beat 1.75, 0.5 beats" \
    --press "Shorter" --expect "Piano roll, C 5, bar 1 beat 1.75, 0.25 beats" \
    --press "Add" --expect "Piano roll, C 5, bar 1 beat 2, 1 beat" --expect-announcement "Added." \
    --press "Delete" --expect-announcement "Removed." \
    --key Escape --expect "Piano roll, no note selected" \
    --press "Earlier" --expect-same-name --expect-announcement-count "Select a note first.:0" \
    --press "Delete" --expect-same-name --expect-announcement-count "Removed.:0"

run_with two-tracks "R-15: duplicate a track, select the copy's note, undo the duplication" \
    --press "PIANO ROLL" --press "Track actions" --press "Duplicate track" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1" \
    --key ctrl+z --expect "Piano roll, no note selected" \
    --expect-announcement "The selection was reset." \
    --key Delete --expect-announcement "Select a note first." \
    --press "02   two" --key F6 --key F6 --key F6 --key Right --expect "Piano roll, E 4, bar 1 beat 1"

run_with three-notes "R-16: cursor on the first of three; Control+Right past the second; undo" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, " \
    --key ctrl+Right --expect "Piano roll, C 4, bar 1 beat 1.25, " \
    --key ctrl+z --expect "Piano roll, C 4, bar 1 beat 1, " \
    --key Right --expect "Piano roll, D 4, bar 1 beat 1.125, "

run_with same-pitch-overlap "R-17, second fixture: a same-pitch overlap that Clean shortens; the cursor follows by start and pitch" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 2 beats" \
    --press "Clean overlaps" --expect-node "Unique timeline notes edited" \
    --key F6 --key F6 --key F6 --expect "Piano roll, C 4, bar 1 beat 1, 1 beat"

# --burst sends its chords back to back, but nothing outside the application can show they were
# read in one frame. The one-frame ordering itself is proven by the crate's tests, which feed one
# frame by construction (two_keys_in_one_frame_act_in_the_order_they_arrived,
# an_arrow_in_the_frame_after_f6_moves_the_cursor_and_focus_stays); these rows show the net result.
run "R-18: three orders of two keys sent back to back (one frame: the crate's tests)" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 5, bar 1 beat 1.5" \
    --burst "Delete ctrl+z" --expect "Piano roll, C 5, bar 1 beat 1.5" \
    --key Up --expect "Piano roll, C sharp 5, bar 1 beat 1.5" \
    --burst "ctrl+z Delete" --expect "Piano roll, B 4, bar 1 beat 1.75" \
    --expect-announcement "Removed." \
    --burst "Right Delete" --expect-announcement "Removed." --expect-not "C 5, bar 1 beat 2, "

run "R-19: Escape clears the cursor and says operations apply to all notes; again, it leaves" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 5, bar 1 beat 1.5" \
    --key Escape --expect "Piano roll, no note selected" \
    --expect-announcement "No note selected; Quantize, transpose and Copy apply to all notes." \
    --press "Quantize" --expect-node "Unique timeline notes edited" \
    --key Escape --expect-not "Piano roll"

run "R-20: start a take with the cursor on a note, undo during it" \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --key Right --expect "Piano roll, B 4, bar 1 beat 1.75" \
    --press "Record" --key i --expect-announcement "Transport, recording, " \
    --key ctrl+z --expect "Piano roll, B 4, bar 1 beat 1.75" \
    --key i --expect-announcement "Transport, stopped, "

################ the rows of W-A1b
# What --expect-quiet counts, checked without an application: a description or value change on
# the focused node fails it, and every change it counts is one the probe listens for.
echo
echo "################ probe self-check: what a quiet window counts  [no application]"
if ! python3 "$probes/probe_self_check.py"; then
    echo "################ FAILED: probe self-check"
    failed=1
fi

# Space is the transport's everywhere a focused widget does not act on it, the readout included;
# Enter on the readout reads it. The application plays on a silent clock here (run-probe.sh), so
# the readout can be read while the song plays.
run_with long "R-11: read the transport stopped, play, read it playing, five silent seconds on it, read again, stop, read; with focus elsewhere its name keeps up" \
    --key F6 --expect "Transport, stopped, tempo 120 beats per minute" \
    --key Enter --expect-same-name --expect-announcement "Transport, stopped, " \
    --key space --expect-same-name \
    --key-until "Enter|Transport, playing" --expect "Transport, playing, " \
    --expect-quiet 5 \
    --key Enter --expect "Transport, playing, " --expect-changed-name \
    --key space --expect-same-name \
    --key Enter --expect "Transport, stopped, " \
    --key space --key-until "Enter|Transport, playing" --expect "Transport, playing, " \
    --key Tab --expect-not "Transport" \
    --pause 2 --expect-unfocused-name-change "Transport, playing, "

# The Notes view without a cursor and with one, so the note inspector's values show; then the
# Song view with a part selected, so its part inspector shows. Scroll bars are checked with the
# focusable nodes, and the side panel's resize handle is one of those.
run "R-12: no focusable node and no scroll bar unnamed, on the Notes view and the Song view" \
    --expect-node "MIDI Evil" --expect-node "Side panel width" \
    --press "PIANO ROLL" --expect-all-named \
    "${reach_roll[@]}" --key Right --expect "Piano roll, C 5" --expect-node "Velocity" \
    --expect-all-named \
    --press "SONG" --press "+ Pattern part" --expect-node "Pattern name" --expect-all-named \
    --dump

run "R-13: a screen reader started after the application: absent until the flag turns on, then the whole tree" \
    --screen-reader-after-launch \
    --key F6 --expect "Transport, stopped, " --expect-all-named

# Saved before New, so New has nothing to ask about: its question is a native dialog that keys
# sent to the application's own window cannot answer.
run_with tempo "R-21: with the readout focused, change the tempo and read; a new project, and read" \
    --key F6 --expect "Transport, stopped, tempo 90 beats per minute" \
    --set-value "Tempo in beats per minute:100" --expect-same-name \
    --key Enter --expect "Transport, stopped, tempo 100 beats per minute" \
    --key ctrl+s --key ctrl+n --expect-same-name \
    --key Enter --expect "Transport, stopped, tempo 120 beats per minute" --expect-region Toolbar

run "R-22: a disabled button says why, and says nothing once enabled (toolbar, side panel, Notes view)" \
    --expect-node-description "Undo|Nothing to undo." \
    --expect-node-description "Redo|Nothing to redo." \
    --press "PIANO ROLL" --expect-node-description "Earlier|Select a note first." \
    --expect-node-description "Delete|Select a note first." \
    "${reach_roll[@]}" --key Right --expect "Piano roll, C 5" \
    --expect-node-description "Earlier|" --expect-node-description "Delete|" \
    --key ctrl+Right --expect-node-description "Undo|" \
    --press "Record" --expect-node-description "Add a synth track|Not while recording." \
    --expect-node-description "Track actions|Not while recording." \
    --expect-node-description "Next sound|Not while recording." \
    --press "Stop take" --expect-node-description "Add a synth track|" \
    --expect-node-description "Next sound|"

run "R-22 on the Song view: a selected part's buttons while recording and after" \
    --press "+ Pattern part" --expect-node-description "Delete part|" \
    --press "Record" --expect-node-description "Delete part|Not while recording." \
    --expect-event "object:property-change:accessible-description" \
    --expect-node-description "+ Pattern part|Not while recording." \
    --press "Stop take" --expect-node-description "Delete part|"

run "The instrument list: Alt+Right and Alt+Left announce the sound a track now has and its place" \
    --expect-node "Sound: Lead, 7 of 12 synth sounds" \
    --key F6 --key alt+Right --expect-announcement " 8 of 12 synth sounds" \
    --expect-node "Sound: " --expect-same-name \
    --key alt+Left --expect-announcement "Lead, 7 of 12 synth sounds"

echo
if [ "$failed" -ne 0 ]; then
    echo "################ native gate: FAILED"
    exit 1
fi
echo "################ native gate: every sequence passed"
