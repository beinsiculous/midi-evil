# W0 — the accessibility spike's result

`coordination/web/plan.md` § W0. Two halves; this file is written as each closes.

**Verdict so far: the native half PASSED, 2026-11-24, on both kinds of evidence (D4). The web half
has not started.** No verdict on the port until it has.

The spike's code is on `spike/w0-accessibility`, never merged. The probes are in
`coordination/web/probes/`.

## W0-native

### The mechanism W-A1 builds on

1. **The piano roll is one focusable widget whose accessible *name* carries the cursor.** The name
   is `"Piano roll, "` followed by the cursor's note, or `empty`, or `no note selected`. Orca speaks
   a change to the focused node's name on every key. Nothing else was needed for moves and edits.
2. **A polite live node carries what changes no note.** "Cannot go higher.", "Nothing after this
   one.", "Removed." — a node marked `Live::Polite`, added for one frame, emits
   `object:announcement`, and Orca speaks it **when the name does not change in the same frame**.
   When both happen in one frame the name wins and the announcement is lost, so an outcome's
   announcement goes out first and the name's change is held one frame.
3. **Not the value, and not the description.** AccessKit's AT-SPI adapter exposes only numeric
   values; a string value on a custom node reaches nothing. A description's change is an event Orca
   does not speak. The description holds the roll's key help, read once on focus.

D6 held: one widget with a note cursor. D5 held: one description function feeds the name and the
visible status line. D11's bar — state that can be read again on demand — is met natively by the
name, which is there whenever the roll is focused.

### What Jesse heard (Orca 49.4, COSMIC, Wayland-native)

- Step 1, the counter build: "Piano roll, step 1", "Piano roll, step 2" on each Right; the
  description's change and the announcement raised by the same key were not heard. (An earlier try
  heard nothing at all: the roll had been clicked, and a click does not focus a custom egui widget.
  The prototype takes focus on click.)
- The prototype, the gate's sequence by keyboard from F6: passed — "we're good".
- The standalone announcements — "Nothing after this one.", "Cannot go higher.", "Removed." —
  **heard**, all three.

### The mechanical transcript

`coordination/web/probes/native-gate.sh`, run by the executor twice and by the planner once, exit 0
each time, 60 expectations in its final form: the main sequence and five edges, each in a fresh application. From the
planner's run:

```
after F6: status bar 'Transport, stopped, tempo 64 beats per minute, position 0 minutes, 0 seconds'
after F6: toggle button '01   one'
after F6: canvas 'Piano roll, no note selected'
after Right: canvas 'Piano roll, C 5, bar 1 beat 1.5, 0.25 beats, velocity 127'
after Up: canvas 'Piano roll, C sharp 5, bar 1 beat 1.5, 0.25 beats, velocity 127'
after ctrl+Right: canvas 'Piano roll, C sharp 5, bar 1 beat 1.75, 0.25 beats, velocity 127'
after shift+Right: canvas 'Piano roll, C sharp 5, bar 1 beat 1.75, 0.5 beats, velocity 127'
after Delete: canvas 'Piano roll, deleted C sharp 5 at bar 1 beat 1.75, C 5, bar 1 beat 2, 0.25 beats, velocity 127'
  event object:announcement detail1=1 payload='Removed.'
after Tab: button ''
after shift+Tab: canvas 'Piano roll, deleted C sharp 5 at bar 1 beat 1.75, C 5, bar 1 beat 2, …'
```

and the edges: Up at pitch 127 → `'Piano roll, already at the top, G 9, …'` with
`payload='Cannot go higher.'`, and again on a second press with the name unchanged; Ctrl+Left at
tick 0 → `already at the start` / `'Cannot move earlier.'`; Enter in an empty shared pattern →
`'Piano roll, added, C 4, pattern bar 1 beat 1, 1 beat, velocity 100; used by 1 part'`, Delete →
`'Removed.'`, Delete again → `'Nothing here yet.'`; Enter after the last note of a full pattern →
refused, the project still valid; select, Clean overlaps, return, Delete → the note deleted is the
one that was selected, though its index changed.

**The two kinds of evidence came from two setups.** The probe runs the application under XWayland
on a private accessibility bus in a private runtime directory, because sending keys with
`XSendEvent` to the application's own window is the only delivery found that cannot land anywhere
else. Jesse's sitting ran it Wayland-native on the desktop's own bus.

### The keyboard model as it stood

With the roll focused; every name begins "Piano roll, ".

| chord | effect | at a limit |
|---|---|---|
| Right / Left / Home / End | cursor to the next / previous / first / last note, ordered by start, pitch, index | stays; "Nothing after this one." / "Nothing before this one." |
| Up / Down; with Shift, an octave | pitch, clamped 0–127 | "Cannot go higher." / "Cannot go lower." |
| Ctrl+Left / Ctrl+Right | start by one snap; saturates at 0 and at a pattern's last snap | "Cannot move earlier." / "Cannot move later." |
| Shift+Left / Shift+Right | length by one snap, never below one | "Cannot be shorter." |
| Enter | a one-beat note after the cursor's, at its pitch; middle C at tick 0 with no cursor | past a pattern's end: refused, "Nothing added: the pattern ends here." |
| Delete | removes the cursor's note; cursor to the next, else the previous, else none | empty: "Nothing here yet." |
| F6 / Shift+F6 | toolbar (the transport readout) → the selected track → the editor | — |
| Tab / Shift+Tab | never taken by the roll | — |

An edit in a shared pattern says its reach: "; shared by N parts". A click focuses the roll. Delete
acts on focus, not hover.

### What the plan's step 2 had wrong

- "The roll's node carries the cursor's note as its value" — nothing reads a string value; then
  "as its description" — Orca does not speak it. It is the name.
- "Each edit also goes to a live node" — in the frame the name changes, the live node is unheard.
  It is for outcomes that change no note.
- A held arrow cannot be counted with `num_presses`, which ignores modifiers and so cannot tell
  Right from Ctrl+Right; the roll walks the frame's key events in order.
- "Finds its note again by value" fails after Clean overlaps, which shortens notes. Relocation
  tries the exact value, then an edit in place, then a unique start and pitch, and only then resets
  and says so.

### What W-A1 starts from

- **The tree is flat and half anonymous**: ninety nodes, all children of one unnamed frame,
  sixty-one focusable. Unnamed, by module — `app/chrome.rs`: the About button, Save, the project
  name, the meter, START BAR, LOOP END, the track name (the tempo was named in the spike);
  `midi_input.rs`: the MIDI port; `app/sounds.rs`: SOUND; three scroll bars and a panel's resize
  handle from egui. The window's title is not exposed. Tab from the roll lands on the unnamed About
  button.
- **AccessKit builds no tree until the session's `ScreenReaderEnabled` is true**; a person starts
  the screen reader first.
- **Left open by the spike:** the transport was only read stopped — its position while playing is
  unheard and unprobed, and its name is a snapshot taken on focus, stale in the tree between reads;
  Alt as a modifier is unproven through the probe (Shift and Control are); the `canvas` role was
  accepted by Orca at the sitting but nothing was tried against it; holding a key was not judged.

### What the review of the spike found, which W-A1 must not repeat

The spike was reviewed (codex, deepseek) and not fixed — it is thrown away. Each of these is a case
for W-A1's tests.

- **Relocation order.** The cursor is found again by value *before* checking for an edit in place,
  so with two identical notes, dragging one's velocity in the inspector moves the selection to its
  twin and the drag goes on editing the wrong note. An edit confined to the selected index is
  checked first.
- **The first arrow after F6 can leave the roll.** egui ignores a focus-lock filter set in the
  frame focus arrives, so an arrow in the very next frame both acts on the note and moves focus.
  A person is too slow to hit it; a held key or a script is not. Handled arrows are consumed.
- **F6 judges the roll by the centre of its whole scroll canvas**, which for a bass part lies above
  the editor: the roll is taken for the toolbar and the toolbar becomes unreachable by F6. The roll
  is classed by its id.
- **One Delete, two deletions** — the roll on focus, the automation lane on hover — as the plan
  already says; W-A1 moves all three sites.
- **A stale outcome in the name.** "already the last note" stays in the name until the next key, so
  an undo that changes the note makes the name say both. The outcome is cleared when the notes
  change from outside.
- **A selection lost to undo is not announced**: the index is clamped away before the roll can see
  that it had one.
- **The transport's spoken name is not invalidated** by New, Open or a tempo change.
- **The key help omits Home, End and the octave chords**, and says Control where macOS takes
  Command.
- **F6's editor stop outlives the roll** when the last track is deleted.

### How it went wrong along the way, for whoever runs this again

- AT-SPI's own key synthesis reaches nothing Wayland-native, and under this XWayland
  (`-enable-ei-portal`) it is passed to the compositor: a focus-wide injector. One Tab left the
  application that way at 09:40:35 before it was removed.
- Setting `ScreenReaderEnabled` on a private bus is stored by its launcher through GSettings; with
  the real backend it wrote `screen-reader-enabled=true` into the desktop's dconf at 09:36, reset at
  09:48. `GSETTINGS_BACKEND=memory` must be set *outside* `dbus-run-session`. `run-probe.sh` does.
- An early `run-probe.sh` kept its process ids in locals its trap could not see, and left a window
  open for two minutes.

## W0-web

Not started.
