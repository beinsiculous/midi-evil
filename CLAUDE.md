# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Workspace

MIDI Evil is a Rust music workstation with a native `.evil` JSON project format, Standard MIDI import/export, original synthesis, offline stereo rendering, and an `eframe`/`egui` desktop editor. Active work targets native sound recreation, NanoStudio 1-inspired feature parity tracked in `docs/nanostudio1-parity.md`, and assisted recovery of user-owned WAV exports, not NanoStudio backporting. Keep the code DRY, simple (KISS), and divided by responsibility rather than duplicating CLI and GUI logic.

## Common commands

Run these from the repository root.

### Build and verify

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The root workspace is the public native build and contains only `evil-*` crates. The ignored `compat/Cargo.toml` workspace links private Oracle crates for compatibility work. Verify both when changing shared native code:

```bash
cargo test --workspace
cargo test --manifest-path compat/Cargo.toml --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path compat/Cargo.toml --workspace --all-targets -- -D warnings
```

Format changes before committing:

```bash
cargo fmt --all
```

Run tests for one crate or one test by name/substring:

```bash
cargo test -p evil-synth
cargo test -p midi-evil
cargo test -p evil-project legacy_project_json_is_migrated_to_the_evil_format
cargo test -p midi-evil snaps_to_the_nearest_grid_point_without_underflow
cargo test -p evil-synth seeded_noise_is_reproducible
```

Build the desktop app for release:

```bash
cargo build --release -p midi-evil
```

### Run

Desktop editor:

```bash
cargo run -p midi-evil
```

CLI and command discovery:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- --help
cargo run --manifest-path compat/Cargo.toml -p oracle -- inspect target/fixtures/Minimal.nsp
cargo run --manifest-path compat/Cargo.toml -p oracle -- midi inspect examples/public-domain/bach-invention-02/bach-invention-02.mid
```

Create and render a native song:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- project new "My Song" song.evil --tempo 120
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-track song.evil Lead --patch lead
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-note song.evil 0 69 0 128 --velocity 110
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render song.evil song.wav
```

## Architecture

| Crate | Responsibility |
| --- | --- |
| `midi-evil` | Desktop app: project/file state, track controls, interactive pattern arranger and piano roll, MIDI import, keyboard/pad/desktop-MIDI recording, audio preview, and branding. Uses `eframe`, `rfd`, `rodio`, and `midir`. |
| `oracle` | Unified CLI for native projects, synthesis, MIDI, and compatibility commands. CLI parsing, command modules, and atomic compatibility I/O are separate responsibilities. |
| `evil-project` | Native `.evil` model and validation, reusable instruments/patterns, direct unique performances and arrangement parts, MIDI-performance conversion, and atomic JSON saves. Handles version-1 and legacy migration. |
| `evil-performance` | Neutral note/track and tempo-map representation with tick-to-seconds conversion. This is the shared performance model, not a NanoStudio-specific model. |
| `evil-imports` | Standard MIDI loading plus assisted WAV analysis and reconstruction: PCM loading, activity/onset detection, tempo candidates, and quantized native `.evil` drafts with JSON reports. Used by the desktop app and native CLI. |
| `evil-exports` | Standard MIDI export for native projects and shared performance data, plus event-preserving MIDI track edits. Contains no NanoStudio compatibility dependencies and is used by the desktop app. |
| `evil-synth` | Oscillators, pitch sweeps, ADSR envelopes, filters, deterministic noise, polyphony, stereo mixing, and WAV writing. |
| `evil-beats` | Original sample-pad and TRG-style drum voices with tuning, velocity, gate/release envelopes, overlapping-hit rendering, portable kits, and deterministic offline stereo kit rendering. Independent of NanoStudio compatibility code. |
| `evil-render` | Shared native-project renderer and instrument loader used by both CLI and GUI. Resolves synth presets, relative JSON patches, and drum kits, then produces a stereo mix. |
| `evil-porting` | Private NanoStudio compatibility: reverse-engineered `Package.prj` / `Package.sng` parsing and serialization, safe `.nsp` archive handling, timeline expansion, and compatibility MIDI conversion. Treat this as a porting/validation oracle, not the native architecture. |

### Main data flow

Native `.evil` documents and Standard MIDI Files are converted into `evil-performance` notes. Reusable pattern parts expand into the same neutral performance, so CLI/GUI audio and MIDI export share timing. `evil-render` maps those notes to seconds using the tempo map, renders synth tracks with `evil-synth` and drum tracks with `evil-beats`, applies instrument/gain/pan settings, and mixes stereo output. The same path serves CLI WAV export, desktop WAV export, and desktop `rodio` preview.

NanoStudio projects take a separate compatibility path through `evil-porting`, after which they can use the shared performance and audio engines. Avoid making native features depend on NanoStudio formats unnecessarily.

## Native and interchange formats

- `docs/insiculous-crossing.md` is the game-engine integration contract. Preserve its MIDI export requirements: exact track names, composed meter and tempo at tick 0, gain/pan controllers, muted tracks as empty named tracks, percussion-safe channel assignment, complete tracks, and atomic writes. Never write the engine's `.mid.ron` sidecar.
- Canonical native extension: `.evil`; JSON format identifier: `midi-evil-project`.
- Preserve compatibility with `.rmp` files and `rusty-music-project` JSON. They load and migrate to the new canonical identifier when saved.
- Native projects store reusable instruments/patterns, direct unique timeline performances, arrangement parts, gain, pan, mute state, and master audio settings. Synth patch paths ending in `.json` and drum-kit paths ending in `.evilkit` are relative to the project file. Tracks sharing an instrument share its MIDI channel without losing their exact track names.
- Native timing defaults to 480 ticks per quarter note, but MIDI files retain their own division. Tempo changes after the initial tempo are represented explicitly.
- MIDI contains performance data, not instrument audio. Use original synth patches for sound and WAV for rendered audio interchange.
- Preserve deterministic synthesis: the same patch and seed must produce identical audio.
- Preserve signed pitch-bend events, RPN 0 bend ranges, channel/polyphonic aftertouch, CC 1 modulation depth, CC 11 expression, and other CC events through MIDI import, native JSON, tempo-mapped synthesis, and MIDI export. Unsupported CC/RPN render mappings must be reported as not rendered even though their data is preserved.
- Do not remove working WAV support merely to avoid samples; original synthesis is the preferred instrument workflow, while WAV remains useful for interchange.

## UI and branding

- Do not use original NanoStudio artwork in the rendered UI.
- Keep editable Aseprite masters next to their app-ready PNG exports.
- The current user-selected Dr. Maxwell branding is documented in `crates/midi-evil/assets/SOURCE.md`; preserve that provenance.
- Current assets are `midi-evil-icon.aseprite` and `midi-evil-splash.aseprite`, with matching PNG files. The toolbar uses the icon, and the welcome/About window uses the splash.

Re-export the current images:

```bash
aseprite -b crates/midi-evil/assets/midi-evil-icon.aseprite \
  --save-as crates/midi-evil/assets/midi-evil-icon.png
aseprite -b crates/midi-evil/assets/midi-evil-splash.aseprite \
  --save-as crates/midi-evil/assets/midi-evil-splash.png
```

## Integration fixtures

Useful real-data checks beyond unit tests:

- `target/fixtures/Minimal.nsp`: NanoStudio parsing, binary round trips, MIDI export, and arrangement behavior.
- `examples/public-domain/bach-invention-02/bach-invention-02.mid`: public-domain format-1 MIDI with 384 ticks per quarter and two note tracks. Its source and checksum are in the adjacent `SOURCE.md`.
- Additional MIDI files from the Insiculous project have been useful for different track counts, meters, and tempo maps. Do not assume their licenses or redistribute them as public-domain assets.

Keep NanoStudio compatibility checks distinct from native `.evil` and Standard MIDI tests so an oracle failure does not get mistaken for a native-engine failure.
