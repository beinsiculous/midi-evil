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

## The working set

This repository is cloned inside `~/projects/insiculous`, the Be Insiculous working set, at `midi-evil/`, beside `insiculous_2d`, which leverages it across the `.mid` crossing (`docs/insiculous-crossing.md`). MIDI Evil is the fourth part of Be Insiculous — its own thing, not a Games tool.

Branches are `main`, `dev`, and one per machine owner: `jesse` (Iroh and bob-ross) and `m` (Danny; created when M first works the repo). Commit to the branch matching the machine you are on, and confirm with `hostname` when unsure. Jesse merges `jesse → dev → main`.

`AGENTS.md` is a symlink to this file and is what Kimi, Codex and agy read.

`cargo fmt` is this repository's own rule and stays. The never-fmt rule in `insiculous_2d` is the engine's, not this repository's.

## Work tracking

Open work lives on the **Studio Board** (https://github.com/orgs/beinsiculous/projects/1) as issues in this repo. **Always pass `-R beinsiculous/midi-evil`** — a bare `gh` command resolves against the session's working directory, which is often the working-set root, so it lists and files against the wrong repository.

```bash
gh issue list -R beinsiculous/midi-evil
gh api repos/beinsiculous/midi-evil/milestones --jq '.[] | "\(.title): \(.description)"'
```

Issues are grouped into **sprint milestones**; each description records the batch's internal order and its gates. Take the next unblocked issue in a sprint, not an arbitrary one. Claim by assigning yourself; close with `fixes beinsiculous/midi-evil#N` in the commit.

**Unfinished work becomes an issue.** Anything you don't finish — work you deferred, debt you created, a follow-up you spotted — is filed before you report done. Never buried in a doc, never left as a bare `TODO:`, never dropped. The `file-issue` skill carries the shape; `sprint-planning` groups issues into shippable batches.

The board workflow is not installed here until its secret exists, so every new issue is added to the Studio Board by hand — the `file-issue` skill's list of repositories that add automatically does not hold for this one:

```bash
gh project item-add 1 --owner beinsiculous --url <issue-url>
```

## Review workflow

The adversarial-review skills live in `.claude/skills/`, `.kimi-code/skills/` and `.agents/skills/`, and `.claude/settings.json` registers the two hooks that route into them (`scripts/plan-review-hook.sh` and `scripts/commit-review-hook.sh`). `review/` is shared by every session at once, so each review conversation claims its own `review/<subject>/` with a plain `mkdir review/<subject>` — no `-p`: a directory that already exists belongs to another session, and another subject's directory is never touched. Approved plans go to `review/<subject>/plan.md` and are reviewed with `scripts/request-review.sh plan review/<subject>/plan.md --reviewer=kimi` before implementation. A `git commit` with 100 or more pending changed lines is denied by `scripts/commit-review-hook.sh` until the diff has been through a code-mode review; the `ADV_REVIEWED=1` prefix is used only after that review is adjudicated with the user. Skipping review is the user's explicit call, written by the user as the `Adversarial-Review-Skipped:` and `Skip-Signed-Off-By:` trailers at the end of the commit message — an agent never writes the skip trailers. `review/` is gitignored transients.

The scripts, prompts and skills are **copies**: the canonical ones live in the working-set root, not here. Never edit a copy — fix the root's and re-copy — and `scripts/check-skill-parity.sh` there reports any repo that drifted.
