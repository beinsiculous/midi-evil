# MIDI Evil Web — the effort's plan

MIDI Evil in the browser at `/midi-evil/play/`. `docs/web-port.md` is the feasibility record and the
ordered steps; this file is the plan that executes them: the decisions, the batches, each batch's
files and gates, and what each leaves out. Where the two disagree about *what a step is*, the record
wins and this file is corrected; where they disagree about *order, roles, evidence or a step's
gate*, this file wins, because the record says those are this plan's to settle — and the record is
corrected in the same commit that settles this plan (see *The record's corrections*).

Milestone **MIDI Evil Web** in `beinsiculous/midi-evil`: W0 `#2`, W1 `#3`, W2 `#4`, W3 `#5`,
W4 `#6`, W5 `#7`, W-A `#8`, W6 `#9`. Branch `jesse` throughout.

**Status: SETTLED by Jesse, 2026-11-24 (v4, committed `b1d2a8f`). W0 is open.** Rounds 1 and
2 (`codex`, and `deepseek` standing in for `kimi`, which has no credential on bob-ross) are answered
in `review/web/rebuttal-1.md` and `rebuttal-2.md`.

**What this plan settles and what it does not.** W0 and W-A1 are planned here in full. W1–W6 and
W-A2 are planned here as *constraints* — what each must do beyond the record, found in review — and
each gets its own full section, with files and gates, in a revision of this file that goes back
through review after W0's result. Settling this plan authorises W0 and nothing after it.

## Decisions

- **D1 — Roles.** Claude Fable plans; **Claude Opus 5.5 builds each batch from a handoff** (Jesse,
  2026-11-24), and the planner takes the result back as `handoff-loop` § 3 says; a vendor never reviews
  its own vendor's work, so every plan revision and every diff goes to `kimi` (`deepseek` when kimi
  is out); `codex` (Astra) joins wherever a screen is in it — this plan, W0's result, W-A1, W-A2,
  W6; `gemini` joins where a harness, fixture or public seam changes — W0's result and probes, W1,
  W2, W-A2, W4, W5. Jesse adjudicates every finding and is the ears of every screen-reader gate (D4).

- **D2 — Order: W0, W-A1, W1, W2, W-A2, W3, W5a, W4, W5b, W6.** *(Ruled by Jesse, 2026-11-24.)* The record lists
  W-A after W5; this plan splits it and moves the native half forward. **W-A1** is the production
  keyboard model and its announcements, natively — it needs nothing from W1–W5, it is worth shipping
  on the desktop whether or not the port ever does, and it is the part most likely to change shape
  under review, so it should not be found out last. **W-A2** is the web bridge on W0's mechanism and
  needs the web crate, so it follows W2. Both close under `#8`. W5 is split because W4's gate needs
  bytes in and bytes out (D10). One batch is out at a time.

- **D3 — W0 is throwaway, on a branch that is never merged; its probes are not.** The spike's
  application code lives on `spike/w0-accessibility`, pushed, never merged into `jesse`, and kept
  until W6 ships. What lands on `jesse` is `coordination/web/w0-result.md` and
  `coordination/web/probes/` — the two probe scripts and a README saying how to run them — because
  D4's mechanical evidence is only evidence if it can be regenerated against W-A1 and W-A2.

- **D4 — A screen-reader gate has two kinds of evidence, and needs both.** *(Ruled by Jesse, 2026-11-24.)*
  *Mechanical:* a transcript an agent can produce and re-run — natively, the AT-SPI tree and its
  events read through `gi.repository.Atspi` while keys are delivered; in the browser, Playwright's
  accessibility snapshot and the bridge's text after each key. *Human:* Jesse performs the same
  sequence with Orca and says what he heard. The transcript makes the gate repeatable; it does not
  replace the ears. A gate with only one of the two is not passed — if no agent-driven way to deliver keys can be
  found, the half is *blocked* until Jesse waives the transcript in writing. A batch's timebox clock
  stops while it waits on a sitting.

- **D5 — One source of strings, with a stated context.** What a note, a part, an instrument or the
  transport *says* is produced by one function per thing in the `midi-evil` library, and every
  mechanism — the native accessibility node, the web bridge, the visible status line — reads it.
  A note's description takes the meter and **the origin it is described from**: in the roll over a
  unique timeline, the song's bar and beat; in the roll over a shared pattern, the position *within
  the pattern*, and the string says so ("pattern bar 1 beat 3"), because the same pattern sounds at
  several song positions and naming one of them would be wrong for the others. The arranger
  describes a part's placement in song time. An edit to a shared pattern says how far it reaches
  ("deleted; shared by 3 parts"), because a sighted person sees the repeated blocks and a listener
  does not. The string tests carry a shared-pattern case.

- **D6 — The piano roll is one focusable widget with a note cursor, not a node per note.** The roll
  paints only what the viewport shows, so per-note nodes would make off-screen notes unreachable.
  The roll takes focus as a whole, keeps a cursor over the notes in time order, scrolls the cursor
  into view, and exposes the cursor's note so that it can be **read again on demand** — on both
  surfaces (D11). Natively that is the roll's *description*, not its value: AccessKit's AT-SPI
  adapter exposes only numeric values, and a string value on a custom node reaches nothing (W0
  step 1, 2026-11-24). **The cursor names a note, not an index.** The notes are stored in insertion
  order and the selection today is an index into them (`RollState::selected_note`), while Clean
  overlaps re-sorts the vector (`evil-project/src/sequence.rs:121-132`) and undo replaces it. So the
  cursor walks the notes ordered by start, then pitch, then index; and after any change it did not
  make itself — a toolbar operation, undo or redo, a replaced document — it finds its note again by
  value, or, failing that, resets to none and says so. It never silently points at a different
  note. W0 may overturn the single-widget shape; the result says whether it held.

- **D7 — Upstream web AccessKit is not waited for.** eframe 0.37.0, the newest published, still
  discards the update on the web (`src/web/app_runner.rs:403`, checked 2026-11-24).

- **D8 — The eframe version does not move during this effort** unless a batch's gate cannot be met
  on 0.35; a bump is its own batch with the native suite as its gate.

- **D9 — Every batch re-verifies its own section against the tree before it starts**, as its own
  commit. Line numbers below are as of 2026-11-24, `jesse` at `252990b`.

- **D10 — Rendering in the browser runs in a Web Worker.** *(Ruled by Jesse, 2026-11-24.)* The record's W4 says
  "inline or chunked, with a worker later if needed". Neither inline nor chunked can meet its own
  gate: `RenderJob::spawn` (`render_job.rs:57-66`) runs one synchronous closure, and under it
  `evil-render` synthesises and mixes whole buffers in synchronous loops, so the page freezes for
  the length of the render — input and the accessibility bridge with it — and chunking between
  tracks does not bound one long track. Making synthesis resumable would mean threading yield
  points through `evil-synth`'s deterministic core for the browser's sake. A dedicated worker
  running the same wasm module — project and resolved sources in as bytes, PCM out as bytes — needs
  no atomics and no COOP/COEP, leaves the DSP crates untouched, and takes the recovery analysis the
  same way. That is why W5 splits: **W5a** is the byte interfaces (the resolver in place of
  `patch_base: &Path`, the byte twins, `create_draft` on bytes), **W4** is the worker on top of
  them with a wasm-side gate, **W5b** is the browser's dialogs, saves and downloads.

- **D11 — The two surfaces are held to the same bar: queryable state, not only passing speech.**
  Natively the roll's value can be read at any time. On the web a live region alone cannot: a
  visitor who returns to the canvas has nothing to re-read, and a held arrow key overwrites a polite
  region faster than it is spoken. So whatever mechanism W0-web picks must offer both an
  announcement on change and a way to read the current thing on demand, and its gate tests both.

- **D12 — The page exposes only what passed.** *(Ruled by Jesse, 2026-11-24.)* The record's bar is that *every
  exposed editing workflow* is keyboard-operable and announced; its nine-step sequence is the
  minimum regression, not the bar. So W6's gate is an **inventory**: every workspace and editing
  surface the web build shows is listed, and each either has passed its own keyboard-and-announced
  gate or is not shown in the web build, with the page saying which parts of MIDI Evil are
  desktop-only for now. Classified by the application's own views (`workspace.rs:7-15`): the
  first release shows **Song** (the arranger and the track panel) and **Notes** (the piano roll),
  with the instrument list, the transport and the project file commands; it holds back **Mixer**,
  **Synth** (the patch lab), **Keyboard**, **Pads**, and the Notes view's **Automation** panel, until
  each has its own batch and its own gate. Every shown surface has a named sequence in a batch's
  gate (W-A1's, below) — the inventory is checked against those, and a surface with no sequence is
  not shown. Filing an issue is not a way to ship a pointer-only surface. **Browsers:** Chromium and
  Firefox are gated; WebKit is not — these machines have no Safari and no VoiceOver, and a Linux
  WebKit build would prove little about either — so the page says which browsers it was tested in.

- **D13 — A save is a statement about one revision of one document.** In the browser a save
  completes later than it starts, and the native `save_to` (`app.rs:797-845`) both replaces
  `self.project` with the saved copy and clears the dirty state. W5b therefore gives every save a
  document identity and a revision: a completion clears dirty only if the document is the same one
  and unchanged since the snapshot was taken, never replaces the live project with the snapshot,
  and is ignored if New or Open happened in between. **What counts as
  completion is named per path:** where the File System Access API exists, the write's `close`
  resolving; for a download, nothing the browser reports — so a download never clears dirty by
  itself, and the application asks, in a keyboard-operable and announced prompt, whether the file
  was kept. **One save per document is in flight at a time**; a second is refused with a message
  until the first ends, so an older write can never land on top of a newer one. W5b's gate: edits, a
  MIDI take and a replaced document while a completion is pending; a second save refused; a
  cancelled and a blocked download (the `.evil` and its `.mid` are two files); and the reopened
  destination inspected, not only the dirty flag.

- **D14 — The browser owns some keys, and the web build says so.** Ctrl/Cmd+N cannot be taken from
  Chromium; Ctrl/Cmd+O and S, Alt+Left/Right (history) and Ctrl+Left/Right vary by browser. The
  keyboard model is therefore designed so that **every command is reachable without a
  browser-owned chord** — through the menus and through chords the browser leaves alone — and the
  help text is generated per surface, so the web page never advertises a chord that opens a
  browser window. Which chords a browser keeps cannot be learned from Playwright, whose synthetic keys go
  to the page past the browser's own handling: those rows of the table are Jesse's observation in a
  real browser, each row marked with how it was obtained. W-A2 owns the web binding set.

- **D15 — Private tools stay private to this repository.** The engine pins wasm-bindgen 0.2.126 and
  its `build_wasm.sh` hard-fails on any other CLI; this repository's lock resolves 0.2.129.
  Installing 0.2.129 over the machine's CLI would break every game's web build. MIDI Evil's CLI is
  installed under this repository's ignored `target/tools/`
  (`cargo install wasm-bindgen-cli --version <lock's> --locked --root target/tools`) and its scripts
  call that path, never the one on `PATH`. The install is W0-web's first step, not W2's.

## The tree today (2026-11-24, `252990b`)

What the record's evidence table and its 2026-11-12 addendum need corrected before any batch acts on
them. Each was grepped today.

- **Baseline.** `cargo test --workspace` on bob-ross: 317 passed, 0 failed. `cargo check --target
  wasm32-unknown-unknown` passes today for all seven library crates — `evil-project`, `evil-synth`,
  `evil-beats`, `evil-render`, `evil-performance`, `evil-exports`, `evil-imports` — so the port's
  compile work is entirely in `crates/midi-evil`. `compat/` is absent on bob-ross; it lives on Iroh.
- **Modules.** `main.rs:1-26` declares 26 modules (the record says 24).
- **Dialogs.** Eight direct `rfd::FileDialog` uses: `beats/bank.rs:26,49,65,153`,
  `recovery.rs:61,76`, `patch_lab.rs:501,633`. `file_dialog()` is defined at `app.rs:1315` and
  called twelve times: `app.rs:552,741,785,879,893,919,1328,1340,1356,1366` and
  `app/sounds.rs:73,116`; the two at `app.rs:1356,1366` are `pick_folder`. Four confirms compare
  `MessageDialog` with `Yes`: `app.rs:380`, `beats/bank.rs:11`, `app/sounds.rs:29`,
  `app/drafts.rs:92`.
- **Clocks.** Four `Instant::now` calls — `beats.rs:241`, `beats_player.rs:61`,
  `app/recording.rs:35,81` — and the types that hold them (`beats.rs:88`, `beats_player.rs:29,37`,
  `app.rs:71`). The record's `app.rs:931` site no longer exists.
- **Threads.** `render_job.rs:61` and `recovery.rs:85`.
- **Audio out.** Three places open the device, each on a different gesture: `preview.rs:96` (Play),
  `beats_player.rs:56` (a pad), `live_player.rs:34` (a piano key).
- **Microphone.** `sample_recorder.rs` records through `cpal` directly; `patch_lab.rs:106` is its
  one caller. The record lists microphone capture under *Out* but names no seam for it.
- **The transport is not in `transport.rs`.** That file is the section range (start, end, repeat).
  The transport a person reads is drawn in `app/chrome.rs`: the playing state at `:150-174`, the
  tempo `DragValue` at `:191-204` with its "TEMPO" caption as a separate unassociated label, and the
  position at `:218-231`, drawn only when the window is at least 1160 points wide and changing
  every frame while playing.
- **Keys already taken natively:** Ctrl/Cmd+N, O, S, Z, Y and Shift+Z (`app.rs:390-415`);
  Alt+Left/Right cycles the sound (`app.rs:418-427`); Space toggles the transport (`app.rs:429`);
  the computer-keyboard piano takes A W S E D F T G Y H U J K O L P ; ' and Z, X, Escape
  (`keyboard.rs:13-30,166-184`); the pads take Z X C V A S D F Q W E R 1–4 and Escape
  (`beats.rs:19-34,245`); Delete acts in the roll, the automation lane and the mixer lane when
  *hovered* (`piano_roll.rs:373-375`, `automation.rs:396`, `mixer_automation.rs:286`).
- **The accessibility tree, as it is** (W0 step 1, `review/web/w0/baseline-tree.txt`): ninety
  nodes, all direct children of one unnamed frame, sixty-one focusable, fifteen anonymous — Save,
  the tempo, the meter, START BAR and LOOP END, the project and track names, the MIDI and SOUND
  combo boxes among them. The playing state and the position are labels focus never reaches.
  AccessKit builds no tree at all until the session's `ScreenReaderEnabled` is true.
- **Toolchain on bob-ross.** `wasm32-unknown-unknown` is installed; the CLI on `PATH` is 0.2.126,
  the engine's pin (D15). Orca is at `/usr/bin/orca`; the session is Wayland; `Atspi` 2.0 imports.

## W0 — the accessibility spike (`#2`)

Decides whether the port runs at all, and on which mechanism. Throwaway (D3). Two halves on the
real `piano_roll.rs`; the native half runs first because the web half reuses its keyboard prototype
and its strings.

**Timebox.** Two working sessions per *mechanism tried*, not per half, the clock stopped while
waiting on Jesse (D4). Proving the instruments — the build, the CLI, key delivery, the probe — is
outside the clock: an overrun there extends the half and is never a verdict. The browser-key table
is separable and may follow the web half's verdict. A half that cannot reach its gate in that time is written up as
what was tried, what blocked, and what a third attempt would need. **"Not reachable" is a verdict
about accessibility, and only Jesse gives it**: a half blocked by tooling, by a file boundary, by
key delivery or by a focus detail with a known remedy is written up as *blocked*, not as *not
reachable*.

### W0-native

1. **Prove the instrument before the experiment.** Build the application as it is. Dump the AT-SPI
   tree with the roll open and record which controls have a role, a name and a value, and which are
   anonymous — the baseline the result quotes. Then prove key delivery: the session is Wayland, and
   AT-SPI's key synthesis is built on XTest, which may not reach a Wayland-native window. Deliver
   one Tab and see the focused node change. If it does not, pick the first of these that works and
   name it in the result: run the application under XWayland (`WAYLAND_DISPLAY` unset) for the
   probe; `ydotool` over uinput; or — blocked under D4 unless Jesse waives it — a person types while the script only reads. Then
   prove speech: Orca announces focus changes, and it is not established that it announces a value
   changing on a focused custom node, or that egui 0.35 exposes a live region at all (its sources
   set none). With a ten-line change, try in order and record which produced speech: a value change
   on the focused node; a status node marked live through `accesskit_node_builder`; re-describing
   the focused node. If none speaks, that is a finding for the result now, not a surprise at the
   gate. No prototype work starts until a key is seen to arrive and one announcement is heard.
2. **Prototype** on the spike branch, in `piano_roll.rs`, a new description module, and a new
   transport-readout module — a function of a small plain state struct (playing state, tempo,
   position) that `app/chrome.rs` fills from the real application and calls. The readout is
   extracted so that the web half can include the *same file*; the spike never invents a transport:
   - the roll's painted area becomes focusable and shows a visible focus indicator; Tab and
     Shift+Tab are never consumed by it;
   - the note cursor (D6): Left/Right to the previous/next note in time order, Home/End to the
     first/last; at either end the cursor stays and says so; moving it selects the note and scrolls
     it into view;
   - on the selected note: Up/Down change pitch by a semitone and Shift+Up/Down by an octave,
     **clamped to 0–127** (`Note.pitch` is a `u8`; a note at 127 stays at 127 and says so);
     Ctrl+Left/Right move it by one snap, **saturating at tick 0 and at the pattern's last snap**;
     Shift+Left/Right change its length by one snap, **never below one snap**;
   - Enter adds a note one beat long: after the cursor's note at its pitch, or, **with no cursor or
     an empty roll, at tick 0 on middle C**; in a shared pattern, **if that start is at or past the
     pattern's end the note is not added and the roll says there is no room** — a note starting
     there fails `Pattern::validate` (`evil-project/src/pattern.rs:54`) and the project could no
     longer be saved;
   - an arrow held down moves once per repeat: the roll counts presses (`num_presses`), it does not
     ask only whether the key was pressed this frame; Delete removes the selected note when the roll *has
     focus*, not when it is hovered, and moves the cursor to the next note, else the previous, else
     to none — and an empty roll says it is empty;
   - the roll gets a role and a name that begins "Piano roll" — it is an anonymous `unknown` node
     today — and **takes focus when clicked**, which egui does only for text fields: without it a
     pointer selection is something no key can act on;
   - it takes both arrow axes with egui's focus-lock filter, without which an arrow key
     moves focus off it; Tab stays unfiltered;
   - **F6 and Shift+F6 move focus between the regions** — the toolbar, the side panel, the editor —
     because the roll is about sixty Tab stops into a flat tree of ninety ungrouped nodes, and a
     gate that reaches it only by sixty Tabs, or only through the probe's own focus call, has not
     shown it is reachable (WCAG 2.4.1);
   - D5's description for a note and for the transport, and their exposure by the two means step 1
     found on the bus: the roll's *description* carries the cursor's note (readable at any time,
     and its change is an event), each edit also goes to a separate polite live node (which emits
     `object:announcement`). **Heard by Jesse, 2026-11-24, Orca 49.4:** a change to the focused
     roll's *name* is spoken on every key ("Piano roll, step 1", "Piano roll, step 2"); the
     description's change and the live node's announcement, raised by the same key, were not heard.
     So the cursor's note goes in the roll's name, after "Piano roll", and that is the mechanism;
     whether the live node speaks when the name does *not* change in the same frame — the "no room"
     and "already the last note" messages need exactly that — is the prototype's first check. The
     transport is read
     **on demand** — one focusable, named readout giving state, tempo and position — never
     announced on repaint, and present at every window width. The tempo control gets its name.
   These chords avoid every native key under *The tree today*; D14's browser pass is W0-web's.
3. `coordination/web/probes/atspi-probe.py`, extended from step 1's: finds the roll and the
   transport readout, reaches the roll with F6 rather than its own focus call, delivers the gate's
   keys — modifier chords included, which step 1 left untested — and prints the name, role,
   description and announcement after each. It runs the application under XWayland on a private
   accessibility bus, because that is the only way found to deliver a key to this application and
   nothing else (`XSendEvent` to its own window); Jesse's sitting runs it as the desktop does, and
   the result names both setups.
4. Jesse performs the gate with Orca.

**Gate (native):** keyboard-only, and then with Orca — focus the roll; select an existing note and
get its pitch and timing; change its pitch and its start; delete it; read the transport's state,
tempo and position. Then the edges: Up on a note at pitch 127; Ctrl+Left on a note at tick 0; Enter then Delete in an
empty pattern; Enter after the last note of a full shared pattern; and select a note, press Clean
overlaps, return to the roll and delete — the note deleted is the one that was selected. No panic
in a debug build, and each says what happened.
Both kinds of evidence (D4).

### W0-web

A scratch crate on the spike branch at `spike/roll-web/`: listed in the root manifest's `exclude`
and carrying its own empty `[workspace]` table, so cargo builds it standalone and the release audit
never sees it; its `Cargo.lock` copied from the root's so wasm-bindgen resolves to the same version
as D15's CLI. A `cdylib` on `eframe::WebRunner` that includes by `#[path]` the spike's
`piano_roll.rs`, `theme.rs`, the description module and the transport-readout module, fed by a
harness-side state struct — any new `crate::`
path the spike's roll gains is added to that list in the same change — over the Bach invention
imported with `evil_imports::import_midi`. Built with D15's CLI at `target/tools/bin/wasm-bindgen`, installed as this half's first step.
Served from a local directory; nothing is synced to the site.

Mechanisms, tried in this order, stopping at the first that passes the gate:

1. **The application's own keyboard model, a live region, and a readable element.** The page gives
   the canvas a name and `role="application"`. The application writes D5's strings through
   `web-sys` to two elements beside the canvas: an `aria-live` region for changes, and a named,
   focus-synchronised element holding the *current* description, which is what a returning visitor
   or a "where am I" key reads (D11). egui already describes every focus change of a standard
   control (`PlatformOutput::events`, `OutputEvent::FocusGained` with a `WidgetInfo`,
   `egui-0.35.0/src/data/output.rs:134`); whether those can be read from inside `update` on the web
   before the runner drops them is the first thing this half finds out.
2. **A DOM twin.** Hidden, focus-synchronised DOM elements mirroring the focused control with a real
   role and value. Tried only if (1) fails the gate — it is a second representation to keep true.
3. **A full tree from egui's AccessKit update**, bridged into the DOM. Recorded as considered;
   attempted only if (2) fails and Jesse says to spend the time.

**The focus boundary is its own item, under every mechanism.** egui handles Tab for its own
traversal, and whether Tab from its last widget reaches the page depends on whether eframe's web
input prevents the default. If it traps, the remedies are tried in order and the one used is named:
release Tab to the browser at egui's first and last widget; a visible, focusable "leave the editor"
control and a documented chord. A trap with an untried remedy is *blocked*, never *not reachable*.

**The browser-key pass (D14).** For each chord in the model and each native global shortcut, in
Chromium and Firefox: does the application receive it, and does the browser also act on it. Chords
the browser may own are pressed by Jesse in a real browser during the sitting; the probe covers only
the rest. Each row says how it was obtained. The table goes in the result.

**Gate (web):** in Chromium and in Firefox, keyboard-only and then with Orca — the native gate's
sequence and its edges; **focus enters the canvas and leaves it by keyboard alone**, in both
directions (WCAG 2.1.2); **a held Right arrow across twenty notes** ends with the last note's
description readable, and nothing spoken is wrong for the note the cursor is on; **leaving the
canvas and returning** lets the visitor read the current note without moving the cursor. Both kinds
of evidence (D4): the transcript comes from `coordination/web/probes/web-probe.mjs`, run with the
site's Playwright, which presses the keys and prints the accessibility snapshot and both elements after each. The held
key is driven two ways: one repeat-flagged keydown per animation frame, which is how a real held
key arrives, asserting the description after each; and twenty in a single frame, which must also
move twenty notes.

### W0's result

`coordination/web/w0-result.md`, tracked, reviewed with the probes by `kimi`, `codex` and `gemini`
before it commits:

- the verdict — the mechanism W-A1 and W-A2 build on; or *blocked*, with what would unblock it; or,
  from Jesse, "not reachable";
- per half: what was built, the gate's transcript quoted, what Jesse heard, what failed first;
- how keys were delivered natively; the focus-boundary remedy used; the browser-key table;
- whether D5, D6 and D11 held, and the keyboard model as it stood at the end, chord by chord, edges
  included;
- which standard egui controls came through unnamed, by module;
- anything the later batches must change because of it.

A "not reachable" verdict parks W1–W6 and W-A2: the milestone's issues are labelled parked and the
site's `/midi-evil/` sentence is corrected. W-A1 is then Jesse's separate call.

**Leaves out:** the arranger, the instrument list, creating a project and saving; any change to
`jesse`'s application code; any site change.

## W-A1 — the keyboard model, natively (`#8`, first half)

On W0's mechanism and its closing keyboard model. Re-detailed after W0's result (D9); the shape:

- D5's description functions in one module, unit-tested as strings, the shared-pattern case
  included;
- the roll's cursor and edits as W0 left them, rewritten for production, every edit a tested
  function of `(sequence, cursor, key)` with W0's edges as cases — the pointer path and the key path
  call the same edits;
- the arranger on the same cursor shape (parts in time order per track, tracks by Up/Down);
- the instrument list and the transport readout named and readable; the regions grouped and named,
  with F6 between them;
- every control W0's result listed as unnamed **in the modules this batch owns** gets a name; the
  rest of that list is carried, by module, into D12's inventory;
- **Delete has one owner, the focused editor.** Moving only the roll's Delete to focus would leave
  the automation lane and the mixer lane deleting on hover from the same keypress — one key, two
  deletions. All three sites move to focus in this batch (`piano_roll.rs:373`, `automation.rs:396`,
  `mixer_automation.rs:286`), though the two lanes stay held back from the web;
- the cursor's identity rule (D6), with select → Clean overlaps → edit and select → undo → edit as
  integration tests;
- every command reachable from the menus by keyboard (D14), and the keyboard notes in the
  application's own help, generated per surface.

**Files:** `piano_roll.rs`, `arranger.rs`, `instruments.rs`, `app/chrome.rs`, `app.rs`, the
description and transport-readout modules, and the Delete line in `automation.rs` and
`mixer_automation.rs`.

**Gate:** natively, keyboard-only and with Orca, both kinds of evidence, the probe extended to
each: the record's nine-step sequence; and one sequence per surface D12 shows that the nine steps do
not reach — *Song:* move between tracks and parts, hear a part's placement, move a part, open it in
the roll; *the track panel:* rename, mute and re-assign a track's instrument; *files:* open an
existing project and hear that it loaded. A surface whose sequence does not pass is moved to D12's
held-back list before the batch reports done; the native suite, clippy and fmt; `compat/` verified
on Iroh, since this batch does not touch the shared crates but the rule is the repository's.

**Leaves out:** anything `wasm32`; the Mixer, Synth, Keyboard and Pads views and the Automation
panel — beyond the Delete line above — each of which is held back from the web build under D12 and
filed as its own issue, in the milestone, before the batch reports done.

## The later batches — constraints, not yet sections

Each is the record's step plus what follows. Each gets its files and gates in a reviewed revision of
this file after W0's result, and none starts before that.

- **W1 (`#3`)** — with *The tree today*'s counts; plus the microphone seam (`sample_recorder` behind
  `cfg(not(target_arch = "wasm32"))` with a stub whose `start` returns "not available in the
  browser"); `#10` closes here. The three device-open sites are W3's.
- **W2 (`#4`)** — the CLI under `target/tools/` (D15); the bundle is not synced to the site's
  tracked tree until W6.
- **W-A2 (`#8`, second half)** — W0's web mechanism in `crates/evil-web` and the library, fed by
  D5; the focus boundary; the web binding set and per-surface help (D14). Gate: W0-web's gate in the
  real crate, both browsers, with its own Orca sitting — the scratch harness proved the mechanism,
  not this crate.
- **W3 (`#5`)** — all three device-open sites gesture-gated, and the gate is **one gesture per
  site**: click Play, press a pad, press a piano key, each audible in Chromium and Firefox. (The
  pads are held back from the first page under D12; their site is still gated and tested, so
  showing them later is not an audio batch.)
- **W5a (`#7`, first half)** — the byte interfaces in the shared crates (D10). The old path-based
  signatures stay as thin wrappers over the resolver, so nothing outside this workspace breaks, and
  `compat/` is verified on Iroh as part of the gate.
- **W4 (`#6`)** — the Web Worker (D10). Gate, on wasm, with in-memory inputs and outputs: a long
  *single-track* render keeps the page answering within a stated frame-time bound, measured and
  quoted; a WAV-recovery draft computed from in-memory bytes completes in the worker (picking is
  W5b's). The worker's failures are part of the batch, not an afterthought: a trap, an
  out-of-memory or a missing source ends in a visible, announced error with the export usable
  again; a render can be cancelled; a maximum render length is stated and enforced; and the gate
  includes a forced worker abort and a missing source.
- **W5b (`#7`, second half)** — the browser's dialogs, saves and downloads, with D13's ownership
  rule. Gate: the record's round-trips, plus edits, a MIDI take, a replaced document and
  overlapping saves while a completion is pending; and the end-to-end render and recovery, picked
  file to download.
- **W6 (`#9`)** — D12's inventory gate, the nine-step sequence in the browser with both kinds of
  evidence, and `insiculous_web`'s `npm run verify`.

## The record's corrections

Committed to `docs/web-port.md` with this plan when it settles, as a dated section in the record's
own style: the split of W-A and of W5 and the new order (D2); W4 is a worker (D10); the shipping
gate is the inventory, with the nine steps as its minimum (D12); the transport's real location; and
*The tree today*'s counts. The milestone's description is updated to match.

## What is asked of Jesse

1. Settle this plan. **D2** (the order), **D4** (two kinds of evidence), **D10** (a worker, not
   chunking) and **D12** (the first page holds back the patch lab, pads, mixer and automation) he
   ruled on 2026-11-24, as recommended; round 1's twenty dispositions stand with them.
2. `kimi` logged in again on bob-ross, so the next round has its default reviewer.
3. Five Orca sittings over the effort: W0-native, W0-web (with the browser-owned chords), W-A1,
   W-A2, W6 — each short, each booked when its batch reaches it.
4. The ruling if a half of W0 ends anything but passed.
