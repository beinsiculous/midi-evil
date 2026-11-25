# W-A1 inventory — every control on the shown surfaces

Every control, menu item, window and toggle a person can reach on the surfaces the web build will
show: the Song and Notes views, the toolbar (its three rows, the sound bar and the export row),
the side panel, the Project menu, the About window, the sound library, and the two windows the
Project menu opens without a dialog (Keyboard notes, Recover rhythm from WAV). W6's inventory gate
is checked against this list (D12).

**How it was taken.** Tree dumps of each surface through `coordination/web/probes/run-probe.sh
--dump` on branch `wa1c` (the Song view with a part selected, the Notes view, the Project menu
open, the export row shown, the Track actions and + Track menus open, the About window and the
sound library open), reconciled against every `Sense::click`, `Sense::drag`,
`Sense::click_and_drag`, `Sense::CLICK` and `ui.interact` site in the shown views' modules
(`arranger.rs`, `arranger/*`, `piano_roll.rs`, `piano_roll/*`, `app/chrome.rs`,
`app/transport_readout.rs`, `app/track_rack.rs`, `app/sounds.rs`, `app/notes_view.rs`,
`midi_input.rs`, `recovery.rs`) — the section *Pointer gestures* below. Labels that only show text
(`TEMPO`, `SELECTED TRACK`, the status line) are not controls and are not listed; the status line
is where every announced outcome is also shown.

**How a line ends.** Each line ends one of three ways, or fails:

- **A** — an announced outcome, with the gate row (`native-gate.sh`, by its title's start) or the
  crate test that shows it. Every announced outcome goes through `StudioApp::report` (or
  `report_after_focus` when focus moves first), which writes the status line and the announcer
  from one call; the roll's mirror of its name in the status line is not an outcome and is not
  announced (`a_roll_key_is_announced_once_though_the_status_line_repeats_the_rolls_name`).
- **H** — opens a held-back surface: hidden in the web build (D12), its absence asserted in W6.
  The held-back surfaces are the Mixer, Synth, Keyboard and Pads views and the Automation panel.
- **S** — a named standard control; activating it changes nothing a screen reader must be told
  beyond the control's own state, which egui exposes (a value, checked, selected, expanded).
- **F** — a failing line: none of the three. Every one is listed again at the end.

Gate rows are named by the start of their title. Tests are in `crates/midi-evil/src`.

## Toolbar, first row

| control (as named in the tree) | role | ends | evidence |
|---|---|---|---|
| About MIDI Evil | button | A | "The About window is open; Tab reaches it." — gate *Windows and the Project menu* |
| Project | menu button | S | opens the Project menu, named "Project menu" (`every_open_menu_and_window_on_the_shown_surfaces_is_named_too`) |
| Save project | button | A | the same command as Ctrl+S: "Saved song.evil." — gate *Nine steps, 9* |
| PLAY / STOP / CANCEL | button | S | the transport's state is read from the readout, on demand (R-11); Space is the same command |
| Record / Stop take | toggle button | A | "Recording loop overdub into the selected shared pattern." / "Recorded 0 notes and 0 controller events; Undo removes the take." — gate *Recording guard* |
| Transport, … (the readout) | status | A | Enter reads it — R-11, R-21 |
| Project name | text field | S | |
| Tempo in beats per minute | value box | S | R-21 |
| Meter, beats per bar over 4 | value box | S | |

## Toolbar, second row

| control | role | ends | evidence |
|---|---|---|---|
| Start bar | value box | S | also the key path of the ruler's click for any bar |
| Loop end bar | value box | S | |
| Repeat section | check box | S | |
| Quantize recording to 1/16 | check box | S | |
| MIDI input port | combo box | S | its unsupported, denied and granted states are W3's |
| Refresh MIDI inputs | button | **F** | refreshes the port list and says nothing; the "MIDI support could not be initialized" label beside it is not announced (owed to W3, whose section names the port's announced states) |

## Toolbar, third row (the views, Undo, Redo, Export)

| control | role | ends | evidence |
|---|---|---|---|
| SONG | toggle button | S | its selected state; F6 reaches the arranger (gate *Song*) |
| PIANO ROLL | toggle button | S | its selected state; F6 reaches the roll (every roll row) |
| MIXER | toggle button | H | the Mixer view |
| SYNTH | toggle button | H | the Synth view; disabled with "Add a track first." (R-22) |
| KEYBOARD | toggle button | H | the Keyboard view; disabled likewise |
| PADS | toggle button | H | the Pads view |
| Export | toggle button | S | shows the export row; now a toggle, so whether it is shown is its pressed state |
| Redo | button | A | "Redid the edit.", after the focused editor's name; disabled with "Nothing to redo." (R-22) |
| Undo | button | A | "Undid the last edit." — gate *Song: move between tracks* (Ctrl+Z, the same command); disabled with "Nothing to undo." (R-22) |

## The export row (shown by Export)

Each opens the desktop's save dialog, which the probe cannot drive; what each says once the dialog
has answered is shown by a test of the command's second half, and a cancelled dialog says so
("Nothing exported.", "Nothing rendered.").

| control | role | ends | evidence |
|---|---|---|---|
| Standard MIDI… | button | A | `what_a_dialog_command_does_once_its_dialog_has_answered_is_said` ("Exported 1 notes from 1 tracks to song.mid.") |
| Stereo WAV… | button | A | the same test ("Rendering in the background…", then "Rendered 1 notes from 1 tracks to song.wav") |
| Section WAV… | button | A | the same render path as Stereo WAV with the loop's range; the same test |
| Track stems… | button | A | the same test ("Rendering export 1 of 1: …", then "Exported 1 WAV file(s); …") |
| Arrangement parts… | button | A | the same export path as Track stems; the same test |

## The sound bar (toolbar region)

| control | role | ends | evidence |
|---|---|---|---|
| Track whose sound is shown | combo box | S | its list is named "Tracks"; choosing selects the track |
| + Track | menu button | S | its menu is named "+ Track menu" |
| + Track menu › Synth track | button | A | the same command as the side panel's +: "Added Track 2, a synth track with the Lead sound." — gate *Nine steps, 1 to 8* |
| + Track menu › Drum track | button | A | the same command as +KIT — gate *Track list* (and see H: it shows the Pads view) |
| Previous sound | button | A | Alt+Left's command: "Lead, 7 of 12 synth sounds" — gate *The instrument list*, *Nine steps* |
| Sound: …; opens the sound library | button | A | "The Sound library window is open; Tab reaches it." — gate *Track panel* |
| Next sound | button | A | Alt+Right's command — gate *The instrument list* |
| Load files… | button | A | past the desktop's file dialog: what this load came to, or "Nothing loaded." — `loading_sound_files_reports_only_what_this_load_came_to` |

## The Project menu ("Project menu")

| item | ends | evidence |
|---|---|---|
| New (Ctrl+N) | A | "New project, 1 track." — gate *Nine steps, 1 to 8*; `a_new_project_with_nothing_unsaved_says_so_and_a_save_says_where`; with changes unsaved, the desktop's question, then "Discarded the unsaved changes. New project, …" or "Kept the unsaved changes; no new project." (the sitting's) |
| Open… (Ctrl+O) | A | the dialog is the sitting's alone (waiver); what follows it is `open_project_path`: "Opened song.evil, 2 tracks." — gate *Files: open an existing project*, the same test |
| Import MIDI… | A | `what_a_dialog_command_does_…` ("Imported bach-invention-02.mid, 2 tracks, …") |
| Recover rhythm from WAV… | A | "The Recover rhythm from WAV window is open; Tab reaches it." — gate *Windows and the Project menu* |
| Save (Ctrl+S) | A | "Saved saved.evil." — gate *Nine steps, 9*; on a new project it is Save As |
| Save as… | A | the dialog is the sitting's alone (waiver); what follows it is `save_to`, the same as Save |
| Relink MIDI… (only with a linked MIDI file) | **F** | opens the desktop's file dialog and reports "Relinked MIDI to …" or "The MIDI link is unchanged.", but no test or row reaches past the dialog |
| Export MIDI… | A | as Standard MIDI… |
| Render WAV… | A | as Stereo WAV… |
| Render section WAV… | A | as Section WAV… |
| Track stems… | A | as Track stems… |
| Arrangement parts… | A | as Arrangement parts… |
| Keyboard notes… | A | "The Keyboard notes window is open; Tab reaches it." — gate *Windows and the Project menu* |
| About MIDI Evil | A | as the toolbar's About |

## The side panel

| control | role | ends | evidence |
|---|---|---|---|
| Add a synth track (+) | button | A | gate *Nine steps, 1 to 8*; `the_track_panel_says_what_adding_renaming_muting_and_a_sound_did` |
| Add a Maxwell drum-kit track (+KIT) | button | A | "Added Drums 3, a drum track with the Maxwell kit sound." — gate *Track list*. It also switches to the Pads view (H), as it always has: in the web build it must not, or it must be hidden with Pads |
| 01   Lead, 02   Bass, … (the track rack) | toggle buttons | S | selects the track; its selected state |
| Track rack, vertical scroll bar | scroll bar | S | |
| Track actions | menu button | S | its menu is named "Track actions menu"; disabled with "Not while recording." (R-22) |
| Track actions › Duplicate track | button | A | "Duplicated as one Copy, track 2 of 3." — gate *Track list* |
| Track actions › Move up / Move down | buttons | A | "Moved one Copy to track 3 of 3." — gate *Track list* |
| Track actions › Delete track | button | A | "Removed one Copy; 2 tracks left." — gate *Track list*, R-8 |
| Track name | text field | A | "Renamed Clean to …", once, when it lets focus go changed — gate *Track panel*; the test above |
| Browse sounds… | button | A | the same command as the sound bar's Sound button |
| Open Evil Synth / Open Pads | button | H | the Synth or Pads view |
| Samples, Modulation | buttons | H | the Synth view's pages |
| Mixer & FX | button | H | the Mixer view |
| Automation | button | H | the Automation panel |
| Gain, Pan | sliders and their value boxes | S | |
| Mute track | check box | A | "Clean muted." / "Clean unmuted." — gate *Track panel* |
| Performance | collapsing header | S | its expanded state |
| Bend range in semitones | value box | S | |
| Selected track settings, vertical scroll bar | scroll bar | S | |
| Side panel width, dragged with a pointer | splitter | **F** | a Tab stop that only a pointer can drag; no key path (W-A1b left it as Jesse's call) |

## The Song view

| control | role | ends | evidence |
|---|---|---|---|
| + Pattern part | button | A | "Added a part playing the new Pattern 3." — gate *Song: every arranger button*; `every_arranger_button_says_what_it_did`; refused while recording (R-22) |
| Duplicate loop | button | A | "Duplicated 2 events and parts after the loop." — the same row and test |
| Unique timeline | button | A | focus moves to the roll, then "The unique timeline of Lead, in the piano roll." — the same row and test |
| Song, … (the arranger) | canvas, one focusable editor | A | its name is the cursor's track, sound, part, pattern, placement and sharing; every chord's outcome — gates *Song: move between tracks*, *Song: Delete*, *Recording guard*; `the_arranger_is_one_editor_…`, `control_arrows_move_the_part_…`, `enter_opens_the_cursors_part_…`, `delete_removes_the_part_…`, `the_rulers_bar_has_a_key_…` |
| Bar ruler, clicked with a pointer to start playback at a bar | (not focusable) | A | a pointer's shortcut: its key path is the arranger's B ("Playback starts at bar 3.") and the toolbar's Start bar and Loop end bar; `the_rulers_bar_and_the_parts_first_bar_start_playback_the_same_way` |
| Arranger, vertical scroll bar | scroll bar | S | |
| Pattern name (part inspector) | text field | S | |
| Part start in ticks / Part length in ticks | value boxes | S | the value is the box's state, and the arranger's name says the placement; both pass the command boundary (refused while recording) |
| Loop part | button | A | "Looping from bar 1 beat 1 to bar 2 beat 1." — gate *Song: every arranger button* |
| Duplicate shared | button | A | "Duplicated the part; 3 parts now play Riff." — the same |
| Make unique | button | A | "This part now plays its own pattern, Riff Unique." — the same |
| Merge to timeline | button | A | "Merged into the unique timeline." — the same |
| Delete part | button | A | "Removed." — the same; during a take, "Not while recording." — gate *Recording guard* |

## The Notes view

| control | role | ends | evidence |
|---|---|---|---|
| Automation (beside the track's name) | toggle button | H | the Automation panel |
| Arranger | button | A | "The arranger is shown; F6 reaches it." — gate *Song: move between tracks* |
| Zoom | slider and value box | S | |
| Quantize | button | A | "Quantized all 3 notes." — gate *The roll's toolbar*; `the_rolls_whole_sequence_buttons_say_what_they_did`; R-19 |
| −12, −1, +1, +12 | buttons | A | "Moved all 3 notes an octave up." — the same row and test |
| Clean overlaps | button | A | "Cleaned the overlaps of all 2 notes.", whatever is selected — R-17's second row; `clean_overlaps_says_it_acted_on_every_note_even_with_one_selected` |
| Copy | button | A | "Copied 3 notes." — gate *The roll's toolbar* |
| Paste (once something is copied) | button | A | "Pasted." — the same row |
| Earlier, Later, Shorter, Longer, Add, Delete | buttons | A | each as its chord — R-14; disabled with "Select a note first." (R-22) |
| Piano roll, … | canvas, one focusable editor | A | W-A1a's rows, R-1 to R-20 |
| Piano roll, vertical and horizontal scroll bars | scroll bars | S | |
| Pitch, Velocity, Start in ticks, Length in ticks (note inspector) | value boxes | S | the roll's name follows them — R-1 |
| Delete note (note inspector) | button | A | the roll's Delete: "Removed." — `the_one_note_buttons_edit_the_cursors_note_and_are_disabled_without_one` |
| + Add a track to begin composing (no tracks) | button | A | the same command as + — R-8 reaches it |

## The About window ("Dr. Maxwell's studio")

| control | role | ends | evidence |
|---|---|---|---|
| (no title bar) | — | — | the window is anchored, so egui draws no title bar to drag and no Tab stop for one; the same naming call is made should it ever be movable (`every_open_menu_and_window_…` opens it with the others; gate *Windows and the Project menu* checks every node named with it open) |
| Close window | button | S | egui's own, named |
| Start composing | button | A | "The piano roll is open. F6 moves between the toolbar, the side panel and the editor." — gate *Windows and the Project menu* |
| Open project… | button | A | as Project › Open… |
| Import MIDI… | button | A | as Project › Import MIDI… |

## The sound library ("Sound library / Synth" or "/ Drum kit")

| control | role | ends | evidence |
|---|---|---|---|
| Sound library, title bar, dragged with a pointer to move it | title bar | **F** | named (`every_open_menu_and_window_…`), but moving the window is a pointer's drag with no key path: failing line 6 |
| Close window | button | S | |
| Search sounds or file paths | text field | S | |
| All sounds, Factory, User loaded | selectable buttons | S | their selected state |
| Load files… | button | A | as the sound bar's Load files… |
| Laser · FACTORY, …, Bass · FACTORY, … (one per sound) | toggle buttons | A | "Bass assigned to Clean; 1 of 12 synth sounds." — gate *Track panel* |
| Forget library entry (a user sound) | button | A | "Forgot … from your library; the file is untouched." — `forgetting_a_library_entry_says_so_and_leaves_the_file` |
| Sound list, vertical scroll bar | scroll bar | S | |

## The Keyboard notes window

| control | role | ends | evidence |
|---|---|---|---|
| Keyboard notes, title bar, … | title bar | **F** | named; failing line 6 |
| Close window | button | S | |
| Keyboard notes, one chord a line | read-only text | S | written from the three tables (`key_help.rs`); `the_arranger_describes_every_chord_…` |
| Keyboard notes, vertical scroll bar | scroll bar | S | |

## The Recover rhythm from WAV window

| control | role | ends | evidence |
|---|---|---|---|
| Recover rhythm from WAV, title bar, … | title bar | **F** | named; failing line 6 |
| Close window | button | S | |
| Choose WAV… | button | **F** | the desktop's file dialog, then "Chose …" reported; nothing reaches past the dialog |
| Detect tempo automatically | check box | S | |
| Draft tempo in beats per minute | value box | S | newly named |
| Quantize grid | combo box | S | |
| Create .evil draft… | button | **F** | the desktop's save dialog, then "Analyzing the audio…" and "Draft created: …" reported; nothing reaches past the dialog |
| Open recovered draft | button | A | as Open: "Opened recovered-rhythm.evil, 1 track." — through `open_project_path` |

## Pointer gestures

Every pointer-only gesture on the shown views, from the `Sense` and `ui.interact` sites, with its
key path:

| site | gesture | key path |
|---|---|---|
| `piano_roll.rs`, the roll's canvas (`click_and_drag`) | click a note: select it | Left, Right, Home, End |
| | click empty grid: add a note there | Enter adds after the cursor's note (or middle C at the start), then Up/Down, Shift+Up/Down and Control+Left/Right put it anywhere |
| | drag a note: move it | Up, Down, Shift+Up, Shift+Down, Control+Left, Control+Right; the toolbar's Earlier and Later |
| | drag a note's end: resize it | Shift+Left, Shift+Right; the toolbar's Shorter and Longer |
| `arranger.rs`, the arranger (`ui.interact`, `click_and_drag`, a fixed id) | click a lane: select the track's timeline | Up, Down, Home |
| | click a part: select it | Left, Right, End |
| | double-click: open it in the roll | Enter |
| | drag a part: move it by whole bars | Control+Left, Control+Right; the inspector's Part start in ticks for any tick |
| `arranger.rs`, the bar ruler (`Sense::CLICK`, not a Tab stop) | click a bar: start playback there | B on a part; the toolbar's Start bar and Loop end bar for any bar |
| `app/transport_readout.rs`, the readout (`ui.interact`, `click`) | click: focus | Tab, F6 |
| `app/track_rack.rs`, a track button | double-click: open the track in the roll | the button, then PIANO ROLL |
| egui's side panel edge | drag: resize the side panel | **none** — the failing line above |
| egui's movable windows' title bars | drag: move the window | **none** — failing line 6 |

Hover-only senses (`app/chrome.rs`'s status dot, the readout's text rectangle) act on nothing.

## Totals

The totals count the lines of the control tables — the toolbar's three rows, the export row, the
sound bar, the Project menu, the side panel, the Song and Notes views, the About window, the sound
library, the Keyboard notes window and the Recover rhythm window — and not the *Pointer gestures*
table, whose lines are key paths for gestures already counted as controls. A line that names
several controls of one kind (the transposes, the one-note buttons, the factory sounds) counts
once.

| ending | lines |
|---|---|
| A — announced, with a row or a test | 65 |
| H — opens a held-back surface | 9 |
| S — a named standard control | 40 |
| F — failing | 8 |

## Every failing line

1. **Refresh MIDI inputs** (toolbar) — refreshes silently; its "MIDI support could not be
   initialized" label is not announced. Owed to W3, whose section gives the port announced states.
2. **Relink MIDI…** (Project menu) — reports, but only past a file dialog, and no test reaches
   past it.
3. **Side panel width, dragged with a pointer** — a Tab stop with no key path; Jesse's call since
   W-A1b.
4. **Choose WAV…** (Recover rhythm window) — reports, but only past a file dialog.
5. **Create .evil draft…** (Recover rhythm window) — reports, but only past a save dialog.
6. **Moving a window by its title bar** (the sound library, Keyboard notes, Recover rhythm from
   WAV: three title-bar lines above) — a pointer's drag with no key path. Tolerable, I think: no
   window has to be moved to be used, and each opens where its content can be reached by Tab; the
   bar is a named Tab stop, so it is not silent. It is still a failing line by this inventory's
   rule, and the web build either hides these windows or gives them no drag.

Lines 2, 4 and 5 are announced; what they lack is mechanical evidence past their dialog. Each
becomes **A** the way the export row's commands and Load files… did: split the command at its
dialog and test the half that follows it. Six failing items, eight failing table lines (item 6
is three).
