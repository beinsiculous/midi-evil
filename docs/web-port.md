# MIDI Evil in the browser — feasibility and the port's steps

This document records whether MIDI Evil can run in a web browser, what stands in the way, and the ordered steps of a port that would put it at `/midi-evil/play/` on beinsiculous.com. It is a feasibility record and a sequence, not a plan to execute now. The browser build is its own gated effort: its plan will live at `coordination/web/plan.md` in this repository and goes through adversarial review before W0 starts.

The evidence below was checked on 2026-11-05. **Line numbers are as of that date and must be re-verified against the tree before any step starts.**

## Verdict

**Portable with a dedicated web entry crate and bounded shims; three features will not cross without more; and the application's accessibility in a browser is unproven and must be proven first (W0, below).**

## Evidence

Each row was checked against the tree on 2026-11-05. A path with no crate named is relative to `crates/midi-evil/src/`. A path after an `evil-*` crate's name is inside that crate's `src/`. The paths given for eframe 0.35's web runner and rfd's wasm backend are in those dependencies' own sources. `tools/audit-public-packages.sh` is relative to this repository's root, and `build_wasm.sh` is the engine's (`insiculous_2d/scripts/`).

| area | finding |
| --- | --- |
| UI | eframe 0.35 has a web runner; the app's features (`default_fonts, glow, x11, wayland`) are all wasm-safe. `main.rs:54-64` uses `run_native`/`NativeOptions` (desktop-only) but `StudioApp::new` (`app.rs:115-161`) is already separable: `LaunchOptions::default()`, `None` for the instance. Art is `include_bytes!` (`art.rs:5-7`). |
| Accessibility | eframe 0.35's web runner discards `accesskit_update` as "not currently implemented" (`src/web/app_runner.rs:375`), and the app's eframe features omit `accesskit`. The piano roll selects a note from `response.clicked()` and the pointer position (`piano_roll.rs:350`) and Delete (`:375`) acts on that selection, so even with an accessibility tree, editing is not keyboard-operable today. A browser MIDI Evil would be a canvas the surrounding page's axe pass cannot see into. The house bar is WCAG 2.2 AA, one semantic codebase per surface. |
| DSP core | `evil-synth`, `evil-beats`, `evil-render`, `evil-project`: no threads, no clock, built-in presets need no files. Ports as is. |
| Won't compile | `instance.rs:3` (`std::os::unix::net`, also ungated for Windows); 17 sync `rfd::FileDialog` sites (`app.rs:487…1270`, `beats/bank.rs:26,49,65,153`, `patch_lab.rs:462,594`) and `file_dialog()` at `app.rs:1219` returning `rfd::FileDialog`, a type rfd hides on wasm; `crates/midi-evil` is a binary with no `lib.rs` (`main.rs:1-24` declares 24 modules), so a web crate cannot depend on it until it is split. |
| Crashes on wasm | `std::thread::spawn` at `render_job.rs:61` — every preview and WAV render; `Instant::now` at `beats.rs:204` (every frame on Pads), `beats_player.rs:61`, `app/recording.rs:35,81`, `app.rs:931`. `web_time::Instant` is a drop-in and already in `Cargo.lock` via eframe. |
| Silent on wasm | cpal is locked **without** `wasm-bindgen`, so it picks the null host and rodio's sink fails quietly; fix is the `rodio/wasm-bindgen` feature. Every `std::fs` call returns `Unsupported`. `MessageDialog` returns `Ok`, never `Yes`, so `app.rs:341` and `beats/bank.rs:16` can never discard changes in a browser. rfd's wasm `FileHandle::write` returns `Ok(())` whether the user clicked Download or Cancel (`src/backend/wasm.rs:391-399`), so a save flow that clears `dirty` on that result loses work. |
| Files | `Project::from_json`/`to_json_pretty`, `import_midi(&[u8])`, `export_native_project` → bytes already exist, but the application reaches the path-based entry points: `Project::load` (`app.rs:699`), `Project::save` (`:758`), the linked-MIDI export and `atomic_write` (`:744,763`), patch `atomic_write` (`:526`), `load_patch`/`load_patch_file` (`:467,493`), `export_native_project_to_path` (`:810`), `import_midi_file` (`:847`), `load_drum_kit` (`:1174`), `write_wav` on the render worker (`render_job.rs:38,52`), kit read/write and pad WAV (`beats/bank.rs:32,57,71,160-168`), `SampleOscillator::read` (`patch_lab.rs:492,726`). The structural job is `patch_base: &Path` threaded through `evil-render` (`lib.rs:143-298`, resolved at `:324` and `:404`) and `evil-exports/native.rs:51`. WAV/AIFF read and write (`evil-synth/wav.rs`, `aiff.rs`, `sample.rs`) need byte-level twins; hound already has `WavReader::new(Read)`. |
| MIDI | midir 0.11 ships Web MIDI for wasm32 with no feature flag; `ports()` is empty until the user grants permission (the refresh button at `midi_input.rs:57-63` covers it). No Safari. Whether the connection and its callback deliver events into the app's channel in a browser is unproven. |
| Not crossing | microphone capture (cpal's WebAudio host has no input), `pick_folder` for stem/part export (no web equivalent; zip instead), single instance, low-latency AudioWorklet (needs nightly atomics + COOP/COEP). |
| Release audit | `tools/audit-public-packages.sh:49` asserts the workspace holds exactly the eight native crates; any new member fails it until the assertion distinguishes publishable crates. |
| Toolchain | wasm32 target installed; the wasm-bindgen CLI is 0.2.126 but midi-evil's lock resolves 0.2.129 — the CLI is installed at the lock's version and the engine's `build_wasm.sh:145-165` pattern (hard-fail on mismatch) is the guard. |

## What changed after the evidence was checked

Re-read on 2026-11-12, after two further commits. The table above is kept as checked; these are the differences a reader must carry into it.

- **The native build now has AccessKit.** `crates/midi-evil/Cargo.toml` enables eframe's `accesskit` and `persistence` features, so the table's "the app's eframe features omit `accesskit`" is no longer true, and the native half of W0 starts from a build that already exposes egui's standard controls to a screen reader. Nothing else in the Accessibility row moved: eframe's web runner still discards the update, and the piano roll still selects a note from the pointer.
- **Line numbers in `app.rs`, `beats.rs`, `patch_lab.rs` and `main.rs` have shifted**, most by 40 to 100 lines, and `load_patch_file` moved to `app/sounds.rs`. The files the table cites in `piano_roll.rs`, `instance.rs`, `beats/bank.rs`, `render_job.rs`, `beats_player.rs`, `app/recording.rs`, `midi_input.rs`, `evil-exports` and `evil-synth` did not change.
- **New sites the steps must also cover**, in none of the rows above:
  - `recovery.rs:61,76` — two more synchronous `rfd::FileDialog` uses (W1's dialog seam, W5's documents);
  - `recovery.rs:85` — a second `std::thread::spawn` (W4's job runner must take it too);
  - `app/drafts.rs:92-96` and `app/sounds.rs:29-33` — two more confirms that compare `MessageDialog` with `Yes` (W5's confirm fix);
  - `app/sounds.rs:73,116` — two more callers of `file_dialog()` (W1);
  - `instruments.rs:132,166` — `eframe::Storage` persistence, which on the web is the browser's local storage: whether the saved state belongs there is W5's to decide.

## What the site offers a wasm application

The site is `insiculous_web`, cloned beside this repository in the Be Insiculous working set; the paths in this section are relative to it. Today it offers:

- drop-in bundles under `public/<surface>/vN/` (`README.md:110-150`);
- a 25 MiB per-file gate and the embed checks in `scripts/postbuild-check.mjs`;
- axe, announce and sideways-scroll gates in `npm run verify`;
- no COOP/COEP (the headers are commented out in `public/_headers`);
- embeds **hard-gated on WebGPU** (`src/scripts/webgpu-gate.ts`).

MIDI Evil renders through eframe's glow painter (WebGL2), so its embed page needs its own loader with a WebGL2 probe rather than the WebGPU gate the site's `GameEmbed` uses. W6 builds that loader.

## The engine's pattern to copy

The Insiculous 2D engine, `insiculous_2d`, already ships a wasm build to the same site; the paths in this section are relative to it. Its pattern:

- a `cdylib` crate with `#[cfg(target_arch = "wasm32")]` entry modules (`crates/playground/src/lib.rs:11-14`);
- a root `[profile.wasm-release]` (`Cargo.toml:24-29`);
- `scripts/build_wasm.sh`: cargo build → wasm-bindgen `--target web` → size gate → `--sync` into the site's `public/`;
- `scripts/check_wasm.sh` as CI (`.github/workflows/wasm-check.yml`);
- `common::vfs` for files and `web_time` for clocks;
- rodio with `wasm-bindgen` and gesture-gated output (`crates/audio/src/manager/mod.rs:191-258`).

## Accessibility

Accessibility gates the browser build, it is judged by workflows, and it is proven first.

The bar for `/midi-evil/play/` is that **every exposed editing workflow is keyboard-operable and meaningfully announced**. The reason is the house bar — WCAG 2.2 AA, one semantic codebase per surface — and the fact that a browser MIDI Evil is a canvas: the surrounding page's checks cannot see into it, eframe's web runner does not forward an accessibility tree today, and the piano roll's editing is driven by the pointer.

The minimum regression scenario is performed keyboard-only and then with a screen reader, inside the application:

1. create a new project;
2. add an instrument;
3. discover the instruments;
4. add a note;
5. select an existing note and identify its pitch and timing;
6. change it;
7. delete it;
8. read the transport;
9. save.

Passing the surrounding page's axe and announce checks is not sufficient. Upstream AccessKit on the web is an enabler the spike may choose, never a substitute for the sequence.

W0, a spike in two halves on the real piano roll, opens the effort and decides whether the port runs at all and on which mechanism. W-A implements the application's keyboard model before W6, and W6 ships only when the sequence passes in the browser. The site's `/midi-evil/` page says so in one sentence.

## The steps

The steps run in this order; each names the files it touches and the gate that closes it. Every step after W0 waits on W0's written result, and a "not reachable" result parks them all.

### W0 — the accessibility spike, in two halves, before any port work

*Native:* build MIDI Evil with eframe's `accesskit` feature and drive the real piano roll with a screen reader (Orca) and the keyboard — what the app's own semantics expose, independent of any web runner. The piano roll selects a note only from the pointer today, so this half includes a throwaway keyboard-and-announcement prototype on that real code, enough to attempt the gate; the production keyboard model is W-A's, and the prototype never lands. *Web:* a throwaway wasm harness around the same piano-roll interaction code, exploring the bridging mechanism: eframe's web AccessKit if upstream has it by then, a DOM twin of the focused control, or egui's own keyboard model.

**Files:** the native half builds with eframe's `accesskit` feature; the web half's harness is throwaway and never lands in `crates/`.

**Gate, both halves:** select an existing note, hear its pitch and timing, change it, delete it, and read the transport. The result is written: either it names the mechanism W-A will build on, or it says "not reachable" and parks W1–W6.

### W1 — compile-safe seams, native behaviour unchanged

Split `crates/midi-evil` into `src/lib.rs` and a thin `main.rs`, so a web crate can depend on the library. Put a `FileDialogs` trait (pick file, save file, confirm) in front of the dialogs, with the rfd native implementation behind it, replacing the 17 direct `rfd::FileDialog` uses and the `file_dialog()` return type, so the library names no desktop-only type. Put `#[cfg(unix)]` on the single-instance module with a no-op elsewhere, which also fixes the Windows build. Use `web_time::Instant` at the five clock sites.

**Files:** `crates/midi-evil/src/lib.rs` (new), `main.rs`, `instance.rs`, the dialog sites in `app.rs`, `beats/bank.rs` and `patch_lab.rs`, and the clock sites in `beats.rs`, `beats_player.rs`, `app/recording.rs` and `app.rs`.

**Gate:** the native suite, clippy and fmt unchanged; `cargo check -p midi-evil --lib --target wasm32-unknown-unknown` compiles. The dialog and render seams may be stubs that return "not on this platform" — compiling, not working, is W1's bar.

### W2 — the web crate and its gates

`crates/evil-web`: a `cdylib` with `publish = false` and a `#[cfg(target_arch = "wasm32")]` entry on `eframe::WebRunner`, depending on the real `midi-evil` library. A root `[profile.wasm-release]` as the engine's. `tools/audit-public-packages.sh` asserts over the publishable crates and names `evil-web` as the non-published member. `scripts/check_wasm.sh` runs `cargo check -p midi-evil --lib -p evil-web --target wasm32-unknown-unknown`, then clippy the same, and `.github/workflows/wasm-check.yml` runs it, modelled on the engine's (stable toolchain, `wasm32-unknown-unknown` target). `scripts/build_wasm.sh` is modelled on the engine's — a pinned CLI check against `Cargo.lock`, a size gate, and `--sync` to `../insiculous_web/public/midi-evil/v1/` — with the CLI installed at the lock's version (`cargo install wasm-bindgen-cli --version <Cargo.lock's wasm-bindgen> --locked`).

**Files:** `crates/evil-web/` (new), the root `Cargo.toml`, `tools/audit-public-packages.sh`, `scripts/check_wasm.sh` (new), `.github/workflows/wasm-check.yml` (new), `scripts/build_wasm.sh` (new).

**Gate:** the check script green locally and in CI; the native audit green; `build_wasm.sh` produces a bundle.

### W3 — audio and MIDI

Turn on the `rodio/wasm-bindgen` feature, so cpal stops picking the null host. Create the audio output on the first gesture, as the engine does. Wire midir's Web MIDI path to the refresh button and the recording channel.

**Files:** `crates/midi-evil/Cargo.toml` (the `rodio` features), where the output is opened, and `midi_input.rs` with the recording channel it feeds.

**Gates:** a test that the opened sink's host is not the null host on wasm; audible output after a click in Chromium and Firefox, recorded in the step's report; Web MIDI on a virtual ALSA port in Chromium — grant, refresh, connect, record note and CC events into the project — then permission denied, device removed, and the unsupported-browser message where Web MIDI is absent.

### W4 — the render job

Run the render job inline or chunked on wasm (`render_job.rs:61`), with a worker later if needed, so every WAV export reaches a real path before W5 tests it.

The WAV-recovery draft runs on a second spawned thread (`recovery.rs:85`) and goes through the same runner.

**Files:** `render_job.rs`, `recovery.rs`.

**Gate:** a long-song render (the Bach invention repeated to several minutes) keeps the UI answering within a bounded frame time, measured and quoted; and a WAV-recovery draft started from the Project menu runs to completion without a spawned thread.

### W5 — documents as bytes

The web `FileDialogs` uses `rfd::AsyncFileDialog` for picking. Saving goes through a **cancellation-aware export** of the effort's own — the File System Access API where present, else a download whose completion the user confirms — and never clears `dirty` on rfd's `write` result, which is `Ok` on Cancel. The `Yes`/`Ok` confirm mismatch is fixed inside the trait.

Every path-based consumer is converted: `Project::load`/`save` (`app.rs:699,758`), the linked-MIDI export and `atomic_write` (`:744,763`), patch write (`:526`), `load_patch`/`load_patch_file` (`:467,493`), `export_native_project_to_path` (`:810`), `import_midi_file` (`:847`), `load_drum_kit` (`:1174`), the render worker's `write_wav` (`render_job.rs:38,52`), kit read/write and pad WAV (`beats/bank.rs:32,57,71,160-168`), and `SampleOscillator::read` (`patch_lab.rs:492,726`). Two flows that arrived after the evidence was checked are converted with them: WAV recovery, whose `evil_imports::create_draft` opens the recording and saves the draft by path (`evil-imports/src/wav.rs:55-107,141`), gains a byte-based twin; and the sound browser, whose `Sound::load` reads a user's patch or kit by path against `Path::new(".")` (`instruments.rs:99-105`), loads from the bytes the visitor picked. The sound library that `instruments.rs` persists through `eframe::Storage` remembers paths, which mean nothing in a browser: W5 decides whether a remembered sound is restored from stored bytes or deliberately not restored, and says so in the interface. They go through a resolver trait that replaces `patch_base: &Path` in `evil-render` and `evil-exports`, and through byte-level WAV/AIFF twins in `evil-synth`.

**Linked artifacts are saved as one bundle** — the `.evil` and its linked `.mid` together — and the document stays dirty, with a visible partial-save result, until every part succeeds. Stem and part export become a zip download.

**Files:** the consumers above in `app.rs`, `app/sounds.rs`, `instruments.rs`, `recovery.rs`, `render_job.rs`, `beats/bank.rs` and `patch_lab.rs`; `evil-imports` (`wav.rs`); `evil-render` (`lib.rs`); `evil-exports` (`native.rs`); `evil-synth` (`wav.rs`, `aiff.rs`, `sample.rs`).

**Gate:** wasm round-trip tests —

- open → edit → save → reopen gives equal bytes;
- MIDI import, and export back;
- kit and patch load/save with relative references;
- a render reaches a download through the real W4 path;
- a recording picked for WAV recovery becomes a draft `.evil` that downloads and reopens;
- a patch and a kit picked through the sound browser's **Load files…** load, assign to a track and cycle, and a page reload does what the interface says it will with them;
- the linked-MIDI identity survives a save;
- a cancelled pick changes nothing;
- cancelling the real save overlay leaves the document and its dirty state intact;
- the project part succeeding while the MIDI part fails or is cancelled leaves the document dirty and says so.

### W-A — application accessibility

On W0's mechanism, the keyboard model for the piano roll and the arranger: focus, note navigation and selection by keys, creation and deletion, pitch and timing announcements, instrument discovery, and transport reading — so that the accessibility sequence above is operable inside the application, natively and on the web.

**Files:** `piano_roll.rs` and `arranger.rs` first, then the instrument list and the transport as the sequence reaches them, on the mechanism W0's result names.

**Gate:** the accessibility sequence passes natively with a screen reader.

### W6 — the site

`/midi-evil/play/` on the site, with its own loader and a **WebGL2** probe (eframe's glow painter), not the WebGPU gate; the bundle contract from the site's `README.md:110-150`; keyboard notes beside the canvas.

**Files:** in `insiculous_web`, the `/midi-evil/play/` page and its loader, and the bundle under `public/midi-evil/v1/` that W2's `build_wasm.sh --sync` writes.

**Gate:** the accessibility sequence, keyboard-only and with a screen reader, inside the application in the browser — or the page does not ship.

### Out

These stay out of the port:

- microphone capture — cpal's WebAudio host has no input;
- single instance — the module is a no-op off Unix after W1;
- Safari MIDI — Safari has no Web MIDI;
- AudioWorklet latency — a low-latency AudioWorklet needs nightly atomics and COOP/COEP, which the site does not send.

Folder pickers do not cross either; stem and part export become a zip download in W5.

## Tracking

The steps are issues in `beinsiculous/midi-evil`, one per step, under the milestone **MIDI Evil Web**.
