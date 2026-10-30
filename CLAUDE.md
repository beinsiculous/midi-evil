# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Workspace

MIDI Evil is a Rust music workstation with a native `.evil` JSON project format, Standard MIDI import/export, original synthesis, offline stereo rendering, and an `eframe`/`egui` desktop editor. Keep the code DRY, simple (KISS), and divided by responsibility rather than duplicating CLI and GUI logic.

## Common commands

Run these from the repository root.

### Build and verify

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
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
cargo run -p oracle -- --help
cargo run -p oracle -- inspect target/fixtures/Minimal.nsp
cargo run -p oracle -- midi inspect examples/public-domain/bach-invention-02/bach-invention-02.mid
```

Create and render a native song:

```bash
cargo run -p oracle -- project new "My Song" song.evil --tempo 120
cargo run -p oracle -- project add-track song.evil Lead --patch lead
cargo run -p oracle -- project add-note song.evil 0 69 0 128 --velocity 110
cargo run -p oracle -- project render song.evil song.wav
```

## Architecture

| Crate | Responsibility |
| --- | --- |
| `midi-evil` | Desktop app: project/file state, track controls, interactive piano roll, MIDI import, audio preview, and branding. Uses `eframe`, `rfd`, and `rodio`. |
| `oracle` | Unified CLI for native projects, synthesis, MIDI, and compatibility commands. CLI parsing, command modules, and atomic compatibility I/O are separate responsibilities. |
| `evil-project` | Native `.evil` model and validation, direct notes/tracks/tempo changes, MIDI-performance conversion, and atomic JSON saves. Handles legacy migration. |
| `evil-performance` | Neutral note/track and tempo-map representation with tick-to-seconds conversion. This is the shared performance model, not a NanoStudio-specific model. |
| `evil-midi` | Standard MIDI import/export for native projects and shared performance data. Contains no NanoStudio compatibility dependencies and is used by the desktop app. |
| `evil-synth` | Oscillators, pitch sweeps, ADSR envelopes, filters, deterministic noise, polyphony, stereo mixing, and WAV writing. |
| `evil-render` | Shared native-project renderer and patch loader used by both CLI and GUI. Resolves preset names and relative JSON patch paths, then produces a stereo mix. |
| `oracle-format` | Reverse-engineered `Package.prj` / `Package.sng` parsing and serialization plus safe `.nsp` archive handling. Treat this as a compatibility/validation oracle, not the native architecture. |
| `oracle-midi` | NanoStudio timeline expansion and compatibility MIDI export. Reuses `evil-midi` for Standard MIDI interchange but remains outside the native desktop dependency chain. |

### Main data flow

Native `.evil` documents and Standard MIDI Files are converted into `evil-performance` notes. `evil-render` maps those notes to seconds using the tempo map, renders each track with `evil-synth`, applies patch/gain/pan settings, and mixes stereo output. The same path serves CLI WAV export, desktop WAV export, and desktop `rodio` preview.

NanoStudio projects take a separate compatibility path through `oracle-format` and `oracle-midi`, after which they can use the shared performance and audio engines. Avoid making native features depend on NanoStudio formats unnecessarily.

## Native and interchange formats

- `docs/insiculous-crossing.md` is the game-engine integration contract. Preserve its MIDI export requirements: exact track names, composed meter and tempo at tick 0, gain/pan controllers, muted tracks as empty named tracks, percussion-safe channel assignment, complete tracks, and atomic writes. Never write the engine's `.mid.ron` sidecar.
- Canonical native extension: `.evil`; JSON format identifier: `midi-evil-project`.
- Preserve compatibility with `.rmp` files and `rusty-music-project` JSON. They load and migrate to the new canonical identifier when saved.
- Native projects store direct timeline notes, patch references, gain, pan, mute state, and master audio settings. Patch paths ending in `.json` are relative to the project file.
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
- Additional MIDI files under `/home/jedi/projects/insiculous` have been useful for different track counts, meters, and tempo maps. Do not assume their licenses or redistribute them as public-domain assets.

Keep NanoStudio compatibility checks distinct from native `.evil` and Standard MIDI tests so an oracle failure does not get mistaken for a native-engine failure.
