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
  button a description saying why — pending Jesse's ruling between that and leaving it upstream's.
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

### W-A1b — the transport, and a name for everything on a shown surface

**Files:** `transport_readout.rs` (new); `app/chrome.rs` (744 lines — the toolbar's transport block
moves into the new module, which brings it under 600); `app/sounds.rs`; `midi_input.rs`;
`instruments.rs`; the probe and gate.

**Builds:**

- **The transport readout**: one focusable, named status in the toolbar, present at every window
  width, giving state, tempo, meter and position, the position in bars and beats as well as time.
  It must be true whenever it is read and silent between reads (R-11), and **which mechanism gives
  both is not known**: W0 heard a focused node's name change spoken on every key, on the roll's
  role, and never tried a name that changes on a timer. So the batch opens with a proof, heard by
  Jesse before anything is built on it — a focused status node whose name changes once a second:
  - *if Orca stays silent*, the name is simply kept current, and a screen reader's own
    read-current-object is always right;
  - *if Orca speaks each change*, the name stays fixed ("Transport"), the live reading is the
    node's description and numeric value kept current, and an explicit read — arriving, **Enter**
    on it, the roll's chord — announces it. Never Space: Space is the transport's own play and
    stop everywhere (`app.rs:429`), so a read gesture on Space would stop the song to tell you
    where it was. The proof then also says what Orca's
    read-current-object returns for that node, and if it is stale the readout's own help names the
    gesture that is not.
  The result of the proof is written into this section before the rest of the batch is handed off.
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
announcement and no spoken name change** for five seconds. (The probe's private
session has no audio device to open; the row uses the application's existing null-device path or a
clock stub the batch names — playback's *sound* is not what the row is about.) R-12 — a tree dump
of each shown view with **no focusable node unnamed**, asserted by the probe, not read by eye.
R-13 — the application started with the flag off, the flag turned on, the tree present.

**Sitting:** the transport read stopped and playing; Tab through the toolbar and the side panel
hearing a name at every stop; a tempo typed and committed.

**Leaves out:** the Mixer, Synth, Keyboard and Pads views and the Automation panel, beyond the
Delete line — held back from the web build under D12, each its own issue in the milestone, filed
by the planner before W-A1 reports done.

### W-A1c — the arranger, the files, and the nine steps

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
  *W5a* — the path-based wrappers do not exist on `wasm32`, so a missed conversion fails to compile
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
