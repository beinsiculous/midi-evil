# The native gate's rows for the arranger by keyboard, the track panel, the file commands, every
# command's announced outcome and the record's nine steps. Sourced by native-gate.sh, whose run,
# run_with, reach_roll, fixtures and failed it uses; it is not run on its own. A row that
# only presses starts with F6, so that it waits, as every key does, for the window to have focus:
# a window without it may not draw the frame an outcome is announced in.

# On the Song view F6 from nothing focused reaches the toolbar, the side panel, then the arranger.
reach_song=(--key F6 --expect-region Toolbar --key F6 --expect-region "Side panel"
    --key F6 --expect "Song, track 1 of " --expect-region Editor)

# The record's nine steps, keyboard only. The ninth, save, is the next two rows: a new project
# has no file, so its Save is Save As, whose dialog is the desktop's own window, which keys sent
# to the application's window cannot drive. Open from the menu and Save As on a new project are
# therefore not covered here; they are the sitting's alone, under Jesse's written waiver of
# 2026-11-24 (plan, What is asked of Jesse). Ctrl+N asks nothing here because the project the
# row opens has nothing unsaved.
run_with clean "Nine steps, 1 to 8: a new project, add an instrument, discover the instruments, add a note, select it and hear its pitch and timing, change it, delete it, read the transport" \
    --expect-node "Opened clean.evil, 1 track." \
    --key ctrl+n --expect-announcement "New project, 1 track." \
    --press "Add a synth track" \
    --expect-announcement "Added Track 2, a synth track with the Lead sound." \
    --expect-node "Sound: Lead, 7 of 12 synth sounds" \
    --key F6 --expect-region Toolbar \
    --key alt+Right --expect-announcement " 8 of 12 synth sounds" \
    --key alt+Left --expect-announcement "Lead, 7 of 12 synth sounds" \
    --press "PIANO ROLL" --key F6 --key F6 --expect "Piano roll, empty" --expect-region Editor \
    --key Enter --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100" \
    --expect-announcement "Added." \
    --key Escape --expect "Piano roll, no note selected" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 1 beat, velocity 100" \
    --key Up --expect "Piano roll, C sharp 4, bar 1 beat 1, 1 beat" \
    --key Delete --expect "Piano roll, empty" --expect-announcement "Removed." \
    --key i --expect-announcement "Transport, stopped, tempo 120 beats per minute, 4 4 time"

# Save writes to the file the project was opened from: no dialog. The file is the gate's private
# fixture, which the next row opens again; the directory is removed when the gate exits.
run_with saved "Nine steps, 9, and files: opened by launch argument, an edit, Control+S, and hear it saved" \
    --expect-node "Opened saved.evil, 1 track." \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C 4, bar 1 beat 1, 1 beat" \
    --key Up --expect "Piano roll, C sharp 4, bar 1 beat 1, 1 beat" \
    --key ctrl+s --expect-announcement "Saved saved.evil." --expect-node "• SAVED"

run_with saved "Files: relaunched on the saved file, the edit is there" \
    --expect-node "Opened saved.evil, 1 track." \
    --press "PIANO ROLL" "${reach_roll[@]}" \
    --key Right --expect "Piano roll, C sharp 4, bar 1 beat 1, 1 beat"

# A launch's own announcement goes out before any listener is on the bus, so the row that hears a
# project load is a second launch: it hands its file to the running application, as opening a
# project from a file manager does, while the probe listens; the step waits until the load is
# heard.
run_with clean "Files: open an existing project and hear that it loaded" \
    --open-in-running "$fixtures/arrangement.evil" \
    --expect-announcement "Opened arrangement.evil, 2 tracks." \
    "${reach_song[@]}" --expect "Song, track 1 of 2, Lead, sound Lead"

run_with arrangement "Song: move between tracks and parts, hear a placement, move a part, start playback at its bar, read the transport, undo, open it in the roll" \
    "${reach_song[@]}" \
    --expect "Song, track 1 of 2, Lead, sound Lead, unique timeline, empty, 2 parts" \
    --expect-description "Left previous part" --expect-description "Control+Right moves the part a bar later" \
    --key Right --expect "part 1 of 2, Riff, bar 1 beat 1, 1 bar, shared by 2 parts" \
    --key Right --expect "part 2 of 2, Riff, bar 3 beat 1, 1 bar" \
    --key Right --expect-same-name --expect-announcement "Nothing after this one." \
    --key ctrl+Right --expect "part 2 of 2, Riff, bar 4 beat 1" \
    --key ctrl+z --expect "part 2 of 2, Riff, bar 3 beat 1" \
    --expect-announcement "Undid the last edit." \
    --key Down --expect "Song, track 2 of 2, Bass, sound Lead, unique timeline, 1 note, no parts" \
    --key Down --expect-same-name --expect-announcement "No track below." \
    --key Up --expect "Song, track 1 of 2, Lead, sound Lead, unique timeline" \
    --key End --expect "part 2 of 2, Riff, bar 3 beat 1" \
    --key b --expect-same-name --expect-announcement "Playback starts at bar 3." \
    --key i --expect-announcement "Transport, stopped, tempo 120 beats per minute, 4 4 time, bar 3 beat 1" \
    --key Enter --expect "Piano roll" --expect-region Editor \
    --expect-announcement "Riff of Lead, in the piano roll." \
    --key Right --expect "Piano roll, C 4, pattern bar 1 beat 1, 1 beat" \
    --press "Arranger" --expect-announcement "The arranger is shown; F6 reaches it."

run_with arrangement "Song: Delete removes the part under the cursor and says so; the cursor goes to the next part" \
    "${reach_song[@]}" --key Right --expect "part 1 of 2, Riff, bar 1 beat 1" \
    --key Delete --expect-announcement "Removed." --expect "part 1 of 1, Riff, bar 3 beat 1" \
    --expect-not "Removed" \
    --key Home --key Delete --expect-same-name --expect-announcement "Select a part first."

# Which part the take records into is held by index; that it is unchanged is the crate's test
# (during_a_take_delete_on_a_part_removes_nothing_says_why_and_leaves_the_takes_target). Here the
# part is still there, the take still running, and the refusal heard.
run_with arrangement "Recording guard: during a take, Delete on a part removes nothing and says why; so does a nudge" \
    "${reach_song[@]}" --key Right --expect "part 1 of 2, Riff, bar 1 beat 1" \
    --press "Record" --expect-announcement "Recording loop overdub into the selected shared pattern." \
    --key Delete --expect-same-name --expect-announcement "Not while recording." \
    --key ctrl+Right --expect-same-name --expect-announcement "Not while recording." \
    --expect "part 1 of 2, Riff, bar 1 beat 1, 1 bar, shared by 2 parts" \
    --key i --expect-announcement "Transport, recording, " \
    --press "Stop take" --expect-announcement "Recorded 0 notes and 0 controller events"

run_with arrangement "Song: every arranger button says what it did" \
    "${reach_song[@]}" --key Right --expect "part 1 of 2, Riff, bar 1 beat 1" \
    --press "Loop part" --expect-announcement "Looping from bar 1 beat 1 to bar 2 beat 1." \
    --press "Duplicate shared" --expect-announcement "Duplicated the part; 3 parts now play Riff." \
    --expect "part 2 of 3, Riff, bar 2 beat 1" \
    --press "Make unique" --expect-announcement "This part now plays its own pattern, Riff Unique." \
    --press "Merge to timeline" --expect-announcement "Merged into the unique timeline." \
    --press "+ Pattern part" --expect-announcement "Added a part playing the new Pattern 3." \
    --press "Delete part" --expect-announcement "Removed." \
    --press "Duplicate loop" --expect-announcement "Duplicated " \
    --press "Unique timeline" --expect "Piano roll" \
    --expect-announcement "The unique timeline of Lead, in the piano roll."

# Clean overlaps says what it did in R-17's second row.
run_with three-notes "The roll's toolbar: Quantize, a transpose, Copy and Paste each say what they did" \
    --key F6 --expect-region Toolbar \
    --press "PIANO ROLL" --press "Quantize" --expect-announcement "Quantized all 3 notes." \
    --press "+12" --expect-announcement "Moved all 3 notes an octave up." \
    --press "Copy" --expect-announcement "Copied 3 notes." \
    --press "Paste" --expect-announcement "Pasted."

run_with clean "Track panel: rename, mute, re-assign a sound, each said once" \
    --key F6 --expect-region Toolbar --focus "Track name" --key x --expect-announcement-count "Renamed:0" \
    --key Tab --expect-announcement "Renamed Clean to " \
    --press "Mute track" --expect-announcement "muted." \
    --expect-node "Sound: Lead, 7 of 12 synth sounds" \
    --press "Mute track" --expect-announcement "unmuted." \
    --press "Sound: Lead, 7 of 12 synth sounds; opens the sound library" \
    --expect-announcement "The Sound library window is open" \
    --press "Bass   ·   FACTORY" --expect-announcement " assigned to " \
    --expect-node "Sound: Bass, "

run_with two-tracks "Track list: duplicate, move down and up, delete, and add a drum track, each said" \
    --key F6 --expect-region Toolbar --press "Track actions" --press "Duplicate track" \
    --expect-announcement "Duplicated as one Copy, track 2 of 3." \
    --press "Track actions" --press "Move down" --expect-announcement "Moved one Copy to track 3 of 3." \
    --press "Track actions" --press "Move up" --expect-announcement "Moved one Copy to track 2 of 3." \
    --press "Track actions" --press "Delete track" \
    --expect-announcement "Removed one Copy; 2 tracks left." \
    --press "Add a Maxwell drum-kit track" \
    --expect-announcement "Added Drums 3, a drum track with the Maxwell kit sound."

run_with clean "Windows and the Project menu without a dialog: Keyboard notes, Recover rhythm from WAV, About's Start composing" \
    --key F6 --expect-region Toolbar --press "Project" --press "Keyboard notes…" \
    --expect-announcement "The Keyboard notes window is open; Tab reaches it." \
    --expect-node "Keyboard notes, one chord a line" --expect-all-named \
    --press "Project" --press "Recover rhythm from WAV…" \
    --expect-announcement "The Recover rhythm from WAV window is open; Tab reaches it." \
    --press "About MIDI Evil" --expect-announcement "The About window is open; Tab reaches it." \
    --expect-node "Dr. Maxwell's studio" --expect-all-named \
    --press "Start composing" --expect-announcement "The piano roll is open."

# The last line: native-gate.sh checks it ran, so a file that stopped early is a failed gate.
arranger_and_files_rows_ran=1
