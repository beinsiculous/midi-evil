# W0 — the accessibility spike's result

`coordination/web/plan.md` § W0. Two halves; this file is written as each closes.

**Verdict: W0 PASSED, 2026-11-24 — both halves, each on both kinds of evidence (D4). The port
runs.** W-A1 builds the keyboard model on the mechanism under *W0-native*; W-A2 builds the web
bridge on the mechanism under *W0-web*, which is the plan's mechanism 1 with eframe's text entry
adopted into it. W1 to W6 are unparked, and are re-planned and re-reviewed before any starts, as
the plan says.

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
visible status line. D11's bar — state that can be read again on demand — is met natively *for the
roll* by its name, which is there whenever the roll is focused. It is **not** met for the
transport, whose name is a snapshot taken on focus, and it is weakened for the roll by an outcome
that stays in the name; both are in *What the review of this result added*.

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

**Passed: the mechanical gate in headless Chromium and Firefox, and Jesse's sitting with Orca in a
real Firefox and a real Chromium — "yep it passes".** Mechanism 1 was enough; the DOM twin was not
started.

### The mechanism W-A2 builds on

- **The canvas** is `role="application"`, named, and `aria-describedby` a visible **current**
  element that always holds what egui's focus is on: the roll's name, the transport readout's
  string, or a standard control's description. That element is D11's "read again on demand".
- **A polite, atomic live region**, visually hidden, written once per frame and only while the
  canvas holds the page's focus. A notice and the name it changed go out as **one string, notice
  first** ("Removed. Piano roll, deleted C sharp 5 at …"): two writes to one polite region and the
  second replaces the first before it is spoken — the native half's lesson, in the browser's form.
  The current element is written before the live region in the same task. A repeated identical
  announcement carries a trailing no-break space, because rewriting the same text announces
  nothing. Nothing is announced when focus returns to the canvas; the screen reader reads its name
  and description itself.
- **Standard egui controls are described by egui itself.** `OutputEvent::FocusGained` *can* be read
  inside `update` on the web: `Response::widget_info` pushes it during the pass
  (`egui-0.35.0/src/response.rs:851-884`), `Context::output` reads the current pass
  (`context.rs:992`), and the runner takes and drops it only after the app returns
  (`eframe-0.35.0/src/web/app_runner.rs:283-289,372`). It carries no widget id, so the bridge pairs
  it with `memory.focused()`.
- **eframe's text entry is part of the mechanism, not beside it.** Focusing a `DragValue` or a
  `TextEdit` — tempo, meter, the project and track names — moves the *page's* focus to eframe's
  hidden `<input>`. Left as eframe makes it, that is an unnamed element outside the accessibility
  tree: the screen reader's "where am I" reads nothing. The bridge adopts it: while egui is editing
  text it is in the tree, `aria-label`led with the control's name, `aria-describedby` the current
  element, and placed after the canvas; otherwise hidden and refusing focus. Committing a value is
  announced once.
- **Tab is released at the boundary.** eframe prevents the default of every Tab on the canvas
  (`events.rs:265-273`, private; `WebOptions::should_prevent_default` covers only text events) and
  egui wraps its own traversal, so the canvas is a keyboard trap in both browsers. A capture-phase
  keydown listener on the window stops a Tab at egui's last stop, and a Shift+Tab at its first,
  before eframe sees it; the browser then moves focus on. Remedy 1 of the plan; the same in both.
- **Two things eframe's text entry does that nobody foresaw:** it takes focus into the canvas at
  page load, and, being a nameless input after the page's last element, it makes Tab loop back into
  the canvas. The bridge refuses its focus while it is hidden.

### The mechanical transcript

`coordination/web/probes/web-gate.sh`, headless Chromium 151 and Firefox 153, exit 0: eleven
sequences per browser and two negative controls each. The native gate's sequence and its edges with
the same strings as the native transcript; focus into the canvas and out again in both directions,
through all of egui's stops; a held Right at one repeat-flagged keydown per animation frame, each
announcement matching the cursor's note; twenty keydowns in one frame moving twenty notes with one
announcement; leaving and returning with no announcement and no cursor movement, the current note
readable from the canvas's description; the value box named and described where focus actually is,
typed into and committed.

**The gate's first run passed with the page's focus on the unnamed input.** Its `--expect` checked
the current element's text and looked at the focused element only when that was the canvas. Two
reviewers found it — one in the harness, one in the transcript's own lines. `--expect` now fails
unless the focused element is the canvas or is named by the step, and has a name and a description
carrying the text; and two negative controls per browser must fail, each for its own stated reason.

### What the sitting was asked, and what it left for W-A2

Jesse passed the half as a whole. He was asked the items below and did not answer them one by one,
so none of them is recorded as heard in a particular way; W-A2's own sitting takes them item by
item, and its first step is the browser-key table, on which its binding set depends (D14).

- Whether Orca speaks each note once or twice: the current note reaches it as a live-region write
  *and* as a change to the focused canvas's description.
- Whether the repeat mark makes a repeated notice speak ("Cannot go higher." twice).
- Whether a held key leaves stale announcements queued.
- At the value box: whether the entry is named on arrival, and the new value spoken once on Enter.
- The chords a browser may keep, which synthetic keys cannot show: Ctrl+Left, Ctrl+Right, F6,
  Shift+F6, Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+W, Ctrl+T, Alt+Left, Alt+Right. The synthetic table — every
  chord of the model and every native shortcut reaches egui in both browsers; eframe prevents the
  browser's default for the arrows, Ctrl+O, Ctrl+S, Space and every letter, and does not for Home,
  End, Enter, Delete, F6, Ctrl+N, W, T, Z, Y — is in the transcript, every row marked synthetic.

### What W-A2 starts from

- **Unnamed controls read "unnamed drag value".** egui gives a slider's value box no label; the
  tempo, the meter and the name fields are the same until W-A1 names them.
- **Typing then an arrow leaves the description stale** ("editing 2" after Up made it 3): the
  typed text is cached until focus moves. Found in review, not fixed in the harness.
- **The transport readout is re-described every frame on the web.** The harness's transport is
  frozen, so nothing showed it; a playing one would rewrite the live region each frame. The native
  readout's cached, read-on-demand string is what the bridge must publish.
- **`role="application"` hands every key to the page.** That is the point, and it is also why the
  page's own keyboard notes (W6) are not optional.
- **The bundle** is 6.4 MiB of wasm for the roll alone, about 2.2 MB gzipped, without `wasm-opt`.
- **Playwright's accessibility snapshot is its own computation**, not the browser's tree. Chromium's
  real tree is read over CDP for the checks that matter; Firefox offers no equivalent.

### What the plan's W0-web had wrong

- "Release Tab to the browser at egui's first and last widget" cannot be configured through eframe;
  it is a listener that gets to the key before eframe does.
- "A named, focus-synchronised element" is not a second focusable thing: it is the canvas's own
  description. The focused element changes only for text entry, which the plan did not know about.
- The held-key drive "twenty in a single frame" exists only as untrusted DOM events.

## What the review of this result added

The result was reviewed whole (codex, deepseek, gemini; `review/web/rebuttal-6.md`). Nothing in it
overturns the verdict, which rests on a person's sitting as well as the transcripts. What it adds
is what the two mechanisms do not yet answer. Each line is a requirement on the batch named, and is
written into that batch's section, as a gate row where it can be one, when the batch is re-planned.

**W-A1 — natively**

- **Identity when continuity cannot be shown.** Two identical notes, the cursor on the second, undo
  removes it: finding the note "by value" lands on its twin and the next Delete takes the wrong
  one. Where the cursor's note cannot be shown to be the same note, the selection is cleared and
  says so. Checking edit-in-place first does not cover this.
- **The transport must be readable now, not as it was.** Read on demand cannot mean a snapshot from
  when focus arrived: focus it stopped, press Space, wait, read again, and it says "stopped". One
  rule for a queryable surface — re-made from live state when it is read or activated, changed
  silently otherwise — and gate rows that read it while playing, after stopping, and twice without
  moving focus. A way to ask for the position from inside the roll, without leaving it.
- **An outcome is not state.** "deleted C sharp 5 at …" stays in the roll's name until the next
  key — through Tab away and back, as the gate's own transcript shows, and the gate asserts it.
  Outcomes go to the announcement and leave the name when focus leaves or on the next read.
- **Held keys.** Nobody judged a held key natively and the gate has no row for one. What the
  one-frame hold does when the name changes on every frame, or when "Cannot go higher." repeats
  every frame, is undefined; and a held name change needs a repaint asked for, or in egui's
  reactive mode the second frame may never come. A held-key sequence in the native gate and a
  held key in the sitting.
- **A screen reader started after the application.** The gate only ever enables the flag first.
  Whether the tree appears when Orca starts or restarts later is tested, and if it does not, the
  application recovers or says what to do.
- **Every unnamed node on a shown surface has an owner**, including the MIDI port
  (`midi_input.rs`, outside the batch's first file list) and egui's own scroll bars and resize
  handle, which are named, hidden from the tree, or explicitly accepted.
- **The nine defects of the spike are nine named gate rows**, the duplicate-note inspector drag
  first.
- **The native transcript is XWayland on a private bus; the sitting is Wayland-native.** Each gate
  row says which it is, and the sitting stays a per-batch row because it is the only evidence from
  the configuration people run.
- **The probe refuses to be dangerous.** No key is sent unless the probe is inside the private bus,
  and it will not start one without the in-memory settings backend — both incidents of this spike
  happened outside those conditions.

**W-A2 and after — in the browser**

- **Once or twice is settled before anything is built on it.** The current note reaches a screen
  reader as a live-region write and as the focused canvas's description changing. W-A2 opens with
  a sitting on exactly that, and the fallback is decided now: if both are spoken, the live region
  carries notices only and the description carries state.
- **Browser-owned chords are unbound, not merely unadvertised, and leaving is guarded.** Alt+Left
  is bound to cycling the sound and is the browser's Back; Ctrl+W closes the tab; F6 is the
  browser's own way to its address bar. On the web such chords are not bound at all, region
  movement gets a chord a browser leaves alone — found by a person, since synthetic keys cannot
  show it — and the page registers `beforeunload` while the document is dirty. New and Open get an
  in-application, keyboard-operable, announced discard prompt (W5b's dialogs).
- **The Tab boundary is computed against a changing set of stops** — a menu open, the About window,
  a panel appearing, a value box at the first stop — and tested against each; and the plan's
  second remedy, a visible, focusable "leave the editor" control, is built as a standing backstop
  rather than kept as an untried fallback, because a stale boundary is a keyboard trap.
- **The repeat mark has a stated rule** that survives a third identical notice, with a gate row.
- **Chromium's real tree is asserted, not printed**: the focused node's name and description over
  CDP, with its own negative control. Firefox's rows are marked as Playwright's computation, and
  its real tree is covered by the sitting, stated per batch.
- **The wasm boundary of the device and dialog code** is planned with the batch that first compiles
  it: the MIDI port (Web MIDI is absent in Firefox and permission-gated in Chromium) gets a named,
  keyboard-operable web state or is held back under D12; `rfd::MessageDialog` has no browser form.
- **eframe's private internals are load-bearing** — the hidden input's shape, the Tab handling
  cited above by line. The web crate pins eframe and egui (D8), and an upgrade re-runs the web
  gate before anything else.

