# W-A2 inventory — every control of W-A1's list, re-read for the browser

Every line of `w-a1-inventory.md`, as the browser build draws it. W-A1's list is of the surfaces
the web build shows; this one says, for each of its lines, what the browser build does with it and
which row of `coordination/web/probes/web-app-gate.sh` shows that. W6's inventory gate starts from
this list (D12).

**How it was taken.** Each line of `w-a1-inventory.md` against the browser's list of what it
leaves out (`crates/midi-evil/src/platform.rs`, `LEFT_OUT_OF_THE_BROWSER`), the crate's tests of
that list (`app/shown_tests.rs`), and the web gate's rows, run on the bundle each names. Rows are
named by their key in `web-app-rows.mjs`, `web-app-matrix-rows.mjs` and `web-app-page-rows.mjs`;
every one runs in headless Chromium, judged by its real tree over CDP, and headless Firefox,
judged by Playwright's computation.

**How a line ends.** Each line ends one of five ways:

- **B** — announced in the browser, with its row.
- **U** — unavailable until a named batch: a Tab stop, described as unavailable with its reason,
  that says the reason again when activated; with the row that hears the reason.
- **X** — absent from the browser: not drawn, so neither a Tab stop nor in the frame's tree, with
  its absence row and the batch that brings it back by removing its line from the platform's
  list. Its absence rows are *absence-walk* (no stop of the Song and Notes views, a menu or a
  window is a removed thing; its negative control, the gate's bundle with `?shows=desktop`, must
  reach the Mixer's tab and fail), *absence-no-way-in* and *no-window-moves*, and the crate's
  `the_browser_draws_nothing_its_list_leaves_out_and_the_desktop_draws_it_all`.
- **S** — a named standard control; activating it changes nothing a listener must be told beyond
  the control's own name, role, state and value, which the bridge writes (*states*,
  *text-fields*, *r12-every-stop-named*).
- **F** — failing: none of the four. Every one is listed again at the end.

The batches that bring an **X** back: **W3** gives the MIDI input announced states; **W5b** gives
the browser its file dialogs, saves and downloads; **W4** renders and recovers in a worker. The
Mixer, Synth, Keyboard and Pads views and the Automation panel come back each with a batch and a
gate of its own, none of them planned yet (D12). The stops only a pointer can drag are not brought
back to the browser: a window there does not move and the side panel does not resize.

## Toolbar, first row

| control | W-A1 | browser | evidence |
|---|---|---|---|
| About MIDI Evil | A | B | "The About window is open; Tab reaches it." — *click-from-outside*, *windows-and-the-project-menu* |
| Project | S | S | "Project, button, menu open" — *states* |
| Save project | A | B | the same command as Control+S: "Not saved." — *nine-steps* |
| PLAY / STOP / CANCEL | S | U (W3, W4) | "PLAY, button, unavailable. Playback is not in the browser build yet.", and Enter says it — *unavailable*; Space says it — *unavailable* |
| Record / Stop take | A | U (W3, W4) | "Record, toggle button, not pressed, unavailable. Playback is not in the browser build yet." — *r22-unavailable-controls* |
| Transport, … (the readout) | A | B | Enter reads it — *r21-tempo-and-a-new-project*; I reads it — *nine-steps*, *song-by-keyboard* |
| Project name | S | S | *name-field-editing*; every fixture row reads it first |
| Tempo in beats per minute | S | S | typed, stepped, committed and heard once — *text-fields*, *held-up-on-tempo*, *r21-tempo-and-a-new-project* |
| Meter, beats per bar over 4 | S | S | *text-fields*, *field-ended* |

## Toolbar, second row

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Start bar | S | S | *r12-every-stop-named* |
| Loop end bar | S | S | the same |
| Repeat section | S | S | "Repeat section, check box, checked", written once — *states* |
| Quantize recording to 1/16 | S | S | *r12-every-stop-named* |
| MIDI input port | S | X (W3) | *absence-walk* |
| Refresh MIDI inputs | **F** | X (W3) | *absence-walk*; W3 gives the port its announced states and brings both back |

## Toolbar, third row

| control | W-A1 | browser | evidence |
|---|---|---|---|
| SONG | S | S | its selected state — *r12-every-stop-named* |
| PIANO ROLL | S | S | the same |
| MIXER | H | X (its own batch) | *absence-walk*, and its negative control reaches it with `?shows=desktop` |
| SYNTH | H | X (its own batch) | *absence-walk* |
| KEYBOARD | H | X (its own batch) | *absence-walk* |
| PADS | H | X (its own batch) | *absence-walk* |
| Export | S | X (W4, W5b) | *absence-walk* |
| Redo | A | B | "Redid the edit." when available; "Redo, button, unavailable. Nothing to redo.", and Enter says it — *r22-unavailable-controls* |
| Undo | A | B | "Undid the last edit." — *r22-unavailable-controls*, *delete-and-undo*; unavailable with "Nothing to undo." — *unavailable* |

## The export row

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Standard MIDI… | A | X (W5b) | *absence-walk*: the row is never shown, its toggle being absent |
| Stereo WAV… | A | X (W4, W5b) | the same |
| Section WAV… | A | X (W4, W5b) | the same |
| Track stems… | A | X (W4, W5b) | the same |
| Arrangement parts… | A | X (W4, W5b) | the same |

## The sound bar

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Track whose sound is shown | S | S | *r12-every-stop-named* |
| + Track | S | S | its menu, named "+ Track menu" — *absence-walk* walks it |
| + Track menu › Synth track | A | B | the same command as the side panel's +: "Added Track 2, a synth track with the Lead sound." — *nine-steps* |
| + Track menu › Drum track | A | B | "Added Drums 3, a drum track with the Maxwell kit sound.", and the view stays where it was — *track-list*, *absence-no-way-in* |
| Previous sound | A | B | "Lead, 7 of 12 synth sounds" — *instrument-list*, *nine-steps* |
| Sound: …; opens the sound library | A | B | "The Sound library window is open; Tab reaches it." — *track-panel* |
| Next sound | A | B | " 8 of 12 synth sounds" — *instrument-list*, *nine-steps* |
| Load files… | A | X (W5b) | *absence-walk* |

## The Project menu

| item | W-A1 | browser | evidence |
|---|---|---|---|
| New | A | B | "New project, 1 track." — *nine-steps*; with work unsaved it refuses and says why, until W5b's prompt — *r21-tempo-and-a-new-project* |
| Open… | A | B | "Nothing opened.", until W5b's dialog — *windows-and-the-project-menu* |
| Import MIDI… | A | X (W5b) | *absence-walk* |
| Recover rhythm from WAV… | A | X (W4, W5b) | *absence-walk* |
| Save | A | B | "Not saved.", never "Saved", until W5b — *nine-steps* |
| Save as… | A | B | "Not saved." — *windows-and-the-project-menu* |
| Relink MIDI… | A | X (W5b) | *absence-walk*; the crate's test draws it with a linked file and finds it absent |
| Export MIDI… | A | X (W5b) | *absence-walk* |
| Render WAV… | A | X (W4, W5b) | *absence-walk* |
| Render section WAV… | A | X (W4, W5b) | *absence-walk* |
| Track stems… | A | X (W4, W5b) | *absence-walk* |
| Arrangement parts… | A | X (W4, W5b) | *absence-walk* |
| Keyboard notes… | A | B | "The Keyboard notes window is open; Tab reaches it." — *focus-boundary*, *windows-and-the-project-menu* |
| About MIDI Evil | A | B | as the toolbar's About — *windows-and-the-project-menu* |

## The side panel

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Add a synth track (+) | A | B | *nine-steps* |
| Add a Maxwell drum-kit track (+KIT) | A | X (with Pads) | *absence-walk*; a drum track is added from + Track › Drum track instead |
| 01   Lead, … (the track rack) | S | S | selects the track; *regions* reaches it |
| Track rack, vertical scroll bar | S | S | |
| Track actions | S | S | its menu, named "Track actions menu" — *absence-walk* walks it |
| Track actions › Duplicate track | A | B | *track-list*, *r15-duplicate-a-track-and-undo* |
| Track actions › Move up / Move down | A | B | *track-list* |
| Track actions › Delete track | A | B | *track-list*, *r8-delete-the-last-track* |
| Track name | A | B | "Track name, Cleanx" committed once — *track-panel*, *text-fields*; markup reaches the page as text — *markup-as-text* |
| Browse sounds… | A | B | the same command as the sound bar's Sound button — *track-panel* |
| Open Evil Synth / Open Pads | H | X (their own batches) | *absence-walk* |
| Samples, Modulation | H | X (with Synth) | *absence-walk* |
| Mixer & FX | H | X (with the Mixer) | *absence-walk* |
| Automation | H | X (with the Automation panel) | *absence-walk* |
| Gain, Pan | S | S | *r12-every-stop-named* |
| Mute track | A | B | "Clean muted." and "unmuted." — *track-panel* |
| Performance | S | S | |
| Bend range in semitones | S | S | |
| Selected track settings, vertical scroll bar | S | S | |
| Side panel width, dragged with a pointer | **F** | X (not in the browser) | the panel does not resize, so the handle is no stop — *no-window-moves*, `in_the_browser_no_window_moves_and_the_side_panel_does_not_resize` |

## The Song view

| control | W-A1 | browser | evidence |
|---|---|---|---|
| + Pattern part | A | B | *song-buttons*, *w0-edge-empty-pattern* |
| Duplicate loop | A | B | *song-buttons*, *w0-edge-full-shared-pattern* |
| Unique timeline | A | B | "The unique timeline of Lead, in the piano roll." — *song-buttons* |
| Song, … (the arranger) | A | B | *song-by-keyboard*, *song-delete-a-part*, and every fixture row's first step |
| Bar ruler, clicked with a pointer | A | U (W3, W4) | its key path, B, says "Playback is not in the browser build yet." — *song-by-keyboard*, *unavailable* |
| Arranger, vertical scroll bar | S | S | |
| Pattern name | S | S | *r12-every-stop-named* |
| Part start in ticks / Part length in ticks | S | S | the same |
| Loop part | A | B | *song-buttons* |
| Duplicate shared | A | B | *song-buttons* |
| Make unique | A | B | *song-buttons* |
| Merge to timeline | A | B | *song-buttons* |
| Delete part | A | B | *song-buttons* |

## The Notes view

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Automation (beside the track's name) | H | X (with the Automation panel) | *absence-walk*; R-4 is not applicable — *r4-no-automation-lane* |
| Arranger | A | B | "The arranger is shown; Alt+Page Down reaches it." — *roll-toolbar* |
| Zoom | S | S | |
| Quantize | A | B | *roll-toolbar*, *r19-escape-clears-then-leaves* |
| −12, −1, +1, +12 | A | B | *roll-toolbar* |
| Clean overlaps | A | B | *r17-clean-overlaps-re-sorts*, *r17-clean-overlaps-shortens* |
| Copy | A | B | *roll-toolbar* |
| Paste | A | B | *roll-toolbar* |
| Earlier, Later, Shorter, Longer, Add, Delete | A | B | each as its chord — *r14-one-note-buttons*; with no note, unavailable with "Select a note first.", said when activated — *r14-one-note-buttons*, *r22-unavailable-controls* |
| Piano roll, … | A | B | *nine-steps*, *delete-and-undo*, *held-key*, and the matrix's W0 and R rows |
| Piano roll, scroll bars | S | S | |
| Pitch, Velocity, Start in ticks, Length in ticks | S | S | the roll's name follows them — *r1-identical-twins* |
| Delete note | A | B | "Removed." — *r14-one-note-buttons* |
| + Add a track to begin composing | A | B | reached by the region chord and activated — *r8-delete-the-last-track* |

## The About window

| control | W-A1 | browser | evidence |
|---|---|---|---|
| (no title bar) | — | — | anchored, and in the browser no window moves |
| Close window | S | S | *focus-boundary* passes it |
| Start composing | A | B | "The piano roll is open. Alt+Page Down moves between the toolbar, the side panel and the editor." — *windows-and-the-project-menu* |
| Open project… | A | B | "Nothing opened." — *windows-and-the-project-menu* |
| Import MIDI… | A | X (W5b) | *absence-walk* walks the window |

## The sound library

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Sound library, title bar | **F** | X (not in the browser) | the window does not move, so the bar is no stop — *no-window-moves* |
| Close window | S | S | |
| Search sounds or file paths | S | S | |
| All sounds, Factory, User loaded | S | S | their selected state |
| Load files… | A | X (W5b) | *absence-walk* |
| one button per sound | A | B | "Bass assigned to Cleanx; Bass, 11 of 12 synth sounds." — *track-panel* |
| Forget library entry | A | X (W5b) | drawn only for a sound loaded from a file, which only Load files… gives |
| Sound list, vertical scroll bar | S | S | |

## The Keyboard notes window

| control | W-A1 | browser | evidence |
|---|---|---|---|
| Keyboard notes, title bar | **F** | X (not in the browser) | *no-window-moves* |
| Close window | S | S | |
| Keyboard notes, one chord a line | S | S | written by the function that writes the page's notes, so the two say the same of Space and B — `the_notes_say_why_a_refused_chord_does_nothing_and_name_no_chord_a_browser_keeps`; *focus-boundary* |
| Keyboard notes, vertical scroll bar | S | S | |

## The Recover rhythm from WAV window

The window opens only from Project › Recover rhythm from WAV…, which the browser leaves out, so
every line of it is absent with it (W4, W5b): its title bar, Close window, Choose WAV…, Detect
tempo automatically, Draft tempo in beats per minute, Quantize grid, Create .evil draft… and Open
recovered draft — eight lines, **X**.

## Pointer gestures

Each of W-A1's key paths holds in the browser, through the browser's own chords: the roll's and
the arranger's gestures by their chords (the matrix), the readout by Tab and the region chord
(*regions*), a track's double-click by its button and Enter in the arranger. The ruler's click is
the B of the Song view, which says why it cannot play (**U**, W3, W4). The two gestures W-A1 left
without a key path, dragging the side panel's edge and a window's title bar, are not offered in
the browser at all (*no-window-moves*).

## Totals

The lines of the control tables above, counted as `w-a1-inventory.md` counts them, the Recover
rhythm window's eight among them.

| ending | lines |
|---|---|
| B — announced in the browser, with its row | 45 |
| U — unavailable until a named batch | 3 |
| X — absent from the browser | 41 |
| S — a named standard control | 33 |
| F — failing | 0 |

122 lines, as W-A1's 68, 9, 40 and 5 are; the About window's line for its absent title bar is
not a control and is counted in neither list.

## Every failing line

None. W-A1's five failing lines end: *Refresh MIDI inputs* absent until W3; *Side panel width* and
the three windows' title bars absent from the browser, where nothing drags.
