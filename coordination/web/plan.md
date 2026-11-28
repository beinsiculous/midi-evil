# MIDI Evil Web — the effort's plan

MIDI Evil in the browser at `/midi-evil/play/`. `docs/web-port.md` is the feasibility record and the
ordered steps; this file is the plan that executes them: the decisions, the batches, each batch's
files and gates, and what each leaves out. Where the two disagree about *what a step is*, the record
wins and this file is corrected; where they disagree about *order, roles, evidence or a step's
gate*, this file wins, because the record says those are this plan's to settle — and the record is
corrected in the same commit that settles this plan (see *The record's corrections*).

Milestone **MIDI Evil Web** in `beinsiculous/midi-evil`: W0 `#2`, W1 `#3`, W2 `#4`, W3 `#5`,
W4 `#6`, W5 `#7`, W-A `#8`, W6 `#9`. Branch `jesse` throughout.

**Status: SETTLED by Jesse, 2026-11-24 (v4, committed `b1d2a8f`). W0 PASSED the same day —
`coordination/web/w0-result.md`. Next: W-A1's section is re-detailed from that result and reviewed
before it is handed off; W1 to W6 and W-A2 likewise, each before it starts.** Rounds 1 and
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
  on 0.35; a bump is its own batch, and its gate is the native suite, the native gate **and the web
  gate in both browsers** — W0-web's until W-A2's production gate exists, that one after, since the
  spike's harness goes when W6 ships — the web mechanism leans on eframe's private internals (its hidden text
  entry, its Tab handling, when it takes the frame's output), and a bump that breaks them must not
  be found out at W-A2.

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
  browser window. **A chord a browser keeps is not bound at all in
  the web build** — compiled out, with a row asserting it does nothing — and the path that
  replaces it is named: Alt+Left/Right cycles a track's sound natively and is the browser's Back.
  Which chords a browser keeps cannot be learned from Playwright, whose synthetic keys go
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

**Status: SETTLED by Jesse, 2026-11-24, re-detailed from `coordination/web/w0-result.md`.** Line
numbers are `jesse` at `fc2713a`. The result's two sections of requirements — what the spike's
review found, and what the result's own review added — are this section's inputs; where one becomes
a gate row it is named **R-n** below so nothing is carried as prose only.

Native only: nothing here compiles for `wasm32` or touches the site. Three batches, one out at a
time, each built by Opus from a handoff, reviewed (`deepseek` for `kimi` while kimi has no
credential on bob-ross, `codex` for the screen, `gemini` for the probe), and committed before the
next is handed off. Each batch's gate is the mechanical transcript **and** a sitting (D4): the
probe's rows run under XWayland on a private bus and say so; Jesse's sitting is Wayland-native and
is the only evidence from the configuration people run.

### The design, which all three batches share

- **One module says things.** `speech.rs` (new): `describe_note`, `describe_part`, `describe_track`,
  `describe_transport` — pure functions of the model and a stated origin (D5) — and nothing else.
  Unit-tested as strings, the shared-pattern and 6/8 cases included.
- **One module speaks them.** `announcer.rs` (new) owns the two channels W0 proved and the rule
  between them, so no widget hand-rolls it:
  - a focused widget's **name** is state: what the cursor is on *now*, and nothing else. **An
    outcome never goes in a name** (R-5);
  - an **outcome** — "Removed.", "Nothing after this one.", "Added." — goes to the polite live node.
    When an outcome and a name change come from one key, the outcome goes out in that frame and the
    name's change in the next, **with a repaint requested**, since in egui's reactive mode the next
    frame otherwise may not come (R-7);
  - **a held key says where it came to rest, once.** A name that changes on every repeat is twenty
    interrupted utterances, or a queue that is still reading an earlier note when the person
    presses Delete on a later one. While a key is repeating, the focused name is not republished;
    when the hold ends, the resting item's name is published once. The visible status line keeps
    up throughout. An outcome identical to the one just announced within the same hold is not
    repeated. (W0's sitting did not judge a held key; W-A1a's does, and may overturn this for a
    bounded rate instead.) A hold ends on the key's release, **on the widget losing focus, or on any key event
    that is not a repeat** — a release that happens in another window never arrives (R-7).
- **One shape for a custom editor.** A custom-painted editor is one focusable widget with a role, a
  name that begins with what it is, a cursor, a focus-lock filter on both arrow axes with handled
  keys **consumed** (R-2), focus on click, and Tab never taken. The roll and the arranger are both
  this; the edits are pure functions of `(model, cursor, key)` returning the new cursor and an
  outcome, and the pointer path calls the same edits.
- **A cursor is an identity, not an index — and identity is kept by whoever changes the list, not
  guessed afterwards.** Notes and parts have no ids in the model, and two rounds of review showed
  that no comparison of the list before and after can tell a note from its twin, an edit in place
  from a different track arriving at the same index, or a moved note undone. So every way the
  edited sequence can change is one of five kinds, and each says what happened to the cursor:
  1. **The editor's own edits** — a key, a toolbar button, a pointer gesture, the inspector's
     fields — are functions that return the new cursor. No reconciliation.
  2. **Undo and redo restore the cursor with the project.** `EditorHistory` (`history.rs`) keeps
     whole `Project` snapshots; each gains the editor's selection as it was — view, track, part,
     cursor — and restoring a snapshot restores that selection (less the view: see W-A1a's
     outcome). The selection stored with a
     snapshot is the one captured **at the start of the frame, beside `previous_project`**
     (`finish_project_frame`, `app.rs:854-875`) — the selection that belonged to that project, not
     the one the edit left behind; the state being left is stored with *its* selection, so redo
     returns to it. Undo during a take goes the same way: the take's `recording_snapshot`
     (`app.rs:259-278`) carries the selection captured when the take began. Undoing a move puts the cursor
     back on the note where it was; undoing the insertion of a twin puts it where it was before
     the twin existed (R-6, R-9); undoing a duplicated track cannot leave the cursor on whichever
     track now has that index (R-15).
  3. **A change of what is being edited** — another track, part or pattern selected, a document
     opened or replaced, an arranger command that changes which sequence the roll shows — resets
     the cursor to none. The cursor carries its context (document generation, track, part,
     pattern) and is never compared across a different one.
  4. **A whole-sequence operation the editor starts** — Quantize, the transposes and Clean overlaps
     with no selection, Paste — keeps the count or grows it, and may re-sort. The cursor follows
     its note only when the note is unique by value before and occurs once after; failing that,
     and only when the count is unchanged, when it is unique by start and pitch on both sides
     (Clean overlaps shortens the notes it re-sorts); otherwise it is cleared.
  5. **Anything else** — a take adding notes under the editor — is treated as kind 4.
  Whenever the cursor is cleared by anything but the person's own selection, the roll says "The
  selection was reset." It never silently points at a different note.
  **The toolbar's buttons that edit one note** — Earlier, Later, Shorter, Longer, Delete — act on
  the cursor and are disabled without one; they do **not** inherit the older buttons' convention
  that no selection means every note, which would turn a second press into an edit of the whole
  track.
- **Keys are dispatched once, in the order they arrived.** Today `StudioApp::ui` runs the global
  `shortcuts()` before any editor is drawn, so Delete then Ctrl+Z arriving in one frame is undone
  first and deleted second: the wrong result, from fast hands or one slow frame. Key events are
  walked once per frame in order, each going to the focused editor if it takes that chord and to
  the global commands if not, each consumed by whoever handles it. Counting presses per key cannot
  do this; W0 already found it cannot tell Right from Ctrl+Right.
- **Escape clears the cursor, and says what that means.** The older toolbar commands — Quantize,
  the transposes, Copy — act on the selected note, or on every note when none is selected, and a
  keyboard that can only ever select makes the second meaning unreachable. With a cursor, Escape
  clears it and the roll says "No note selected; Quantize, transpose and Copy apply to all notes";
  without one, Escape is egui's own.
- **Commands refuse what recording forbids.** The arranger's pointer controls already stop
  structural edits during a take (`allow_structural_edits`, `app.rs:1495`, `arranger.rs:22`),
  because the recorder holds its target by index. That check moves to the shared command boundary,
  so a key or a button cannot do what a click may not; a refused command is announced.
- **Regions.** Toolbar, side panel, editor: each a named group in the accessibility tree, so the
  ninety flat nodes become three sets, and **F6 / Shift+F6** move between them. A widget's region
  is known from the panel it was built in, never from its rectangle (R-3), and a region's stop
  that no longer exists falls back to that region's first focusable widget (R-8).
- **Delete has one owner: the focused editor.** `piano_roll.rs:373`, `automation.rs:394` and
  `mixer_automation.rs:284` all gate Delete on *hover* today; all three move to focus, so one
  keypress can never delete in two places (R-4). Moving the predicate alone would break the lanes:
  they are `click_and_drag` painters that never take focus, so a pointer user who clicks a point
  and presses Delete would delete nothing — or a note, if the roll still held focus. Both lanes
  therefore **take focus when a point is clicked or dragged** and consume the Delete they handle.
  That much of the custom-editor shape, and no more: their keyboard *editing* stays held back.
- **Every edit has a command that is not a chord.** D14 says every command must be reachable
  without a chord a browser may keep, and Ctrl+Left/Right is such a chord. The roll's toolbar
  already has Quantize, the transposes, Clean overlaps, Copy and Paste; it gains Earlier, Later,
  Shorter, Longer, Add and Delete, each calling the same edit as its chord.

### W-A1a — the roll, the announcer, the regions — DONE 2026-11-24

Built by Claude Opus 5.5 over five passes; 419 tests, the native gate green three runs running, and
Jesse's sitting passed: "yep it passes, speaking at rest is fine". What differed from the section
below, and what it leaves the next two batches:

- **Undo and redo do not restore the view.** The design's kind 2 said they would; that is
  withdrawn. The workspace is where the person is, not what was selected — and restoring it let a
  key later in the same frame edit a roll no longer shown. They restore track, part and cursor.
- **One undo entry per hold is a rule of the history, not of the frame.** The first attempt
  postponed the end-of-frame bookkeeping while a nudge was held, and four defects came out of it —
  the unsaved-changes guard waiting on a key, batches spanning different keys, the toolbar's Undo
  corrupting the stack, Save As recording an unrebased snapshot. `EditorHistory::record` now takes
  the gesture an edit belongs to and does not push when the entry on top is the same gesture's; a
  roll key's edit is recorded in the dispatch, at that key; every frame ends as it always did.
  Left as it is: a hold that crosses a Save As is two entries.
- **A repeat is not a new command.** Only the cursor's moves and the nudges act on a key's repeats.
- **The dispatch asks whether a text field is being edited**, not whether anything has focus, and a
  focused text field keeps its own undo; nothing is claimed behind an open menu or window; keys
  after a Tab in the same frame do not reach the editor being left.
- **R-17's first fixture was wrong** — moving C 5 onto B 4's start re-sorts nothing; the gate moves
  B 4 before C 5 instead. **R-7's first press publishes its name**; the hold begins at the first
  repeat.
- **The transport chord is plain `I`.** The lanes are named ("Automation lane, Pitch bend,
  1 point") and take focus on click.
- **egui does not replay a frame's keys on a second pass** (`RawInput::take` moves them out,
  `data/input/raw_input.rs:117-137`), so no pass guard was needed.
- **For W-A1b: a disabled button is never reported disabled.** AccessKit's AT-SPI adapter reports
  that state only for roles that can be read-only (`accesskit_atspi_common-0.18.1/src/node.rs:350`);
  every greyed-out button in the application reaches Orca as available. W-A1b gives each disabled
  button a description saying why. **Ruled by Jesse, 2026-11-24: "go with the descriptions".**
- **For W-A1c:** the arranger's own `allow_structural_edits` checks move to the command boundary
  with the arranger's keys; `describe_part` and `describe_track` are written when something calls
  them.
- **The probe** waits for a node it is about to press and for a name it expects, after one gate
  run in four lost two rows to a half-second window on a loaded desktop.

**Files:** `speech.rs`, `announcer.rs`, `regions.rs` (new); `piano_roll.rs` (687 lines — the cursor
and the edits go to a new `piano_roll/cursor.rs` and `piano_roll/edits.rs`, so no file passes 600);
`app.rs` (1,754 lines: only the F6 handling, the region wiring and the `mod` lines; anything more
is moved out, not added); `app/chrome.rs` for the panels' groups; `automation.rs`,
`mixer_automation.rs` for the Delete line; `coordination/web/probes/atspi-probe.py`,
`native-gate.sh`, `README.md`.

**Builds:** the roll as W0 left its keyboard model (the table in the result), on the design above;
the key help as the roll's description, complete — Home, End and the octave chords included, and
"Command" where the platform's command key is not Control (R-10); an edit in a shared pattern says
its reach; a way to hear the transport's position **from inside the roll** without leaving it (the
chord is the batch's to propose, checked against *The tree today*'s taken keys, and reported).

**The probe refuses to be dangerous, and so does its launcher.** `run-probe.sh` has already set
the screen-reader flag, started a registry and launched the application before Python runs, so a
check only in the probe would refuse after the damage. The launcher's inner entry verifies, before
its first bus call or child process, that its session bus is the one `dbus-run-session` gave it,
that its runtime directory is the private one, and that the settings backend is the in-memory one;
the probe checks the same again before it sends a key. The two incidents of W0 were a launcher
whose environment had slipped. A gate row invokes the inner entry with the desktop's environment
and asserts the refusal — and that the desktop's flag and its settings file were not touched.

**Gate rows** (each in `native-gate.sh`, each also a unit or integration test where it can be):

| row | what it shows |
|---|---|
| W0's sequence and its five edges, less the transport | carried over, with the outcomes now in announcements and absent from names; the transport's step joins in W-A1b, which re-runs the whole sequence |
| R-1 | two identical notes; select one; change its velocity in the inspector over several frames — the selection never moves to the twin |
| R-2 | F6 to the roll and an arrow **in the very next frame**: the cursor moves and focus stays on the roll |
| R-3 | a bass part (notes around pitch 36): F6 from the roll reaches the toolbar, Shift+F6 returns |
| R-4 | roll focused, pointer over the automation lane with a point selected, Delete: the note goes and the point stays. And the reverse, for each lane: click a point, Delete: that point goes and no note does |
| R-5 | Delete, Tab, Shift+Tab: the roll's name is the cursor's note with no "deleted" in it |
| R-6 | select the last note, undo the edit that added it: "The selection was reset." is announced |
| R-7 | Right held across twenty notes: **one** name change, for the resting note, after the release, and none during the hold; Up held at pitch 127 announces "Cannot go higher." once per hold, and again after a release; a held name change is never left unrepainted. The probe sends repeats and then an explicit release |
| R-8 | delete the last track from the Notes view: F6's editor stop is the "+ Add a track" button |
| R-9 | two identical notes; select the first; add a twin after it (the cursor moves to the twin); undo: the cursor is back on the first, and Delete removes that one |
| R-15 | two tracks with one different note each; duplicate the first, select the copy's note, undo the duplication: the cursor is not on the second track's note, and Delete removes nothing of it |
| R-16 | cursor on the first of three notes; Ctrl+Right past the second; Ctrl+Z: the cursor is on the moved-back note and its name says so |
| R-17 | the Clean overlaps edge, fixture stated: the person moves C 5 onto B 4's start with Ctrl+Right — the editor's own edit — and then presses Clean overlaps, which re-sorts the two and changes neither: C 5 is unique by value before and after, and the cursor follows it. And a same-pitch overlap that Clean shortens, its start unchanged: the cursor follows by start and pitch |
| R-18 | Delete then Ctrl+Z delivered in one frame leaves the note present; Ctrl+Z then Delete in one frame deletes; Right then Delete in one frame deletes the note moved to |
| R-19 | navigate to a note, Escape: the cursor is cleared and the announcement says operations apply to all notes; Quantize then changes every note; Escape again leaves focus as egui does |
| R-20 | start a take with the cursor on a note, Ctrl+Z during it: the cursor is where it was when the take began, or the roll says the selection was reset |
| R-10 | the roll's description names every chord in the model |
| R-14 | with the roll focused, every edit made from its toolbar button alone — Earlier, Later, Shorter, Longer, Add, Delete — each announced as its chord is; Later pressed twice across a neighbouring note moves the same note twice; with no cursor the one-note buttons are disabled |

**Sitting:** the roll's sequence; a held Right and a held Up at the top; R-5 by ear.

**Leaves out:** the arranger, the transport's own readout, names for controls outside the roll.

### W-A1b — the transport, and a name for everything on a shown surface — DONE 2026-11-24

Built by Claude Opus 5.5 in two passes; 446 tests and the native gate at 320 expectations. What
differed from the section below, and what it leaves W-A1c:

- **The readout** is as the proof decided, the rule in `Announcer::on_demand_name`. One addition:
  Enter on a reading that has not changed is announced, since an unchanged name says nothing.
- **Everything could be named**, scroll bars included; no waiver was needed. Two named Tab stops do
  nothing from the keyboard: the side panel's resize handle and the arranger's bar ruler. The ruler
  gets its key path in W-A1c; the handle is Jesse's call.
- **A screen reader started after the application works**: AccessKit watches the flag, and R-13
  shows the tree appearing.
- **The transport plays in the gate through a clock seam**, not a device: with
  `MIDI_EVIL_SILENT_PLAYBACK` set, and only in a debug build, the real player feeds an output that
  is pulled at the sample rate and thrown away. ALSA's null device does not keep time.
- **`Project::timing()`** is new in `evil-project`: the tempo map with no tracks. The readout asks
  for the playhead every frame, and a whole performance expands every pattern in the song.
  `compat/` is still to be verified on Iroh.
- **For W-A1c:** `describe_part` and `describe_track` now exist and the arranger's lanes and parts
  are named buttons; its cursor, its keys and its recording guard are still W-A1c's.
- **The gate on a desktop in use:** when the compositor will not give a new XWayland window
  keyboard focus — which happens while a person is typing elsewhere — the probe refuses to send
  keys and the gate fails. That is the probe working. A gate run is retried, not waived.

**Files:** `transport_readout.rs` (new); `app/chrome.rs` (744 lines — the toolbar's transport block
moves into the new module, which brings it under 600); `app/sounds.rs`; `midi_input.rs`;
`instruments.rs`; the probe and gate.

**Builds:**

- **The transport readout**: one focusable, named status in the toolbar, present at every window
  width, giving state, tempo, meter and position, the position in bars and beats as well as time.
  It must be true whenever it is read and silent between reads (R-11).
  **The proof, heard by Jesse 2026-11-24 (Orca 49.4):** a focused status node whose *name* changes
  on a timer is spoken at every change; one whose name is fixed and whose *description and numeric
  value* change is spoken even more. Nothing about a focused node may change on a timer. Orca's
  own rules say the rest (`orca/ax_utilities_event.py`): a name change is presented only when its
  source **is** the focused object; a description change when its source is the focused object
  **or an ancestor of it**. So:
  - **while the readout is not focused, its name is kept current** — silently, since it is not the
    focus — so that the moment focus arrives, what is spoken is true;
  - **while it is focused, nothing about it changes by itself.** Enter reads again: the name is
    brought up to date once, and that change is the announcement. Its description is fixed help —
    "Enter reads the transport again" — which is also what a screen reader's own read-current-object
    gives, with the name as of the last read;
  - **from the roll, `I`** announces it through the announcer, as W-A1a built;
  - **no region's description ever changes**: a region is an ancestor of whatever is focused in it.
  The visible position label beside it keeps counting; it is never the focus.
- **A name for every control on the Song and Notes views, the toolbar and the side panel** — the
  fifteen anonymous nodes of W0's baseline and any since. By module — `app/chrome.rs`: About, Save,
  the project name, tempo, meter, START BAR, LOOP END, the track name, the note inspector's four
  values; `app/sounds.rs`: SOUND; `midi_input.rs`: the MIDI port. Each `DragValue` and `TextEdit`
  gets its label **as its accessible label**, not as a neighbouring text node, because on the web
  that label is what names eframe's text entry (W0-web). egui's own scroll bars and the panel's
  resize handle are named, or taken out of the Tab order and the tree (R-12). If egui 0.35 allows
  neither, the batch stops there and reports it: a silent Tab stop on a shown surface is Jesse's
  ruling, as a written waiver naming each stop, not the batch's to accept. The window's title is
  exposed.
- **A disabled button says why.** AccessKit's AT-SPI adapter never reports a button as disabled, so
  Orca hears every greyed-out button as available (W-A1a). Every button on a shown surface that can
  be disabled carries a description while it is — "Select a note first", "Nothing to undo",
  "Not while recording" — which is also its hover text, so a sighted person reads the same reason.
  The reasons live in `speech.rs`. Gate row R-22: each such button, in its disabled state, has a
  non-empty description, and loses it when enabled.
- **The instrument list** readable and operable: which sound a track has, how many there are,
  Alt+Left/Right's cycling announced by the announcer rather than by a status label nobody hears.
- **A screen reader started after the application** (R-13): the batch finds out whether the tree
  appears when `ScreenReaderEnabled` turns true after launch — the probe can flip it on its private
  bus. If it does, a gate row shows it. If it does not, and the remedy is inside eframe, which D8
  freezes, the application says what to do where a person who cannot see it will still find it —
  the README and the site's page, since a silent window cannot speak — and the row asserts the
  tree stays absent, so a later eframe that fixes it is noticed.

**Gate rows:** W0's whole sequence, the transport's step now included. R-21 — with the readout
focused: change the tempo, read; Ctrl+N, read: each reading is the new project's, with no move of
focus between. R-11 — read the transport
stopped (Enter), press Space, wait, read again with Enter without moving focus: it says playing and
a later position, and the song is still playing; Space; read again: stopped; and **between the reads, with focus resting on it while it plays, no
announcement and no change to its name, description or value** for five seconds — the probe counts
the events, and the count is zero. With focus elsewhere while it plays, its name does change. (The probe's private
session has no audio device to open; the row uses the application's existing null-device path or a
clock stub the batch names — playback's *sound* is not what the row is about.) R-12 — a tree dump
of each shown view with **no focusable node unnamed**, asserted by the probe, not read by eye.
R-13 — the application started with the flag off, the flag turned on, the tree present.

**Sitting:** the transport read stopped and playing; Tab through the toolbar and the side panel
hearing a name at every stop; a tempo typed and committed.

**Leaves out:** the Mixer, Synth, Keyboard and Pads views and the Automation panel, beyond the
Delete line — held back from the web build under D12, each its own issue in the milestone, filed
by the planner before W-A1 reports done.

### W-A1c — the arranger, the files, and the nine steps — DONE 2026-11-25

Built by Claude Opus 5.5 in two passes; 502 tests and the native gate at 455
expectations. What differed from the section below:

- **The arranger is one widget**, "Song, …", whose cursor is the roll's own selection: a track
  and one of its parts, or its unique timeline. The lanes and parts W-A1b named as buttons are
  gone; the pointer hit-tests the one widget and sends the same commands as the keys. Home is the
  timeline, End the last part; B is the ruler's click; I reads the transport.
- **The ruler's click was never "place the playhead"**: it sets the start bar and loops the bar
  after it. B does the same from the cursor's part.
- **One boundary**, `run_song_command`, takes every arranger command from a key, a button, the
  inspector or the pointer; a take refuses the ones that change parts, and a change of track,
  with "Not while recording." A track is selected in one place, `select_sound_track`.
- **One way to report**: `report` writes the status line and the announcer together. An outcome
  that follows a focus move waits until the newly focused widget has published its name.
- **The gate cannot hear a project opened by launch argument** — it announces before any
  listener is on the bus. The probe's `--open-in-running` hands a file to the running instance
  through its socket instead, as a second launch from a file manager does. The saved file lives
  in the gate's own fixture directory, since a relaunch must outlive one run. A new project's
  Save is Save As, so the nine steps' save row runs on an opened project; Save As on a new one
  is the sitting's, under the waiver.
- **The inventory** is `coordination/web/w-a1-inventory.md`, not a section of the result file:
  65 announced, 9 opening a held-back surface, 40 standard, and six failing items — Refresh MIDI
  inputs (W3's), three commands with no test past their dialog, the side panel's width handle,
  and moving a window by its title bar. The five that are not W3's are one issue in the
  milestone.
- **The keyboard notes** come from `key_help.rs`, one table per surface, shown by Project ›
  Keyboard notes…; the key dispatch reads the same tables.


**Files:** `arranger.rs` (536 lines — the cursor and edits to `arranger/cursor.rs`); `app.rs` and
`app/chrome.rs` for the file commands' announcements and the help; the probe and gate.

**Builds:**

- **The arranger as a custom editor** on the shared shape: one focusable widget named "Song", a
  cursor over (track, part) — Up/Down between tracks, Left/Right between a track's parts in time
  order, Home/End — whose name says the track, its sound, the part's pattern, its placement in song
  bars and how many parts share the pattern; Ctrl+Left/Right move the part by a bar; Enter opens it
  in the roll and moves focus there; Delete removes the part. A track with a unique timeline says
  so. The ruler's click-to-place-the-playhead (`arranger.rs:114-135`) gets a key path.
- **The track panel** by keyboard: add a synth or a drum track, rename, mute, choose its sound —
  each with an announced outcome.
- **The file commands**: New, Open, Save and Save As reachable from the menu by keyboard, each
  announcing what happened ("Saved song.evil", "Opened …, 4 tracks", "Discarded" or not). The
  native dialogs are the desktop's own.
- **An inventory, not a sample.** A surface passing one sequence does not pass its other controls:
  the arranger alone has Unique timeline, Loop part, Make unique and Merge to timeline, whose
  outcomes go to a status line nobody hears. The batch lists **every control, menu item, window
  and toggle reachable on the shown surfaces** — the Song and Notes views, the toolbar, the side
  panel, the Project menu, the About window, the sound library — from a tree dump of each, not
  from `workspace::View`, which cannot see a window, a menu or the Automation toggle. Each line
  ends one of three ways: it has an announced outcome and a gate row; it opens a held-back surface
  and is hidden in the web build, with its absence asserted in W6; or it is a named, standard
  control whose activation changes nothing to announce. A tree dump cannot see what has no node:
  the roll's drag-to-resize, the arranger's drag-to-move, the ruler's click. So the list is
  reconciled against every `Sense::click`, `Sense::drag` and `ui.interact` site in the shown
  views' modules, and each pointer gesture found there has a key path named beside it or is a
  line of its own that fails. The list lands in `w-a1-result.md` and is
  what W6's inventory gate is checked against (D12).
- **The keyboard notes** in the application's help, generated from one table per surface, so W-A2
  can produce the web's from the same table with the browser's chords left out (D14).

**Gate:** the record's nine-step sequence end to end — new project, add an instrument, discover the
instruments, add a note, select it and hear pitch and timing, change it, delete it, read the
transport, save — and D12's per-surface sequences: *Song* — move between tracks and parts, hear a
placement, move a part, open it in the roll; *the track panel* — rename, mute, re-assign a sound;
*files* — open an existing project and hear that it loaded. The probe cannot drive the desktop's file dialog — it is another window, and the
probe's keys go to the application's own. So the mechanical file steps avoid it: the application
already opens a project named on its command line, and Save on a project that has a path writes
there without a dialog. The gate's rows are therefore *open by launch argument and hear that it
loaded*; *edit, Ctrl+S, hear "Saved …"*; *relaunch on the saved file and find the edit* — in the
probe's private runtime directory. What stays outside any transcript is the dialog itself: Open
from the menu, and Save As on a new project. **Those two steps are the sitting's alone under Jesse's
written waiver (D4), granted 2026-11-24** and recorded under *What is asked of Jesse*. A surface whose sequence does not pass is moved to D12's held-back
list before the batch reports done.

**Sitting:** the nine steps, start to finish, Wayland-native, with Orca.

**Then, for all three:** the native suite, clippy and fmt; `compat/` verified on Iroh; the nine
steps' transcript and what Jesse heard added to a `coordination/web/w-a1-result.md`.

## W1 — compile-safe seams, native behaviour unchanged (`#3`) — BUILT 2026-11-25

Built by Claude Opus 5.5 in one pass; 510 tests and the native gate at 455 expectations. What
differed from the section below:

- **The `wasm32` lint is scoped to the application.** As written, `cargo clippy -p midi-evil
  --target wasm32-unknown-unknown` also lints the path dependencies, and `evil-project`'s atomic
  save trips `drop_non_drop` there, where a file has nothing to drop. W1 may not touch a shared
  crate, so its gate is the same command with `--no-deps`. The shared crates' own `wasm32` lint
  is W5a's, which changes them anyway.
- **The `wasm32` dialogs are `impl Dialogs for ()`**, not a named type: nothing constructs a
  `StudioApp` there yet, and a private named type would be dead code.
- **Each site still reads as a builder.** A crate-private `FileDialog` mirrors the calls the
  sites made, and asks through the trait; the trait's methods take plain types, so no request
  type is public. No site asks with a title.
- **All twenty-three uses are reached by the harness** and their requests asserted; the eleven
  that stop the keyboard and the beats are tested holding a note, the pedal, a bend and a pad.
- **The inventory** now has 68 announced lines and three failing items: Refresh MIDI inputs
  (W3's), the side panel's width handle, and moving a window by its title bar.


The record's step, re-surveyed on `a15dc49` after W-A1 moved most of what it names.

### The tree, for this batch

- **What fails today.** `cargo check -p midi-evil --target wasm32-unknown-unknown` ends in nineteen
  errors, in three places: the synchronous `rfd` dialogs, which do not exist on `wasm32` (sixteen,
  with the type inferences that hang off them); `std::os::unix` in `instance.rs` (one); and
  `eframe::NativeOptions` and `run_native` in `main.rs` (two). `rodio`, `cpal` and `midir` compile
  for the target as they are. So the port's compile work is these three seams and nothing else;
  what compiles and then cannot run — a clock, a thread, a device — is named below and is not
  W1's to make work.
- **Dialogs: twenty-three uses in seven files.** `file_dialog()` is defined at `app/files.rs:354`
  and called eleven times; direct `rfd::FileDialog` at `beats/bank.rs:26,49,65,155`,
  `recovery.rs:76,93`, `patch_lab.rs:572,704`; a yes-or-no `rfd::MessageDialog` at
  `app/files.rs:54`, `app/sounds.rs:31`, `app/drafts.rs:92`, `beats/bank.rs:11`.
- **Clocks: five calls.** `Instant::now` at `preview.rs:110`, `beats.rs:244`,
  `beats_player.rs:61`, `app/recording.rs:35,81`, and the fields that hold them. They compile
  on `wasm32` and panic when reached.
- **Threads:** `render_job.rs`, `preview.rs`, `recovery.rs`. They compile and panic when spawned;
  W4's.
- **Microphone:** `sample_recorder.rs`, one caller at `patch_lab.rs:134`. It compiles; the patch
  lab is held back from the first page (D12).
- **Modules:** `main.rs` declares 32 and holds `main` and `run`, 74 lines.

### What it builds

1. **The library, with a stated surface.** `src/lib.rs` declares the 32 modules. `run` moves
   into it as `run_native`, compiled off `wasm32`, and `main.rs` becomes a `main` that calls it
   — so the launch options, the single instance and the desktop's dialogs are wired inside the
   library and none of them has to be public for `main`'s sake. The public surface is exactly:
   `run_native`; `StudioApp` and its constructor; and the types that constructor names —
   `LaunchOptions`, `SingleInstance`, the `Dialogs` trait. Nothing else is public. `speech`,
   `announcer`, `regions` and `key_help` are *in* the library, which is what lets `evil-web`
   read D5's strings and never copy them; **which of their items become public is W-A2's to
   say, when it knows what it reads** — exporting them now would drag the roll's and the
   arranger's command types out with them. On `wasm32` `main.rs` is an empty `main`, so the
   whole package checks, not only `--lib`.
2. **One seam for every dialog.** A `dialogs` module with a `Dialogs` trait: pick a file, pick
   files, pick a folder, choose where to save — each taking a request (title, filters, starting
   directory, suggested name) and returning paths — and ask yes or no, taking a title **and a
   description** and answering true only for Yes; a closed or cancelled prompt is no. There are
   **twenty-three uses**: eleven calls of `file_dialog()`, eight direct `rfd::FileDialog`, four
   `rfd::MessageDialog`. `StudioApp` owns the one `Box<dyn Dialogs>`, given to its constructor;
   the modules that open a dialog and are not `StudioApp` — `BeatsPage` in `beats/bank.rs`,
   `RecoveryWindow` in `recovery.rs`, the sample and zone loaders in `patch_lab.rs` — are
   handed `&mut dyn Dialogs` by their caller for the call, and every signature that gains the
   parameter is listed in the report. No second owner, no global. Every dialog is opened on
   the interface thread, as today. `file_dialog()` stopped the keyboard and the beats before it
   built a dialog; a `StudioApp` method keeps doing exactly that in front of the trait, the
   eleven sites use it, and the twelve that did not stop still do not. The `rfd`
   implementation is `dialogs/native.rs`, compiled off `wasm32`, and `rfd` moves to
   `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`; on `wasm32` the implementation
   answers "nothing chosen" and "no". **The synchronous, path-returning shape is the
   desktop's.** A browser's picker is asynchronous and hands back bytes; W5b decides that shape
   with D13 and is expected to change this trait and its call sites. What W1 gives it is every
   dialog passing through one place, which it needs whatever the shape.
3. **The tests reach past the dialogs, and see what was asked.** The test harness gets a
   scripted `Dialogs`: the test queues the next answers, and it records each request — kind,
   title, description, filters, suggested name. With it:
   - Relink MIDI…, Choose WAV… and Create .evil draft… each get the test the inventory says
     they lack, on the repository's fixtures and a temporary directory, waiting for the
     recovery's worker before asserting. Their inventory lines become announced-with-a-test.
   - Every one of the twenty-three uses the harness can reach has its request asserted — a save
     asks to save, under the name and filters it does today; a confirm carries today's title
     and description — and the four confirms are tested answering yes and no. A use the
     harness cannot reach is named in the report with the reason.
   - One test shows a held note and a running beat are stopped before the eleven sites' dialog
     answers.
4. **The single instance.** `instance.rs` is `#[cfg(unix)]`; elsewhere `SingleInstance::start`
   always answers "primary" and polls nothing. That makes the crate *compile* off Unix. It is
   not a claim that Windows is supported: two copies there would both be primary.
5. **The clocks.** `web_time::Instant` at the five calls and the fields that hold them. It is
   `std::time::Instant` re-exported off `wasm32`, so native behaviour is the same type. One new
   dependency, `web-time`; on `wasm32` it brings `wasm-bindgen` and `js-sys` with it, which is
   expected and is not the batch binding anything itself.
6. **The microphone seam.** `sample_recorder.rs` is compiled off `wasm32`; on `wasm32` a stub of
   the same two functions whose `start` returns "Recording from a microphone is not available
   in the browser." `#10` closes here.

### Rules particular to this batch

- **Native behaviour does not change.** No string, no key, no focus order, no file written
  differently. Every existing test passes untouched; the tests added are item 3's.
- **The stubs are not tested, and the plan does not pretend they are.** The `wasm32` halves of
  `dialogs` and `sample_recorder` and the off-Unix half of `instance` are compiled by no native
  test run and executed by no gate here. Their evidence is that they compile and lint clean for
  the target, and each is a few lines that return a constant. W2 is the first batch that runs
  them.
- **Three files are already over 600 lines**: `patch_lab.rs` 1,777, `app.rs` 1,138, `beats.rs`
  711. None is split here. Each may grow only by the seam's wiring — a parameter, a field, an
  import — by no more than fifteen lines, counted in the report. Every other touched file stays
  at or under 600.
- **`cfg(target_arch = "wasm32")` appears in five places and no others**: `dialogs`,
  `sample_recorder`, `lib.rs` where it declares them and `run_native`, `main.rs`, and the
  manifest. `cfg(unix)` appears in `instance`. A `cfg` anywhere else means a seam is missing,
  and the executor stops and says which.
- **Nothing is made to work in a browser.** The batch writes no `wasm-bindgen` binding, no web
  crate, no worker, no audio: W2, W3 and W4.

### Gate

- The native suite, clippy and fmt; the native gate, every sequence, since the split moves every
  module the gate drives.
- `cargo check -p midi-evil --target wasm32-unknown-unknown` compiles — library and binary —
  and `cargo clippy -p midi-evil --no-deps --target wasm32-unknown-unknown -- -D warnings` is clean. A
  public function naming a private type, or a stub leaving an argument unused, fails here.
- `cargo tree -p midi-evil --target wasm32-unknown-unknown` shows no `rfd`.
- `git grep -n 'rfd::' crates/midi-evil/src` finds only `dialogs/native.rs`.
- `compat/` verified on Iroh is **not** a gate of this batch: W1 touches no shared crate.

**Sitting:** none. Nothing a person hears or presses changes; the native gate is the evidence.

**Leaves out:** the three device-open sites (W3); the threads (W4); the path-based interfaces of
the shared crates (W5a); the browser's own dialogs (W5b).

## W2 — the web crate and its gates (`#4`) — BUILT 2026-11-25

Built by Claude Opus 5.5 in two passes; 510 tests, the native gate at 455 expectations, and the
application drawing in Chromium and in Firefox. What differed from the section below:

- **The application started on its first run**, in both browsers, with no error and no panic.
  The one seam is not for a failure: constructing the application asked the browser for MIDI
  access, with sysex, on page load, and said nothing. `midi_ports` is now a pair like
  `sample_recorder`'s — on `wasm32`, no ports and no request — and the toolbar reads as it does
  natively with nothing plugged in. Web MIDI is still W3's.
- **The wasm is 5.92 MiB raw, 2.40 MiB gzipped; the budget in `build_wasm.sh` is 7 MiB.** The
  size is checked before the swap, so a build over budget leaves the last good bundle.
- **The entry installs its own logger.** eframe's writes an error as a console warning, which no
  smoke could tell from a warning; `evil-web`'s keeps an error an error.
- **The smoke has ten negative controls**, five in each browser: an empty wasm, a wrong canvas
  id, an error logged while the canvas draws, a page that asks for MIDI, and a page that never
  finishes loading. A run takes under two minutes.
- **Playwright is found beside the repository's main checkout**, through `git worktree list`,
  because a worktree does not sit beside the site.
- **CI** is `.github/workflows/wasm-check.yml`, running `scripts/check_wasm.sh` and nothing else.
  Its first run, on `d9f02d6`, was red: GitHub's stable (1.100) is newer than bob-ross's
  (1.94) and its clippy adds two lints. Fixed in `7c7c316`, whose run is green. The workflow is
  still unpinned; this is what that costs, and it is revisited if it happens again.


The record's step, on `0b1c213`, after W1 made the application a library that compiles for
`wasm32`.

### The tree, for this batch

- **The workspace** has eight members and excludes `crates/oracle` and `crates/evil-porting`.
  `tools/audit-public-packages.sh` lists the same eight in dependency order, asserts the
  workspace's membership is exactly that list, and asserts every one is publishable.
- **No script and no workflow builds for the browser.** `scripts/` holds the review tooling
  only; the repository has no `.github/workflows/`. The models are the engine's, read and not
  copied blind: `insiculous_2d/scripts/check_wasm.sh` (24 lines), `scripts/build_wasm.sh`,
  `.github/workflows/wasm-check.yml`, and its `[profile.wasm-release]` (`opt-level = "s"`,
  `lto = true`, `strip = "debuginfo"`).
- **The CLI** is at `target/tools/bin/wasm-bindgen`, 0.2.129, the lock's version (D15).
  `target/` is ignored, so a fresh checkout and CI have none.
- **What a browser entry can construct.** `StudioApp::new(context, options, instance, dialogs)`:
  `LaunchOptions` derives `Default`; the instance is `None`; `()` implements `Dialogs` on
  `wasm32`. Nothing else is public, and nothing else is needed to construct it.
- **What has never been run.** The application has been *checked* for `wasm32`, not started.
  Its constructor and its first frame may reach a thread, a file, a device or a MIDI port.
  The spike's harness (`spike/roll-web`, never merged) ran the roll alone, 6.7 MB of wasm.

### What it builds

1. **`crates/evil-web`**: a workspace member, `publish = false`, `crate-type = ["cdylib"]`. Its
   whole source is one entry, compiled only for `wasm32`, that finds the canvas by id and starts
   `eframe::WebRunner` on `midi_evil::StudioApp::new` with default launch options, no instance
   and the `()` dialogs. On any other target the crate is empty **and has no dependencies**:
   `midi-evil`, `eframe`, `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` and `log` — and
   nothing else — are all under `[target.'cfg(target_arch = "wasm32")'.dependencies]`, and
   `eframe` is asked for with exactly the features that `midi-evil` asks for on that target, so
   the native build's dependency graph and features are what they were. The versions are the
   lock's, and `wasm-bindgen` is pinned exactly so the CLI check below means something.
   **No accessibility bridge, no live region, no focus boundary** — W-A2's. Two things are in,
   because without them a failure in a browser says nothing: a panic hook that writes the panic
   to the console, and the runner's own failure to start — no canvas, no WebGL — written there
   as an error too, never dropped.
2. **A page to load it, and only that.** `crates/evil-web/index.html`: a canvas, the module
   script, an empty inline icon so no browser asks the server for one, and a visible line
   saying this is a build check and not the page that ships. The
   page that ships — its name, its role, its regions, its help — is W-A2's and W6's.
3. **`[profile.wasm-release]`** in the root manifest, the engine's whole block: `inherits =
   "release"`, `opt-level = "s"`, `lto = true`, `strip = "debuginfo"`. Without the first line a
   custom profile inherits `dev`, and the size budget and the first run would be measured on a
   build with debug assertions.
4. **`scripts/check_wasm.sh`**: for `-p midi-evil -p evil-web` on `wasm32-unknown-unknown`,
   all `--locked`: `cargo check`; `cargo clippy --no-deps -- -D warnings` (the flag is
   clippy's, and keeps the shared crates out of the lint until W5a); and `cargo build -p
   evil-web`, so the linker runs and not only the type checker. One script, which the workflow
   below runs, so the local command and CI cannot drift.
5. **`scripts/build_wasm.sh`**: reads the `wasm-bindgen` version from `Cargo.lock`; installs
   that version of the CLI under `target/tools/` when it is absent **or is any other version**
   (`cargo install wasm-bindgen-cli --version <the lock's> --locked --force --root target/tools`
   — `--force`, or cargo refuses to replace the binary that is there), and never touches or
   calls the one on `PATH` (D15); builds `evil-web` with the `wasm-release` profile, `--locked`
   and an explicit `--target-dir` under the repository, so a `CARGO_TARGET_DIR` in the
   environment cannot send the wasm where the script does not look; binds it with `--target
   web`; stages the bundle and the page in a directory of its own and swaps it for
   `target/web/dist/` only after the build, the binding and the copy have all succeeded;
   prints the wasm's size, raw and gzipped; and fails over a size budget. **The budget is a
   number in the script**, set by this batch from its first good build — the raw size rounded
   up to the next mebibyte, plus one — and written into this section's outcome. It is a
   ceiling a later batch may raise, by changing the number and saying why in its own section;
   it is never a warning. **The script has no `--sync`.** The bundle reaches the site's tracked
   tree in W6 and not before.
6. **`.github/workflows/wasm-check.yml`**: on push and pull request, the stable toolchain with
   the `wasm32-unknown-unknown` target, running `scripts/check_wasm.sh`. It does not build the
   bundle and installs no CLI.
7. **The release audit knows the newcomer.** `tools/audit-public-packages.sh` keeps asserting
   over its eight publishable crates, and asserts separately that the workspace's only other
   member is `evil-web` and that it is *not* publishable. A ninth publishable crate, or a
   second unpublished one, still fails it.
8. **The first run.** `coordination/web/probes/web-smoke.sh` and its driver
   `web-smoke.mjs`, in the shape of `web-gate.sh` and `web-probe.mjs` and with their sources:
   the site's Playwright and Node 24, the same browser launch settings that drew W0's canvas
   in both browsers, a port the system picks, and a trap that stops the server however the
   script ends. A missing Playwright or Node is reported as the tooling's failure, not the
   application's. The driver opens the page in Chromium and in Firefox and passes only if,
   within a stated deadline, **a screenshot of the canvas element itself** — not the page,
   whose visible line would make any screenshot pass — holds more than one colour, and the
   console holds no error and no panic at all. There is no list of tolerated errors: the page
   asks for nothing but its own files. It clicks nothing and presses nothing. The screenshots
   are kept beside the report. It is shown to fail on a bundle whose wasm is replaced by an
   empty file, and on a page whose canvas id is wrong.

### What the first run may find

The application has never started in a browser, and W1's bar was compiling. If constructing it
or drawing its first frames panics or raises an error, that is this batch's to fix **only where
the fix is a seam of W1's kind**: a `cfg` in one of W1's named places, or a new module pair
like `sample_recorder`'s — one for each start-up site the first run proves needs it — each a
few lines that return "not available in the browser". The MIDI port list at start-up is the
likely first; its seam is "no ports, and no request made", and Web MIDI itself stays W3's. What is **not** this batch's: anything reached by a
gesture — Play, a pad, a key, a render, a file — which are W3's, W4's and W5's and are not
touched by a smoke that clicks nothing. A failure whose fix is neither is reported, with its
site, and the smoke row is reported INCOMPLETE rather than patched around. **An incomplete
smoke leaves W2 open**: the rest of the batch may land, W-A2 does not start, and the planner
brings the site to Jesse with what fixing it would take. Every seam added is listed in the
report, and native behaviour still does not change.

### Rules particular to this batch

- **Native behaviour does not change**, and the native suite, lints and gate say so.
- `cfg(target_arch = "wasm32")` appears in `crates/evil-web`, and in `crates/midi-evil` only
  where W1 put it or where *What the first run may find* allows a seam.
- No new dependency in any crate but `evil-web`, and none of `evil-web`'s on a native target:
  `cargo tree -p midi-evil` for the host is the same before and after.
- Nothing is installed on `PATH` or in `~/.cargo/bin`, and nothing is written to the site.
  Cargo's own registry cache under `~/.cargo` is cargo's business, and the CLI's first install
  needs the network.
- The scripts are plain `bash` with `set -euo pipefail`, each with a header saying what it is
  for and what it deliberately does not do.

### Gate

- The native suite, clippy and fmt; `tools/audit-public-packages.sh --allow-dirty` passes; the
  native gate, every sequence.
- `scripts/check_wasm.sh` is clean locally. **In CI it cannot be shown green by this batch**:
  the workflow runs only once it is pushed. The batch's evidence is the script's local run and
  the workflow file calling that script and nothing else; the first run on GitHub is checked by
  the planner after the push, and W-A2 does not start on a red one. The workflow uses the
  stable toolchain, as the engine's does and as every local gate does; a lint a new stable
  adds is fixed when it appears.
- `scripts/build_wasm.sh` produces `target/web/dist/` from a tree with no `target/tools/`
  (moved aside for the run and restored), and again with it present; and refuses, having
  changed nothing, when the CLI there reports another version than the lock's and the install
  is made to fail.
- `coordination/web/probes/web-smoke.sh` passes in Chromium and Firefox, and fails on its two
  negative controls.
- `git status --porcelain` in `../insiculous_web` is unchanged by everything above.

**Sitting:** none. Nothing here is operable yet; the first thing a person can hear is W-A2's.

**Leaves out:** the accessibility bridge and the shipped page (W-A2); audio and MIDI (W3); the
worker (W4); files (W5); syncing the bundle to the site (W6).

## W-A2 — the keyboard model in the browser (`#8`, second half) — DONE 2026-11-27

W0-web proved a mechanism on a scratch harness that held the roll alone. W-A2 builds it in
`crates/evil-web`, on the application W2 started, and holds it to W-A1's rows. Two batches:
**W-A2a**, the bridge and the browser's keys; **W-A2b**, the matrix, the surfaces the first page
does not show, and the help.

### What Jesse ruled, and what he was not asked twice

- **"Reads the notes once"** (2026-11-25, Orca, on W0's harness). The live region and the
  canvas's description do not double. W0's rule stands: one string a frame, notice first.
- **"Let's try and not have any hot keys overlap with browser defaults if we can (unless it
  makes sense like Ctrl+S to save the project we can override that one)"** (2026-11-25). This
  replaces the chord-by-chord observation D14 asked for: the web build does not find out which
  chords a browser happens to keep, it **stays off every chord a browser gives a meaning to**,
  and takes one only where the application's meaning is the browser's own.
- **Not answered, and so not assumed:** whether a repeated notice speaks again; whether a held
  key leaves speech queued; whether the value box is named on arrival and its value spoken once.
  Each is a line of W-A2a's sitting.

### The browser's keys (D14, by the ruling above)

| chord | natively | in the browser | why |
|---|---|---|---|
| Ctrl+S | Save | **Save** | the ruling's own example; the browser's meaning is "save this" |
| Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z | Undo, Redo | **Undo, Redo** | the browser's meaning is undo and redo; here they undo the document |
| Space | Play and stop | **Play and stop** | the browser scrolls the page; a canvas with `role="application"` has nothing to scroll, and Space is play in every player |
| Ctrl+N | New | **not bound**: Project › New | a browser never gives it up |
| Ctrl+O | Open | **not bound**: Project › Open… | the browser's own "open a file" |
| Alt+Left, Alt+Right | previous and next sound | **not bound**: the Previous and Next buttons beside the sound | Back and Forward |
| F6, Shift+F6 | next and previous region | **not bound**: Alt+Page Down and Alt+Page Up | F6 is the browser's own walk between its panes |
| arrows, Shift+arrows, Ctrl+Left and Right, Home, End, Enter, Delete, Escape, I | the roll's and the arranger's | **the same** | a browser gives none of them a meaning outside a text field, and the canvas is not one |
| B | start playback at the part | **says why not**, until W3 and W4 | nothing plays in a browser yet (W-A2a, item 5) |
| every other chord with Ctrl, Alt or the logo key, and every function key | nothing | **the browser's** | the canvas takes the chords in this table and no others |
| Tab, Shift+Tab | next and previous control | **the same, released at the first and last stop** | W0's focus boundary |

The three kept chords and the new one are **proposals until Jesse has pressed them** in a real
Firefox and a real Chromium at W-A2a's sitting: a synthetic key shows nothing about what a
browser does first. Alt+Page Down and Up fit the one-modifier chords the tables already hold;
the sitting tries Ctrl+Alt+Period and Comma beside them — the pair a well-known web editor uses,
which a keyboard with AltGr may turn into a character — and the table changes in one place. Space
is in the table as the transport's key, but nothing plays in a browser before W3 and W4: until
then it says so (W-A2a, item 5). A chord marked *not bound* is compiled out of the web build's tables —
not ignored at run time — and a row asserts that pressing it changes nothing and announces
nothing. The command it reached natively is reached by the path named, by keyboard.

**This table is for a keyboard whose command key is Control**, in Chromium and Firefox on Linux,
which is what D12 gates. On a Mac the same commands would move to Command, and Command+Left,
Command+Right and Command+Y are the browser's there. The web build therefore uses Control on
every system and says, where it says which browsers it was tested in, that a Mac's own
shortcuts are not designed yet; that is an issue in the milestone, filed with this section.

### W-A2a — the bridge, and the browser's keys — DONE 2026-11-25

**Sitting, 2026-11-25:** Jesse, with Orca, on the gate's bundle: "k looks good keep at it".
Unitemised: recorded as the batch passing as a whole. No row of the key table, neither region
chord and no line about editing a name in its middle was answered one by one, so each stays as
built and none is recorded as heard a particular way: Alt+Page Down and Up remain the region
chord, and review of a field's text by character was not asked for and is not designed.

Built by Claude Opus 5.5 in two passes; 537 tests, the native gate at 455 expectations, and the
web-app gate holding 19 rows in Chromium and 19 in Firefox. What differed from the section below:

- **The frame's own tree is readable.** With `enable_accesskit()` and no adapter, the focused
  widget's node is there at the end of the application's frame with its label, description,
  value, role and states. The bridge has one source, the node a desktop screen reader is given;
  no fallback was built.
- **The public value is `StudioApp::spoken_frame`**, a `SpokenFrame`: the focused control, the
  current text, what happened, the announcement, the ends of Tab's walk, whether anything is
  unsaved, and the page's key table. `evil-web` holds no string a person hears.
- **Exact modifiers are the browser's rule only.** The section's sentence would have changed the
  desktop — Ctrl+Shift+S no longer saving — and "native behaviour does not change" outranks it.
  The desktop's dispatch is what it was, key for key.
- **The build is decided in one place**, `Platform::of_this_build`; everything else reads the
  value. A control that is unavailable goes through `Refusable`: on the desktop it is the
  disabled control it was, in the browser it keeps its stop, says why, and refuses.
- **After Enter or Escape in a field, nothing holds egui's focus**; the field stays what is
  current, and Tab moves on from it by the library's own order. Handing egui the focus back
  reopened the edit.
- **B does not reach the renderer** — it only sets the range — but says the reason as Space
  does, since what it promises is playback.
- **Inside a text field the visitor's own editing chords pass**, a Mac's among them, by egui's
  account of the system. Outside one the table is a Control keyboard's (`#24`).
- **Left for a later batch, each named where it is owed:** the "Track actions" menu, unavailable
  only during a take, is not routed through `Refusable` and must be when recording reaches the
  browser (W3, W4); the SYNTH and KEYBOARD tabs and the WAV recovery window's controls are
  W-A2b's to take off the page.
- **The wasm is 5.99 MiB raw, 2.43 MiB gzipped**, under the 7 MiB budget.


**What it finds out first, before anything is built on it.** Three things, each a throwaway
check in both browsers whose result is written into the report:

- **Is the frame's own tree readable?** W-A1 put every name where AccessKit reads it. eframe's
  web runner throws the finished tree away, but `Context::accesskit_node_builder`
  (`egui-0.35.0/src/context.rs:3582`) reaches a node while the frame is being built. With
  `enable_accesskit()` called and no adapter attached, does the focused widget's node exist at
  the end of the application's frame, with its label, description, value, role and states? If
  yes, the bridge has one source, the node a desktop screen reader is given. **If no, the
  batch stops and reports**: the fallback — W0's `OutputEvent::FocusGained` paired with the
  focused id, plus the library's own names — is a second representation to keep true, and
  whether to build it is the planner's and Jesse's call.
- **What eframe does to each key before the application sees it.** For every chord in the key
  table and for Home, End, Enter, Delete, Ctrl+P and the function keys: whether eframe prevents
  the browser's default. W0 found it prevents the arrows, Ctrl+O, Ctrl+S, Space and every
  letter and does not prevent Home, End, Enter, Delete, F6, Ctrl+N, W, T, Z, Y.
- **What the dispatch does with an extra modifier.** Whether Ctrl+Shift+S, Ctrl+Alt+O and their
  like reach a command today.

**Builds:**

1. **What the library says, as one value, made for a surface that speaks by writing text.**
   `StudioApp` gains one public method, called by a front end after each frame. It returns the
   focused control — name, description, value, **its role and its states in words** (a check
   box that is checked, a menu that is open, a button that is unavailable and why), and
   whether it is a text entry — and what happened this frame: an outcome, a change of focus, a
   change of the focused control's name or state, a committed text entry. The desktop needs
   none of the words for role and state, because a screen reader composes them; they are
   written once, in `speech`, so the browser's wording has one home. **It is not read after the
   fact from the tree and the `Announcer`**: natively an outcome goes out in one frame and the
   name it changed is held to the next, and `end_frame` has consumed the outcome by then. The
   `Announcer` produces this value itself at the end of the frame, pairing an outcome with the
   name *as it now is*, so that one action is one value: Delete on a note followed by another
   yields "Removed." and the next note together, once, and nothing the frame after. This is
   the only item made public; `speech`, `announcer`, `regions` and `key_help` stay private, and
   `evil-web` contains no string a person hears. The value also says what a page must know and
   cannot ask egui: whether focus is on the first or the last stop of this frame, and whether
   the document has unsaved changes. `evil-web` gets it through a thin `eframe::App` of its
   own that runs the application's frame and then asks.
2. **The bridge**, in `crates/evil-web`, as W0-web's result describes it: the canvas
   `role="application"`, named, `aria-describedby` a visible *current* element that always
   holds the focused control in full; a polite, atomic, visually hidden live region written at
   most once a frame, while the page's focus is on the canvas **or on the adopted text
   entry**. What is written, and when, is stated and each line is a row: **focus moves to
   another control** — its name, role and state, once; **an outcome** — the outcome, then the
   name if it changed, as one string; **the focused control's value or state changes** with no
   outcome — the new value or state, once; **focus returns to the canvas from the page** —
   nothing, the screen reader reads the description itself. A string identical to the last one
   written is made to differ by a trailing no-break space that **alternates**, so a third and
   a fourth identical notice still speak. **While a key is held, nothing is written** — for
   the editors, because the library's names already rest, and for a standard control such as
   the tempo box under a held Up, because the value's changes rest by the same rule — and one
   write follows at release; the current element stays true throughout. A value that changes
   on a timer is never a change to announce: the transport's reading is the cached,
   read-on-demand name the desktop rests on, and the row that proves it under playback is
   entered in W-A2b's matrix as W3's and W4's. The bridge adds no timer of its own.
   Text reaches the page only as `textContent` and attribute values.
3. **The browser gets back what is not bound.** Dropping a chord from the application's tables
   does not return it to the browser: eframe cancels it when it sees it. A capture-phase
   listener — the one that releases Tab at the boundary — **stops a chord that is not in the
   web table from reaching eframe at all**, without cancelling it, so the browser does what it
   does: Ctrl+O its own dialog, Alt+Left back, Ctrl+P print. The rule is the table's last row,
   not a list: a chord with Ctrl, Alt or the logo key, or a function key, that the table does
   not hold is the browser's. Plain keys and Shift are untouched, so typing is. **While a
   text entry is being edited the table is a different one**: egui does its own select-all,
   copy, cut, paste, undo, word movement, word deletion and selection, and the input eframe
   keeps does not hold the field's text for a browser to do them on — so those chords go to
   egui, and only the browser's own navigation still goes to the browser. And because
   Alt+Left now leaves the page, **the page asks before it is left with unsaved work**: the
   `beforeunload` answer is "unsaved" when the library's value says so **or when any input
   has reached the canvas or the text entry since the last finished frame** — a key, a paste,
   a composed character — so an edit and Alt+Left pressed together cannot slip through the
   frame between them. The announced, keyboard-operable prompt for
   New and Open stays W5b's. The same listener cancels the browser's
   default for the kept chords eframe lets through — Home, End, Enter, Delete, the region
   chord — and the page does not scroll: a row reads the scroll position after each.
4. **The binding set has one home and takes a platform.** The table above becomes data in
   `key_help`: the region chord and every chord named in an announcement, a description or the
   start-up notice, **a menu item's label, a button's hover text and the Keyboard notes
   window** come from it — F6, Ctrl+N, Ctrl+O and Alt+Left are written in all of those today —
   and the dispatch matches a chord's modifiers exactly. The desktop's tables and strings do
   not change. Rows: nothing the web build can speak or show, the opened Project menu and the
   Keyboard notes window among it, contains "F6", "Alt+Left", "Alt+Right", "Ctrl+N", "Ctrl+O"
   or their spelled-out forms; a chord with an extra modifier does nothing.
5. **What does not work yet says so, and is not a chord.** Nothing can play in a browser until
   W3 and W4: Play, Record and the arranger's start-here reach a renderer that spawns a
   thread. In the web build until then each is unavailable with its reason ("Playback is not
   in the browser build yet."), Space and B announce that reason and start nothing, and no row
   plays. **And an unavailable control can be found.** egui takes a disabled widget out of the
   Tab order, and a desktop screen reader finds it anyway by reviewing the window; a canvas
   offers no review. So in the web build a control that is unavailable — Undo with nothing to
   undo, Play — keeps its place in the Tab order, is described as unavailable with its reason,
   looks as a disabled one does, and when activated does nothing but say the reason again.
   The desktop is unchanged. W3's and W4's sections remove the reason; the
   transport's playing rows are theirs. Save, with no file behind it, says "Not saved." and
   never "Saved"; Open says "Nothing opened."; New makes a new project when nothing is
   unsaved and refuses, saying why, when something is — until W5b gives the browser a prompt.
6. **Text entry.** eframe's hidden input is adopted as W0 adopted it: in the tree only while
   egui is editing text, labelled with the control's name, described by the current element,
   placed after the canvas, refusing focus otherwise; when an edit ends, focus is back on the
   canvas and the next key reaches the application. The current element holds the field's
   whole text as it is edited, and a committed value is announced once. **What it does not
   give**, and the section says so rather than hiding it: eframe's input holds only the
   characters being composed, so a screen reader's own review of the text — by character, by
   word, the caret, a selection — is not available. Whether hearing the whole value at rest is
   enough to edit a name in its middle is a line of the sitting, and if it is not, the remedy
   is W-A2b's first item.
7. **The focus boundary**, W0's mechanism with what its harness did not have: the set of
   stops changes when a menu, a window or the side panel opens, so the first and last stop are
   read from the frame, not remembered; a standing, named "leave the editor" control stays as
   the backstop; and focus arriving from the page enters at the first or the last stop by the
   direction it came from, through a sentinel before and after the canvas.
8. **The page.** `crates/evil-web/index.html` becomes the application's page for the gate — a
   heading, one sentence, a sentinel, the canvas, the current element, the live region, a
   sentinel, and after them the standing control, a button named "Leave the editor" that moves
   focus to the heading —
   laid out so the document never scrolls. Its shipped wording, the keyboard notes and the
   list of what is desktop-only are W-A2b's and W6's.

**Gate.** W0's `web-gate.sh` and `web-probe.mjs` are written against the scratch harness —
its element ids, its fixtures, its query string — and stay as that half's evidence. This batch
adds `coordination/web/probes/web-app-gate.sh` and its driver for the real bundle, starting
from the application's own default project, in headless Chromium and Firefox. A row passes
only when the element that has the page's focus is named and described by what the row
expects; **in Chromium the browser's real tree is read over CDP and asserted** — role, name,
description — not printed; in Firefox the check is Playwright's own computation and the row
says so, the real tree being the sitting's. Negative controls, each failing for its stated
reason, in each browser: a bundle whose live region is never written; one whose current
element is stale; one that binds a *not bound* chord.

- the nine steps as far as the browser build goes: a new project from the menu on a clean
  start, add an instrument, discover the instruments with the buttons, add a note, select it,
  change it, delete it, read the transport stopped; Save says "Not saved.";
- every line of item 2, Delete and Undo among them across every frame they take: one write,
  the final selection in it;
- three identical notices, three writes;
- focus into the canvas and out again in both directions, through every stop, with a menu
  open, a window open and the side panel's fields, and the "leave the editor" control;
- a held key: one write at rest, matching where the cursor is;
- leaving and returning: nothing written, the current thing readable;
- the tempo box and a name field: named where focus is, typed into, the current element true
  after each key and after an arrow, committed, heard once, the next key reaching the canvas;
- a check box, a menu and a disabled button: role and state in the current element, and a
  change of state written once;
- item 3's, 4's and 5's rows, as far as a synthetic key can show them: that a chord outside
  the table never reaches eframe and is not cancelled, that the application did nothing and
  wrote nothing, that the `beforeunload` answer is "unsaved" after an edit and in the same
  task as an edit. **What the browser then does with the chord is the sitting's**, and each
  such row says so;
- in a name field: select all and type replaces the name; a word is deleted; a selection is
  extended; typed text and a paste each make the page answer "unsaved" in the same task;
- a held Up on the tempo box: nothing written while it is held, one write at release;
- Undo on a clean start and Play: reached by Tab, described as unavailable with the reason,
  activated, the reason written, nothing changed;
- the region chord reaches the toolbar, the side panel and the editor, and back;
- a track named with `<`, `&` and a direction override, typed into its field, reaches the page
  as text. A newline cannot be typed into that field; its row is W5b's, when a crafted
  project can be opened.

And the native suite, clippy, fmt, the native gate — item 1 and item 4 touch the `Announcer`
and the dispatch — `check_wasm.sh`, `build_wasm.sh` within its budget, `web-smoke.sh`.

**Sitting** (Orca; Firefox, then Chromium; on the gate's own bundle), written down row by row,
a row not mentioned recorded as not answered:

- the nine steps as far as they go;
- a repeated notice, three times; a held key; moving between controls with Tab;
- the tempo box; a track's name edited **in its middle**;
- **every row of the key table pressed for real**: the kept chords do the application's thing
  and not the browser's; each *not bound* chord does the browser's thing; the page asks before
  Alt+Left leaves unsaved work;
- **the region chord**, both candidates: Alt+Page Down and Page Up, which the batch builds,
  and Ctrl+Alt+Period and Comma. The table takes whichever Jesse finds a browser and the
  desktop leave alone.

**Leaves out:** playing and recording (W3, W4); files (W5b); the MIDI input and its refresh
button, which are on a shown surface and silent — W-A2b takes them off the browser's toolbar
until W3 gives them states.

### W-A2b — the matrix, what is not shown, and the help — DONE 2026-11-27

**Sitting, 2026-11-27:** Jesse, with Orca, on the visitor's bundle built from `87fa107`: "sitting
passes". Unitemised: recorded as the batch passing as a whole, and W-A2 closed with it. Neither
question of the sitting's third part was answered by itself, so a change of state goes on being
said as the state alone — "…unavailable", "…pressed" — without the control's name.

Landed as `9c075c1`, then merged with the render lifecycle (`8e7caa5`) as `87fa107`: two files
conflicted, the resolution was reviewed, and the merged tree passed every gate below with 616
tests.

Built by Claude Opus 5.5 in three passes; 548 tests, the native gate at 455 expectations, and the
web-app gate holding 57 rows in five groups in each browser, with eight negative controls. What
differed from the section below:

- **What the browser leaves out is fifteen lines** of `platform.rs`: the four views, the
  Automation panel, the MIDI input, the six kinds of command that need a file or a renderer,
  the export row, and moving a window and resizing the side panel. The executor's own search of
  every way into a hidden view or a removed command found none the list did not name. The view
  is private to `app/view_selection.rs` and chosen through `select_view`.
- **One action is one write — W-A2a's rule, finished here.** The matrix found actions that
  wrote twice, a frame apart, when the control just used changed or went away. In the browser a
  frame that produces an outcome is now run again at once, so the outcome goes out with what
  the action left: "Merged into the unique timeline. Nothing in the editor has focus; …". The
  desktop still runs one pass. Two keys in one frame each act once through it — R-2's and
  R-18's browser tests.
- **The page's order** is the heading, the notes' button, the notes, a sentinel, the canvas,
  the current element, the live region, a sentinel, "Leave the editor". The canvas's floor is
  240 pixels or two fifths of the window's height, whichever is less, so a short window keeps
  its controls.
- **Three things the gate's bundle must carry or the gate refuses to start**: the marker only
  the feature compiles in, a digest of the twelve tracked fixtures, and a build identity made
  from the sources — which the visitor's bundle carries too, so the two are the same
  application.
- **The matrix is two groups**, the rows and the surfaces, to keep each inside five minutes a
  browser; the whole gate takes about twenty-six. Firefox's matrix group runs at four and a
  half of its five minutes.
- **The inventory** is `w-a2-inventory.md`: 45 announced in the browser, 3 unavailable until W3
  and W4, 41 absent, 33 standard, none failing.
- **The wasm is 5.99 MiB raw, 2.43 MiB gzipped**; the gate's is 6.05.
- **Heard and not changed:** after an Undo that empties the history the write ends
  "…unavailable", and after a sound is chosen "…pressed" — the state alone, without the
  control's name. It is the existing rule for a change of state, and a line of the sitting.


W-A2a made the browser build speak. W-A2b makes the page show only what has passed (D12), holds
it to W-A1's rows, and puts the keyboard notes where a visitor can read them before entering a
canvas that takes every key.

**Builds:**

1. **What the browser build shows is decided in one place, and enforced where a view changes.**
   `platform.rs` gains the list of what a platform shows. Two things read it and nothing else
   knows: each control, to decide whether it is drawn; and **the one place a view is selected**,
   which answers a request for a view the platform does not show by staying where it is — so
   a route nobody listed cannot arrive at a hidden view, and a project that asks for one opens
   on Song. That place does not exist yet: the view is assigned directly in more than a dozen
   sites. The batch makes the field private to one module with one `select_view`, sends every
   assignment through it, and a test asks for each view by every command that can ask. In the browser, absent — not drawn, not in the Tab order, not in the frame's tree:
   - the **Mixer, Synth, Keyboard and Pads** views, and every way in: their four tabs, +KIT,
     the button that reads Open Evil Synth or Open Pads, Samples, Modulation, Mixer & FX;
   - the Notes view's **Automation** panel, its toggle, and the side panel's Automation button;
   - the **MIDI input** and its refresh button, silent until W3 gives them states;
   - **what needs a file or a renderer**, wherever it is drawn: Import MIDI in the Project menu
     *and in the About window*, Recover rhythm from WAV, Relink MIDI, Export MIDI, the WAV
     renders, stems and parts, the Export toggle, and both Load files… buttons.
   **A drum track stays.** + Track › Drum track adds one and, in the browser, does not go to
   Pads: its notes are edited in the roll like any track's and its kit is chosen from the sound
   list. New, Open, Save, Save as, Keyboard notes and About stay, saying what W-A2a made them
   say until W5b. The executor does not trust this list: it greps every site that sets the view
   and every call of a removed command, and reports any way in the list does not name. Each
   thing comes back in the batch that makes it pass, by a line removed from the list. The
   desktop shows what it showed.
2. **Nothing on the page is pointer-only.** The inventory's two failing kinds that are not
   W3's — moving a window by its title bar, and dragging the side panel's width — are not
   offered in the browser: its windows do not move and its side panel does not resize, and
   neither the title bar nor the handle is a stop there at all — a stop that only ever dragged
   is removed, not left named. On the desktop both stay, and stay `#23`'s.
3. **The page says what it is, in its own HTML.** The keyboard notes, what is desktop-only
   for now, and the browsers it was tested in are **static text in `index.html`**, there the
   moment the page loads and there if the wasm never starts — not written by the application
   after a six-mebibyte download. They would otherwise be a second copy of the library's
   tables, so **a test holds the pair**: it generates the notes and the desktop-only list from
   the browser's tables and item 1's list and fails when `index.html` differs, and the page's
   header says so. A command the browser build refuses is in the notes with its reason —
   "Space: play and stop — not in the browser build yet" — never as if it worked, **and the
   Keyboard notes window inside the application is written by the same function**, so the two
   say the same of Space. The tested-
   browsers sentence, with the line that a Mac's shortcuts are not designed (`#24`), is the
   page's own; "`evil-web` writes no string" means no string at run time. **The notes do not
   cost the editor its room**: they sit in a disclosure above the canvas, closed when the page
   loads, whose opened body is a named, focusable region that scrolls within itself by the
   keyboard; the document still never scrolls. The page's order is stated and the entry rows
   are rewritten to it: the heading, the disclosure's button, its body when open, the
   sentinel, the canvas, the current element, the live region, the sentinel, "Leave the
   editor". The disclosure's button is a stop a visitor meets before the editor, and the rows
   say so.
4. **Fixtures for the gate, in a bundle a visitor never gets.** W-A1's rows start from projects
   the native gate writes into a temporary directory from its own script and opens by launch
   argument. A browser cannot open a file before W5b, and the application cannot be handed a
   project except by path. So:
   - the fixtures become **tracked files**, `coordination/web/probes/fixtures/*.evil` — one
     set, both gates. They are never opened in place: the native gate saves into some of them,
     so it copies the set into its temporary directory at each run, as it writes it there
     today. The set gains what it lacks: the native gate's default start is the Bach
     invention imported from MIDI, which a browser cannot import, so its project is tracked
     as a `.evil` with the same notes in the same order, the rows that only need its notes
     start from that on both gates, and one native row goes on importing the MIDI itself;
   - `midi-evil` and `evil-web` gain a cargo feature, off by default. Under it the library has
     a constructor that takes a project as text, and the entry reads `?fixture=<name>` and
     starts from one of those files, compiled in; and, for the absence rows' negative control
     only, `?shows=desktop` makes the page show what the desktop shows;
   - `build_wasm.sh` builds the visitor's bundle without the feature, exactly as now; with
     `--gate` it builds the other into `target/web/dist-gate/`, **with its own cargo target
     directory**, so a gate build can never leave its wasm where a visitor's build picks it up,
     under the same lock and swap. `check_wasm.sh` checks both feature sets;
   - `web-app-gate.sh` is given both bundles and each row says which it runs on: the fixture
     rows on the gate's; the absence, page-text and first-run rows on the visitor's. A row
     shows the visitor's bundle ignores `?fixture=` and `?shows=`, and that a marker string
     the feature alone compiles in is in the gate's wasm and not in the visitor's — the
     fixtures' own names are ordinary words and prove nothing; W6 asserts both again on what
     is shipped. The gate refuses to start without a gate bundle that carries the marker, so a
     stale or missing one is a refusal and not a row's failure; `check_wasm.sh`'s check of the
     feature has a target directory of its own too.
5. **The matrix below, as rows of `web-app-gate.sh`**, each starting from the fixture its
   native row starts from — taken from the native gate's own table, not chosen again — and
   each first asserting the project it loaded by name and track count, so a row cannot pass
   on the default project. The rows run in named groups — W-A2a's, the matrix, absence, the
   page — each with a deadline of five minutes a browser, where the whole gate takes about
   four minutes today for nineteen rows in both; a group that runs out says its name. R-2 and R-18 are the crate's tests: the batch adds their browser forms — the
   region chord for R-2, the browser's dispatch for R-18 — and names them.
6. **The inventory, re-read for the browser**: `coordination/web/w-a2-inventory.md`, every line
   of `w-a1-inventory.md` ending one of five ways — announced in the browser, with its row;
   **unavailable until a named batch**, with the row that hears its reason; absent from the
   browser, with its absence row and the batch that brings it back; a named standard control;
   or failing, listed. **W-A2 does not close with a failing line** that is not W3's.

**The matrix.** *CDP* is Chromium's real tree, asserted; *computed* is Playwright's own
computation in Firefox, whose real tree is the sitting's. A row whose native form turns on F6
is restated on the browser's region chord.

| row | natively | in the browser | where |
|---|---|---|---|
| W0 edges: top pitch, tick 0, empty pattern, full shared pattern | native gate | the same keys, the same words | CDP, computed |
| R-1 the cursor never moves to an identical twin | native gate | the same, from the fixture | CDP, computed |
| R-2 a region move and an arrow in the next frame | crate's tests | the crate's tests, on the browser's tables | tests |
| R-3 regions from the roll and back | native gate | Alt+Page Down and Up | CDP, computed (W-A2a's `regions`) |
| R-4 Delete beside an automation lane | native gate | **not applicable**: the Automation panel is absent; its absence is the row | CDP, computed |
| R-6 add in an empty pattern, undo: selection reset | native gate | the same | CDP, computed |
| R-7 a held Right, a held Up at the top | native gate | one write at rest (W-A2a's `held-key`), and Up held at the top | CDP, computed |
| R-8 delete the last track; the editor stop is add-a-track | native gate | the same, on the region chord | CDP, computed |
| R-9 add after the first of two twins, undo | native gate | the same | CDP, computed |
| R-10 the roll's description names every chord | native gate | every chord of the **browser's** table and none the browser keeps | CDP, computed |
| R-11 the transport read while playing | native gate | **W3's and W4's**: nothing plays yet; read stopped is W-A2a's | — |
| R-12 nothing focusable is unnamed | native gate | every stop Tab reaches on the Song and Notes views, a menu and each window: named, with a role | CDP, computed |
| R-13 a screen reader started after the application | native gate | **not applicable**: the page is always in the tree; a row reads it with no input since load | CDP, computed |
| R-14 every one-note edit from its button | native gate | the same | CDP, computed |
| R-15 duplicate a track, undo | native gate | the same | CDP, computed |
| R-16 a nudge past a neighbour, undo | native gate | the same | CDP, computed |
| R-17 Clean overlaps re-sorts; the cursor follows | native gate | the same | CDP, computed |
| R-18 two keys in one frame, three orders | crate's tests | the crate's tests | tests |
| R-19 Escape clears, then leaves | native gate | the same | CDP, computed |
| R-20 undo during a take | native gate | **W3's and W4's**: nothing records yet | — |
| R-21 the readout after a tempo change and a new project | native gate | change the tempo and read; on a fresh page, New and read. New with unsaved work refuses and says why — its own row, until W5b | CDP, computed |
| R-22 an unavailable control says why | native gate | W-A2a's `unavailable`, and each control item 1 leaves on the page that can be unavailable | CDP, computed |
| the instrument list | native gate | the Previous and Next buttons | CDP, computed |
| the Song sequences, the track panel, the roll's toolbar, the windows | native gate | the same, without what item 1 removes | CDP, computed |

**Gate** — in this order: the native suite, clippy, fmt and the native gate, since item 1
touches every surface's drawing and item 4 its fixtures; `check_wasm.sh`; `build_wasm.sh`
within its budget, then `build_wasm.sh --gate`; `web-smoke.sh`; and `web-app-gate.sh`, whose
rows are W-A2a's and:

- first, the row W-A2a owes: a click on a control from outside the editor, press and release
  with no frame between, is announced;
- the matrix;
- **absence**, stated as what can be observed: the page has one node for the whole editor, so
  there is no tree to search for a control's name. After a row has first found the controls
  that must be there, a walk by Tab through every stop of the Song and Notes views, each menu
  and each window never puts a removed thing in the current element; no menu path, chord or
  button changes the view to a hidden one; adding a drum track leaves the view where it was.
  The negative control is the gate's bundle with `?shows=desktop`, on which the walk must find
  the Mixer tab and fail;
- no window moves and the side panel does not resize: no stop named for either;
- the page's text: the notes hold every chord of the browser's table, none the browser keeps,
  and the reason beside each command that is refused; the desktop-only list names each thing
  item 1 removes; both are in the page before the wasm has started — a row delays the wasm
  and reads them — and come before the canvas in its order;
- the layout: at 1366 by 768, and at twice the text size, with the notes closed and with them
  open, the canvas keeps a stated least height and "Leave the editor" is reached;
- the visitor's bundle ignores `?fixture=` and `?shows=`.

**Sitting** (Orca; Firefox, then Chromium; on the visitor's bundle): read the page from its top
— the notes, what is desktop-only — then into the editor, across the toolbar and the side
panel, hearing nothing that leads somewhere that is not there. This sitting closes W-A2.

**Leaves out:** review of a field's text by character — W-A2a's sitting did not ask for it, and
it is `#25`, not a batch; everything a row above gives to W3, W4 or W5b; a
probe of Firefox's own accessibility tree, which stays the sitting's.

## W3 — audio and MIDI in the browser (`#5`) — settled 2026-11-27

W-A2 left the browser build silent: Play, B and Record keep their stops and say "not in the
browser build yet". W3 makes Play heard and a take recordable from a MIDI input. Two batches, one
out at a time: **W3a**, Play; **W3b**, the MIDI input and Record. A spike measured what the
section stands on before it was settled: `coordination/web/w3-spike-result.md`, cited below by
its numbered findings.

### What this section decides

- **The library goes on naming no browser API.** The browser's sound and the browser's MIDI
  are each a seam the application is handed where it is made, as it is handed its `Dialogs`,
  and `evil-web` implements both. The desktop's side of each seam is today's code, moved and
  not changed.
- **In the browser a preview is one buffer on the page's own audio context — not rodio.** The
  record said to turn on rodio's `wasm-bindgen` feature. The spike did, and it plays in time
  (finding 3), but cpal feeds its stream from the page's own thread and the sound drops out
  whenever the page stops for longer than about 85 ms (finding 5); a sink kept between Plays
  plays the song at double speed after any such stop (finding 4); and nothing above rodio can
  see whether the context ever started (finding 2). A preview is already a whole rendered
  stereo buffer, which is the thing a browser plays best: handed to the context once, it is
  played on the browser's audio thread, in time whatever the page is doing (finding 6), and
  its position is the context's own clock. So rodio's feature stays off, and the played key and
  the pad — live sources, which a buffer is not — choose their own way in the batch that
  shows them. **Jesse's to overturn:** the other choice is the record's, a smaller batch by one
  seam and one module, with the dropouts.
- **Until W4, a render in the browser runs in place, and only a short one.** `RenderJob` does
  its work on a thread (`render_job.rs`, the one `std::thread::spawn` in `spawn`), and a page
  has none; the worker that replaces it is W4, which stands on W5a's byte interfaces, and D2's
  order is not reopened. So where there is no thread the same closure runs before `spawn`
  returns, its result waits in the same channel, and the application takes it on the next frame
  as it does today. The page does not answer while it runs: 0.6 s in Chromium and 3.3 s in
  Firefox for the Bach invention (finding 7). **What is rendered is bounded, by work and not
  by length alone.** *(Ruled by Jesse, 2026-11-27: a limit of five minutes, lowered until the
  page stops for no more than five seconds in either browser.)* A render's cost grows with how
  much sound is made, so the limit is on **track-minutes**: the tracks that sound, times the
  time from the song's start to the end of what is played — the renderer works from the start
  and hands back the range — at 48,000 samples a second, more in proportion at a higher rate.
  The Bach invention is 3.4 track-minutes and stops Firefox for 3.3 s; repeated to five
  minutes it is 10 and stops it for 6.1 s. The limit is **five track-minutes**. In the
  browser a preview over it is refused aloud — "Too much to play in the browser build yet." —
  and nothing is rendered. **It is a bound on a placeholder, not a promise**: notes sounding
  together cost more than notes in a line, and no count of them is taken. The batch adds a
  fixture of four tracks of chords at the limit and reports how long each browser stops for
  it; over five seconds, the limit comes down until it is not, and this section is corrected.
  W4 removes the limit with the freeze; D10 stands, and W6's inventory asserts the render
  runs in the worker.
- **One device site is gated now: Play.** *(Ruled by Jesse, 2026-11-27.)* Of the three places
  a device is opened (`app.rs` for the preview, `keyboard_page.rs` for a played key, `beats.rs`
  for a pad) only the first is on a surface the browser shows. D12 holds the Keyboard and Pads
  views back from the first release and D2's order has no batch that shows them, so the
  record's "press a pad, press a piano key" leaves W3's gate; whichever batch one day shows
  either view owns its device site.
- **The record said midir; the browser's MIDI is written on `web-sys` instead.** midir's web
  backend (`midir-0.11.1/src/backend/webmidi`) asks for access with system-exclusive rights, which
  is the wider permission, and reports nothing a page can turn into "unsupported", "refused" or
  "granted". midir stays the desktop's.
- **Asking is its own act.** Access to MIDI is asked for by the refresh button and by nothing
  else — not by making the application, not by reading the list.

### W3a — Play is heard

**Builds:**

1. **The seam.** The application is handed what plays a preview: given what a render
   returned — the sound and its channel meters — and whether it repeats, it returns the
   thing being played, which can be stopped and says whether it has finished, where it is,
   what its meters read, and whether its device is running; or the reason it could not
   start. `PreviewPlayer` is the desktop's side, unchanged, meters and all; its device is
   always running. The browser's side reads no meters: the Mixer is not shown there. The
   sound handed over is the range that was asked for, so where it starts and where it
   repeats are the application's as they are now.
2. **The browser's side, in `evil-web`.** One `AudioContext` for the page's life, made in the
   first `keydown` or `pointerdown` that reaches the listeners `keyboard.rs` already installs
   — tried again at each until one is made — and `resume()` called in every one after, its
   refusal caught and not logged: inside the event's own handler, where no browser's rule can
   call it late. With nothing pressed the page has made no context. A preview is one
   `AudioBuffer` at the sound's own sample rate through one `AudioBufferSourceNode`, with
   `loop` for a repeat; its position is the context's clock less the clock when it started,
   folded by the length when it repeats; **it has finished when that clock passes its
   length** — no listener is set on the node, so none can outlive it. Stop stops the node. A
   sound at a rate the browser will not make a buffer for is a device that could not start. Firefox makes a context suspended and starts it a moment later (finding 1),
   so the state read when the context is made decides nothing.
3. **The render, in place** — the decision above, as one function in `render_job.rs` that
   either spawns the worker or runs it. The in-place form is compiled where there is no
   thread and for tests, and a test holds it to the threaded form's result for one snapshot.
   `detach_if_stalled`, which spawns, is not reachable when nothing can stall; the batch shows
   that by a test or removes the path for the target.
4. **Play, Stop and B stop being refused.** `Platform::playback_refused` goes, and
   `Unavailable::NoPlaybackInBrowser` with it. Record keeps its stop and a reason of its own —
   "Recording is not in the browser build yet." — until W3b. The limit above is one function
   beside the platform's other answers, asked before a render is accepted. It reads the end
   of what is played from the project's timing and counts the tracks that are not muted and
   have a part before that end; it does not expand the song into a performance, which is
   most of what a render costs, and the batch reports how long it takes on the largest
   fixture.
5. **A device that does not start says so**, and saying is reporting — what the status line,
   the announcer and the bridge all read; today a failure to open is only shown:
   - the device cannot be opened: "Could not start audio preview: …", reported, on the desktop
     as in the browser. Nothing is played;
   - a preview whose device has not been running for three seconds — at its start or in its
     middle — is stopped, and "The browser is not playing the sound; press Play again." is
     reported. The context is kept: the press that follows resumes it in its handler. The
     desktop's device is always running, so nothing there changes. The application already
     wakes ten times a second, which is what counts the three seconds.
6. **A key pressed while the page was rendering.** It arrives in the frame that takes the
   finished render, after it (finding 8): the preview is started and stopped within the
   frame. The spike heard nothing then, but through rodio and a context made that instant; a
   node started and stopped in one turn on a context already running is not measured. The
   order is left as it is and the row decides: if the tap hears anything, the batch handles
   the frame's keys before a finished render is played, in the browser, and says so.
7. **Sources a page cannot read.** A track whose sound is a patch or a kit named by a path
   cannot be resolved before W5a. The render's existing error for it is reported, and the
   transport returns to stopped. The built-in sounds, all the browser's sound list offers
   until W5b, resolve without a file (`evil-render`'s `load_patch` reads one only for a name
   ending `.json`).
8. **What moves with the refusals** — listed so that no gate is left asserting the opposite:
   the browser-platform tests that hear "Playback is not in the browser build yet."
   (`front_end_browser_tests.rs`); the page-notes test's suffixes (`page_notes_tests.rs`) and
   `index.html` regenerated from it; in `web-app-gate.sh`, the `unavailable` row for Play,
   Space and B (`web-app-rows.mjs`), the page group's `REFUSED` and `BROWSER_CHORDS` lists
   (`web-app-page-rows.mjs`), and the matrix's `r22-unavailable-controls` and
   `song-by-keyboard`, which hear the refusal from Play and from B
   (`web-app-matrix-rows.mjs`); and the inventory's three lines. Each is changed to what is
   now true, and Record's new reason takes the place of the old wherever Record is heard.
   The executor does not trust this list: it searches the tests and the probes for the old
   sentence and for `PLAYBACK`, and reports any place the list does not name.

**Known, and not this batch's:** a sound made in the browser is the rendered sound resampled
by the browser to the device's rate, where the desktop's is resampled by rodio; nothing holds
the two to each other and nothing needs to.

**Gate** — in this order: the native suite, with item 8's browser-platform tests changed and
no other; clippy, fmt, the audit; the native gate, whose transcript must not change by one
expectation; `check_wasm.sh`; `build_wasm.sh` within its budget, and `build_wasm.sh --gate`,
whose size is reported — the budget is the visitor's; `web-smoke.sh`, with one assertion added
to its two passing runs — no `AudioContext` was constructed; and `web-app-gate.sh`, with item
8's rows changed and a new group, `playing`, in both browsers on the gate's bundle.

The probe's tap and its onset detector are the spike's, made the gate's and **committed
with this batch under `coordination/web/probes/`** — until then they are in no commit, and
the handoff carries them: a script
installed before the page's own that counts constructions of `AudioContext`, hears everything
connected **into** a destination, and passes it on at no volume — headless Firefox plays
through the machine's real audio server (finding 9), and a gate is not to be heard. Three
things the gate says before any row: that its tap hears an oscillator the probe connects and
nothing without one; that Firefox has an audio server to play to, without which its context
never starts and the group is refused, not failed; and that **a row about a page nobody has
touched reads the page only through its console until that row's own key or click is past**,
because `page.evaluate` is itself a gesture to both browsers (finding 10). The rows:

- **R-11 in the browser**: read the transport stopped; Space; read it playing; the position
  read twice a second apart has advanced by about a second; the tap heard sound; Space; read
  stopped; the tap falls silent. From the `long` fixture, as natively;
- the same by a click on Play and Stop — **a real click**, the browser's own pointer moved to
  the button and pressed, since an event the page makes for itself is no gesture and the
  gate's existing pointer rows make theirs; where the button is comes from the bridge or
  from a stated place at the gate's window size, and the row passes on what the tap hears;
- **in time**: the `long` fixture's first sixteen onsets are each within 25 ms of half a
  second after the last — the spike's detector is good to 12 — on a first Play, on a second,
  and with the page stopped for 200 ms every second while it plays;
- **keys alone**: on a fresh page that has had no pointer event, Tab to Play and Space — the
  tap hears it. And **a click alone**: on a fresh page, one click on Play;
- **twenty Plays and Stops**: one context was constructed, and the twentieth is heard;
- **Space twice with the render between**: the tap hears nothing and the transport reads
  stopped;
- **the end of the song**: the `arrangement` fixture plays to its end and the transport then
  reads stopped; **a repeat** plays past its end and the position read has gone back;
- **B, then Space** plays from the bar and the position read says so;
- **too much**: the fixture at the limit plays; one a bar longer is refused with its
  sentence, on Play of the whole and on Play of two bars at its end, and nothing is rendered
  — the frame that refuses is as short as any — while Play of its first bars is heard;
- **Record** keeps its stop and says its new reason;
- **a device that will not open**: the probe makes the constructor throw; Play says the
  failure, the transport reads stopped, and with the constructor restored Play is heard;
- **a context that never starts**: the probe keeps it suspended; within four seconds item
  5's sentence is heard and the transport reads stopped; released, the next Play is heard,
  and still one context was constructed. **And one that stops**: suspended by the probe ten
  seconds into a song, the same;
- **Stop, then wait**, and the song's end, then wait: five seconds on, the page answers a
  key and has logged nothing;
- **negative control**: on a page where Play was never pressed the tap hears nothing, and the
  row that expects sound fails on it.

The batch reports, measured: how long the frame that handles Play takes for `bach`, for
the Bach invention at the limit and for the four tracks of chords at the limit, in both
browsers.

**Sitting** (Firefox, then Chromium; with Orca): Play by Space and by the button — is the song
heard, from its start, in time; is what Orca says on Play heard over the music or lost under
it, and which would Jesse rather; move about the roll and open a menu while it plays — does
the sound hold; Stop; play to the end.

**Leaves out:** Record and the MIDI input (W3b); a played key and a pad; any render off the
page's thread (W4); a sound from a file (W5a, W5b).

### W3b — the MIDI input, and a take

**Builds:**

1. **The seam.** The application is handed its MIDI ports. Three acts and a standing:
   **ask** — the only one that may put a prompt in front of a person; **the ports**, each
   with a name and an identity, read from whatever access there already is and never asking;
   **connect**, by identity, to a callback; and where things stand — unsupported, not asked,
   asking, refused, granted, or failed with a reason. The desktop's side is midir, as now:
   asking does nothing, the standing is granted, or failed with the sentence a failed
   enumeration shows today. `MidiInput` is made without asking — its constructor's refresh
   reads the ports and nothing more, and what it finds is neither reported nor made the
   control's description, so a page nobody touched and the native gate's dump both read as
   they do now. The refresh button asks, then reads.
2. **The browser's side, in `evil-web`**, over `navigator.requestMIDIAccess` without
   system-exclusive. **One access object is kept**: once access is granted, Refresh reads
   that object's inputs again and asks the browser nothing, so a Refresh in the middle of a
   take changes no port and no listener. The browser answers later than it is asked; the
   application already wakes ten times a second and reads where things stand each time, so
   an answer is said within that, with no pointer moved.
3. **The chosen port is kept by its identity, not its place in the list**, on both builds —
   the browser's `MIDIPort.id`, midir's `MidiInputPort::id`. A list that changes around the
   chosen port leaves it chosen and connected; the selection ends only when that port is gone.
   Today a port removed ahead of the chosen one moves the selection to its neighbour or drops
   a port that is still there; that is fixed on the desktop too, with a test, since it is one
   rule.
4. **One listener, and the newest choice wins.** A connection asks the browser to open its
   port and stands at opening, open or failed. It is open when the port itself reads open and
   connected — not when the promise resolves, which it may do with the port still pending
   because the device left meanwhile; "Recording MIDI from …" is said then and not before; a
   failure says "Could not open the MIDI input: …" and leaves nothing chosen. **A port that
   stays pending is read again on each wake**: when it reads open — the device came back —
   its listener is set and its sentence said then, under the same generation. Every choice —
   a port, Off, the same port again — is a new generation: the listener is taken off the
   port the moment a choice ends, before anything of it is dropped, since a message for a
   listener that has gone would end the page; an answer that comes for an older generation
   changes nothing and sets no listener; so a port never has two. **Ports are not closed**:
   a port with no listener sends the page nothing, and the browser lets go of them when the
   page goes — so there is no close to race the next opening.
5. **`Shown::MidiInput` leaves the list of what the browser leaves out**, and every sentence
   is said, not only shown. The port list and the refresh button are drawn, named as they are
   natively. Each change of standing or of connection after the first read is reported once —
   today it is a small label beside the control and nothing reads it — and is the control's
   description until the next. One sentence each, made in `speech.rs`:
   - not asked — "Press Refresh MIDI inputs to look for a MIDI input."; in the browser this
     is the description from the start, said to whoever reaches the control;
   - unsupported — "This browser has no MIDI input.";
   - asking — "Waiting for the browser's permission for MIDI."; a Refresh pressed meanwhile
     says it again and asks nothing;
   - refused — "The browser refused MIDI input; allow it in the browser's settings for this
     page."; a Refresh after it asks the browser again and says what comes back, which may be
     the same refusal with no prompt;
   - granted — "MIDI inputs: " and the count, or "No MIDI input found.";
   - failed — the desktop's existing sentence for a failed enumeration;
   - in the browser, a port that appears or goes after access is granted is said when the
     browser says the list changed. The desktop reads its list when Refresh is pressed, as
     now, and says a change then: its enumeration opens a MIDI client each time and is not
     run on every wake;
   - the chosen port going — the desktop's existing sentence.
6. **Record stops being refused**, and its reason goes. A take records what the chosen port
   sends — notes, pitch bend, pressure and controllers — through the channel the desktop's
   connection feeds, stamped as the desktop stamps them, when the frame reads them.
7. **A take keeps the desktop's rule for a preview it cannot have, and says it every time.**
   The recorder has its own clock, made when Record is pressed, and a take goes on without a
   preview: `render_job_queued_file_refuses_preview_but_recording_continues_independently`
   holds that the take continues. What says so today is one branch — a render that fails
   while a take runs reports "Recording with the independent clock; audio preview
   unavailable: …". A preview can now fail a take four more ways: the limit refuses it, the
   device cannot be opened, the device is not running, the sound's rate has no buffer. **One
   function makes that sentence from a reason, and every way a preview is lost while a take
   runs reports through it** — the existing branch included, on both builds — so a person is
   never told only that the preview failed and left to guess whether the take did. None ends
   the take. In the
   browser the recorder is made after the render has run, so its clock starts a frame before
   the sound.
8. **What moves:** `shown_tests.rs`'s two lines for the MIDI input, the absence walk's
   (`web-app-page-rows.mjs`), every row and test that hears Record's reason, the page's
   desktop-only list and its test, and the inventory's lines for Record, the MIDI input and
   "MIDI support could not be initialized".

**Known, and W4's to end:** what a person plays while the page is rendering reaches the page
when the render is done, and lands at the take's first moment. For three seconds in Firefox
the page is not answering and nothing sounds; the sitting is told, and no row pretends
otherwise.

**What the first run may find:** whether Firefox answers `requestMIDIAccess` through a prompt
Playwright can answer — it has the function: `web-smoke.sh`'s `asks-for-midi` control wraps
the browser's own and fails in Firefox as it must; and whether headless Chromium lists the
machine's ALSA sequencer ports. Where it does, a row through the kernel's MIDI Through port
is **required**, not offered; where it does not, a real port is the sitting's alone and that
is said here before the batch is built (D4).

**Gate** — the native suite with item 3's test and item 8's changes; clippy, fmt,
the audit; the native gate's transcript unchanged; the wasm check and both builds;
`web-smoke.sh`, still proving a page nobody touched asks for no MIDI; and `web-app-gate.sh`'s
new group, `midi`, in both browsers. The probe stands in for the browser's MIDI with a script
installed before the page's own — an access object whose inputs the probe adds, removes,
opens, leaves pending or refuses to open, and sends from, and which records every listener
set and taken off — so every state has a row in both browsers whatever either offers:

- nothing is asked before Refresh — counted by the probe — on a page that has been played and
  edited; the control's description then is the not-asked sentence; and one Refresh asks once;
- unsupported: the function is absent; Refresh says so;
- refused: the sentence; a second Refresh asks again and says the same;
- asking: the request is left unanswered; the sentence; a second Refresh asks nothing; the
  page still answers keys. Answered with no input from the probe after, the list is said
  within a second;
- granted with two inputs: the list holds both by name; choosing one is announced when it
  opens;
- **a port that will not open**: its sentence, and nothing is chosen; **one whose opening
  resolves still pending** is not said to be recording, and when the probe then makes it
  open, is — and a note sent after is in the take; and one whose opening is answered
  after Off was chosen changes nothing and sets no listener;
- **Off, then a message**: after Off, and after another port is chosen, the first port has no
  listener, the probe sends from it, and the page goes on answering keys;
- **the same port again while it opens**: A, Off, A, then both answers in either order — one
  listener, and one note sent is one note in the take;
- **Refresh during a take**: the browser is asked nothing, and the take goes on with one
  listener;
- **a take**: choose the input, Record, the probe sends two notes a second apart, a bend and a
  controller, Stop take; the roll reads two notes about a second apart, the first within a
  tenth of a second of where the transport's position read when it was sent, and the take
  is one undo entry;
- **Record over the limit**, **Record with a context that never starts** and **Record with a
  constructor that throws**: the take goes
  on, the independent clock's sentence is said with the reason, and a note sent is in it;
- **R-20 in the browser**: undo during a take, as natively;
- **the list changing around the chosen port**: the first of two removed while the second is
  chosen; one added ahead of it; two with the same name — the chosen one goes on recording;
- the chosen port removed with a note held: said once, and after Stop take the note has its
  end;
- negative control: with the stand-in sending nothing, the take row fails.

**Sitting** (Chromium, with Orca, and Firefox if it offers a prompt): Jesse's own keyboard or
a virtual port — Refresh, the browser's prompt, choose the input, record four notes while the
song plays, stop, hear them back in time. Then the same with the permission refused. The real
prompt and a real device are judged here.

**Leaves out:** MIDI output; a played key on the Keyboard view and a pad; system-exclusive
messages, which this build never asks the right to read; an event's own timestamp, which
neither build uses — an issue for both, filed when this section settles.

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
  not this crate — **and W-A1's rows R-1 to R-17 as a matrix written into W-A2's section**: for each
  row, what drives it natively, in Chromium over CDP, and in Firefox, where the real tree is not
  readable and a row may be the sitting's alone, said in advance and waived in writing where D4
  needs it. The rows that turn on F6 are re-stated on whatever region chord a person finds a
  browser leaves alone. D11's "same bar" means the same rows wherever a driver exists and a named
  human row where none does; W6 re-runs the matrix before its inventory.
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
- **Raised in W-A1's review and owed to these sections when each is written** — none settled
  here: *W1* — the library split moves `speech.rs`, `announcer.rs` and the rest of W-A1's
  modules with every other module, so `evil-web` reads D5's strings from the library and never
  copies them. *W2* — `target/tools` is ignored, so CI and a fresh checkout have no CLI; the build
  script installs the lock's version there when it is absent **or is any other version**. *W3* — the MIDI port sits on a shown surface, and Web MIDI is absent in
  Firefox and permission-gated in Chromium: it gets named, announced states for unsupported, denied
  and granted, each with a row. Gating a pad and a piano key means reaching surfaces D12 hides; either the gate is Play alone and the others are gated when first
  shown, or the test route is itself something W6 asserts absent. *W4* — the worker is very likely a **second wasm artifact**, built from the
  render crates with its own entry and glue, because the application's module binds to a window
  and a document a worker does not have; a smoke test settles it before the section is written,
  and D10's "the same wasm module" is corrected with it. The frame-time bound and
  the maximum render length are numbers its section states, with the copy-or-transfer of the PCM
  and the worker's start-up time inside the gate, and a render over the limit an announced refusal
  rather than an error; cancellation is terminate-and-respawn or a checked generation, with a row
  for cancelling at the last moment.
  *W5a* — its gate lints the shared crates for `wasm32` with `-D warnings`, which W1 could not (`evil-project`'s `drop(file)` in the atomic save is the one known failure); the path-based wrappers do not exist on `wasm32`, so a missed conversion fails to compile
  rather than at Play. *W5b* — `beforeunload` while the document is unsaved or a save is in flight, and a
  keyboard-operable, announced discard prompt for New and Open, both in its scope and gate and
  asserted again in W6; how a probe opens and saves a file in a browser at all, since the file
  pickers are browser UI no driver operates: a test-only open-from-bytes seam that W6 asserts is
  not reachable by a visitor, or sitting rows with their waiver; a failed or partial write always
  ends the in-flight state, with a retry row; the same file open in two tabs; and whether answering "yes, the file was kept" may
  clear the unsaved state at all, or a download is only ever "exported, unverified" — D13's own
  open question, and Jesse's to rule. *W-A2* — text reaches the page only as `textContent` and
  attribute values, with a row for a track named with `<`, `&`, a newline and a direction override.
  *W6* — the bundle's URLs change with its bytes, or a returning visitor's cached glue meets a new
  wasm.
- **W6 (`#9`)** — D12's inventory gate, the nine-step sequence in the browser with both kinds of
  evidence, and `insiculous_web`'s `npm run verify`.

## The record's corrections

Committed to `docs/web-port.md` with this plan when it settles, as a dated section in the record's
own style: the split of W-A and of W5 and the new order (D2); W4 is a worker (D10); the shipping
gate is the inventory, with the nine steps as its minimum (D12); the transport's real location; and
*The tree today*'s counts. The milestone's description is updated to match.

W2, corrected with its section (2026-11-25): `build_wasm.sh` has no `--sync` — the bundle reaches
the site in W6; "green in CI" cannot be shown by the batch that adds the workflow, so the first
run is checked after the push and W-A2 waits on it; and the batch runs the application in both
browsers once, clicking nothing, which the record did not ask for.

W3, corrected with its section (2026-11-27): the browser's preview is not played through rodio
— `rodio/wasm-bindgen` stays off, and a preview is one buffer on a context the web entry owns;
the browser's MIDI is written on `web-sys`, not midir's web backend; "press a pad, press a
piano key" leaves the gate with the views D12 holds back; and until W4 a render in the browser
runs on the page's own thread under a limit, which the record did not foresee because it put
the render before the sound. `coordination/web/w3-spike-result.md` is the measurement.

## What is asked of Jesse

1. Settle this plan. **D2** (the order), **D4** (two kinds of evidence), **D10** (a worker, not
   chunking) and **D12** (the first page holds back the patch lab, pads, mixer and automation) he
   ruled on 2026-11-24, as recommended; round 1's twenty dispositions stand with them.
2. `kimi` logged in again on bob-ross, so the next round has its default reviewer.
3. Orca sittings, each short, each booked when its batch reaches it: W0-native and W0-web (done);
   **four in W-A1** — one per batch, and the transport proof that opens W-A1b; W-A2 (which opens
   with the once-or-twice question and the browser-owned chords); W6. W3's audible checks and
   W5b's dialogs are counted when their sections are written.
5. **A written waiver for W-A1c (D4):** Open from the menu and Save As on a new project pass
   through the desktop's file dialog, which the probe cannot drive; those two steps are judged at
   the sitting alone. **Granted by Jesse, 2026-11-24: "settled, yes on the waiver".**
4. The ruling if a half of W0 ends anything but passed.
