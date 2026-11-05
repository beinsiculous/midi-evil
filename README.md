# MIDI Evil

A Rust music workstation for original synthesis, drum performance, editable `.evil` projects, and reference-guided sound recreation. The focus is building the instruments and composition features needed for new songs and assisted recovery of older audio exports—not backporting a compatibility engine.

## Run

Build and run the native desktop app:

```bash
cargo run -p midi-evil
```

The root workspace contains only native `evil-*` crates and builds without any Oracle source. Run the native verification suite with:

```bash
cargo test --workspace
```

## Publish native crates

The `oracle` and `evil-porting` packages are marked `publish = false`. The desktop and its native libraries can be published without uploading the compatibility tooling. Publish the native crates in dependency order, allowing each published crate to become available before packaging the next:

```bash
cargo publish -p evil-performance
cargo publish -p evil-synth
cargo publish -p evil-beats
cargo publish -p evil-project
cargo publish -p evil-imports
cargo publish -p evil-render
cargo publish -p evil-exports
cargo publish -p midi-evil
```

Use `cargo package -p midi-evil --allow-dirty --list` to inspect the desktop package contents. Full registry verification requires the preceding native crates to have been published; it does not require any Oracle package.

## Native `.evil` projects

A `.evil` file is a versioned, human-readable JSON document containing the song name, tempo, meter, reusable instruments and patterns, direct unique timeline performances, arrangement parts, gain, pan, mute state, and master audio settings. Its format identifier is `midi-evil-project` and the current schema version is 2; it contains no proprietary NanoStudio records or binary data. Version-1 `.evil`, legacy `.rmp`, and `rusty-music-project` JSON remain readable and migrate to the current format when saved.

Create and render a song from scratch:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- project new "My Song" song.evil --tempo 120
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-track song.evil Lead --patch lead --gain 0.8 --pan -0.4
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-track song.evil Bass --patch bass --gain 0.9 --pan 0.2
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-note song.evil 0 69 0 128 --velocity 110
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-note song.evil 1 45 0 256 --velocity 120
cargo run --manifest-path compat/Cargo.toml -p oracle -- project show song.evil
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render song.evil song.wav
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render song.evil section.wav --start-tick 480 --end-tick 960
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render-stems song.evil exports/stems
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render-parts song.evil exports/parts
cargo run --manifest-path compat/Cargo.toml -p oracle -- project export-midi song.evil song.mid
```

Track indices are returned by `add-track` and shown by `show`. Note arguments are track, MIDI pitch, start tick, and duration. Native projects default to 480 ticks per quarter note; existing files keep their stored timing. Instruments own patch/kit references, gain, pan, program, and bend-range settings; tracks sharing an instrument also share its MIDI channel while retaining separate exact-named MIDI tracks. Synth patch references can be built-in preset names or `.json` paths relative to the `.evil` file. `project render` uses the embedded assignments to produce a stereo PCM16 WAV, optionally cropped to an exact tick range with pre-roll. `render-stems` and `render-parts` solo and temporarily unmute each selected track or timeline part, retain its routing/master processing and full tail, and use collision-safe one-based filenames. Their CLI selectors remain zero-based. Noteless batch items are skipped; explicit empty selections fail. Batches are non-transactional, and separately saturated stems need not sum to the full mix. `project export-midi` preserves exact track names, meter, tempo changes, gain/pan CCs, program mappings, and atomic output. Muted tracks remain as empty named tracks so engine track indices stay stable. An imported song's `source_midi` is stored in the project so Save can update the original MIDI file even after reopening it. If the MIDI file moves, **Relink MIDI** selects its new location without breaking future save-back.

Add a drum track with the built-in Maxwell kit, the original measured Chiptune recreation (`--kit chiptune`), or a relative `.evilkit` bank. The default MIDI mapping assigns notes 36–51 to the 16 pads; change it with `--base-note`. Notes outside the mapping or targeting empty pads are reported as skipped during rendering. The Chiptune kit is deterministic original synthesis informed by per-pad measurements, not embedded factory audio; see `docs/chiptune.md`.

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-drum-track song.evil Drums --kit maxwell
cargo run --manifest-path compat/Cargo.toml -p oracle -- project add-note song.evil 2 36 0 480 --velocity 110
```

Edit notes and tracks, or convert MIDI into a native project:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- project notes song.evil 0
cargo run --manifest-path compat/Cargo.toml -p oracle -- project move-note song.evil 0 0 --pitch 72 --start-tick 64 --duration-ticks 128
cargo run --manifest-path compat/Cargo.toml -p oracle -- project remove-note song.evil 0 0
cargo run --manifest-path compat/Cargo.toml -p oracle -- project mute song.evil 1
cargo run --manifest-path compat/Cargo.toml -p oracle -- project unmute song.evil 1
cargo run --manifest-path compat/Cargo.toml -p oracle -- project import-midi input.mid imported.evil --name "Imported Song"
```

Invalid edits leave the document unchanged. MIDI source tracks are remapped to contiguous native track indices and default to the `lead` patch for customization. Optional tempo changes after the initial tempo are preserved in the native JSON. Signed pitch-bend events, RPN 0 bend ranges, channel/polyphonic aftertouch, CC 1 modulation depth, and CC 11 expression are preserved through import, `.evil`, synthesis, and export. Other controller events are preserved and exported but explicitly counted as not rendered.

## Automation and AI agents

The non-interactive `oracle` CLI provides JSON inspection and render reports for scripting and AI agents. See `docs/cli-ai.md` for supported commands, error behavior, and a full-bar A17 test workflow.

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- project show song.evil --json
cargo run --manifest-path compat/Cargo.toml -p oracle -- midi inspect input.mid --json
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth list --json
cargo run --manifest-path compat/Cargo.toml -p oracle -- project render song.evil song.wav --json
```

## MIDI Evil desktop

Launch the native visual workspace:

```bash
cargo run -p midi-evil
```

MIDI Evil creates, opens, edits, saves, previews, and renders `.evil` projects. Its native dependency chain contains only `evil-*` crates; Standard MIDI loading lives in `evil-imports`, MIDI writing lives in `evil-exports`, and the optional `evil-porting` compatibility tooling remains separate. It includes a track panel with add/delete/duplicate/reorder and instrument/gain/pan/mute controls, `+` for synth tracks and `+KIT` for Maxwell drum tracks, a full-MIDI piano roll, a note inspector, and editable automation lanes for pitch bend, channel/polyphonic aftertouch, and arbitrary CC/RPN events. Add or drag automation points, edit exact ticks and values, and delete points with the same undo/redo history as notes. In a controller lane, choose a 1/4–1/64 ramp grid and use **Ramp to next** to replace the selected segment with linearly interpolated MIDI CC events for both native rendering and export. CC 1 modulation and CC 11 expression are rendered; unsupported CC/RPN events remain preserved for MIDI export and are explicitly identified as not rendered in the lane, preview, and WAV-export status.

The Dr. Maxwell workspace uses charcoal/plum panels, ivory displays, red accents,
and original Aseprite control icons. It has six workspaces:

- **Song** is a shared-pattern arranger. Add and drag parts, crop or rename them, duplicate linked parts or the selected loop, detach a part with **Make unique**, merge it into the track timeline, or double-click it to edit its pattern.
- **Piano Roll** edits either the selected shared pattern or the track's unique timeline, with a fixed keyboard and bar ruler, note labels, quantize/transpose/copy/paste/cleanup operations, and an **Automation** toggle. Controller lanes edit the same pattern/timeline context, and bar lengths follow the imported time signature.
- **Mixer** provides track gain faders, pan, mute, master gain, and output sample rate.
- **Synth** opens Evil Synth with grouped voice/filter and modulation/effects panels and an amplitude-envelope display.
- **Keyboard** opens the selected track's instrument in Maxwell's performance lab: three octaves of playable keys, a spring-return pitch wheel, and tone/delay XY pads.
- **Pads** opens Evil Beats: a 4×4 performance grid with an original Maxwell drumkit, WAV loading, pad editing, stereo output controls, and held-pad repeat at the song tempo.

The **Project** menu contains file/import/save commands; **Export** offers MIDI, full-song and transport-section WAV, soloed track-stem directories, and arrangement-part directories.
**Play / Stop** controls the rendered preview and shows elapsed playback time. **REC** starts a repeating take into the selected track or arrangement part; **Stop / TAKE** commits the accumulated notes and controllers as one undoable edit. An independent recording clock captures the first note in an empty song or silent loop even when no audio preview can be rendered. Structural track/arranger edits are locked until the take is committed so its destination cannot change. Recording can use the on-screen keyboard, pads, or a desktop MIDI input selected beside the transport. Held-pad audio repeats are captured as individual hits at their sample-clocked onset times. **Quantize recording to 1/16** is optional. Loop recording overdubs the selected shared pattern and therefore every part linked to it; use **Make unique** first for an independent variation. **Start bar** chooses where playback begins; without repeat, playback continues through the song and release/effect tail. **Loop end** and **Repeat section** repeat an exact bar range for composing. Click a bar in the Song ruler to select it, and use the numeric controls for multi-bar ranges. A green playhead follows the audio clock, wraps at the repeat boundary, and remains correct through tempo changes. Changing the range stops the previous preview. The track rack and inspectors scroll independently in smaller windows.

On **Keyboard**, play with the mouse (including glissando) or use **A W S E D F T G Y H U J K O L P ; '** for computer-keyboard chords. **Z / X** shifts octaves; velocity and sustain controls shape the performance. **Escape / All notes off** silences held notes and delay tails. The tone pad controls filter cutoff/resonance, and the space pad controls delay time/wet mix. XY changes share the current synth draft; use **Edit / save synth patch → Save As** to keep them. Live playing auditions the instrument immediately; arm **REC** to capture the performance into the selected timeline or pattern.

Ctrl/Cmd+N/O/S handles new/open/save, Ctrl/Cmd+Z/Y handles undo/redo, Space toggles preview, and New/Open/Import confirms before discarding unsaved work. Click empty space to add a note, drag notes to move them, drag the right edge to resize, and press Delete to remove the selected note. **Render WAV** uses the same shared `evil-render` engine as the CLI and produces a stereo mix with the project's embedded patches and mix settings; **Section WAV** uses the current transport range with exact pre-roll/cropping. **Track stems** and **Arrangement parts** queue soloed PCM16 WAVs in a chosen directory, writing each completed file before starting the next. Preview and WAV rendering run on worker threads; New/Open/Import cancels stale jobs. **Export MIDI** writes an engine-compatible `.mid`, while **Save** also updates the original MIDI file for imported songs.

Launch it directly with a MIDI file and optional engine hints:

```text
midi-evil [--project <dir>] [--track <name>] <file.mid>
```

A second invocation forwards its file/track/project options to the running instance and focuses it. `--project` supplies dialog and relative-path hints; `--track` selects the exact track name without changing it.

**Import MIDI** converts a Standard MIDI File into an editable native document. **Play** renders the current project and plays it through the default audio device; **Stop** ends playback. Opening, replacing, or previewing another project stops the previous preview. These controls use the same note and tempo-map conversion as the CLI.

A public-domain example is included at `examples/public-domain/bach-invention-02/bach-invention-02.mid`. It is Bach's Invention No. 2 in C minor, BWV 773, downloaded from the Mutopia Project, whose entry marks it “Public Domain / CC: No rights reserved.” See `examples/public-domain/bach-invention-02/SOURCE.md` for the source URL, dedication text, and checksum.

Dr. Maxwell is the app logo and mascot, with a dedicated MIDI Evil icon and full splash selected by the user from the Insiculous branding assets. The icon appears in the toolbar, and the splash appears in the welcome/About window. Editable masters and runtime PNGs are stored together:

- `crates/midi-evil/assets/midi-evil-icon.aseprite`
- `crates/midi-evil/assets/midi-evil-icon.png`
- `crates/midi-evil/assets/midi-evil-splash.aseprite`
- `crates/midi-evil/assets/midi-evil-splash.png`
- `crates/midi-evil/assets/maxwell-controls.aseprite`
- `crates/midi-evil/assets/maxwell-controls.png`
- `crates/midi-evil/assets/SOURCE.md`

Re-export the app images in Aseprite without using any NanoStudio artwork:

```bash
aseprite -b crates/midi-evil/assets/midi-evil-icon.aseprite \
  --save-as crates/midi-evil/assets/midi-evil-icon.png
aseprite -b crates/midi-evil/assets/midi-evil-splash.aseprite \
  --save-as crates/midi-evil/assets/midi-evil-splash.png
```

## Original synth and sound effects

## Standard MIDI import and rendering

Inspect a `.mid` file and render its notes with a preset or custom patch:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- midi inspect input.mid
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render-midi input.mid lead lead.wav --track 1
```

The importer supports format 0 and format 1 Standard MIDI Files with metrical timing, multiple tracks, overlapping notes, track names, tempo changes, signed pitch-bend events, RPN 0 bend ranges, channel/polyphonic aftertouch, CC 1 modulation depth, and CC 11 expression. Format 2 and SMPTE timing are rejected explicitly. Other MIDI controller events are preserved for export and counted as not rendered.

`init-mix` and `render-mix` also accept `.mid` input in place of a project:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth init-mix input.mid midi-mix.json
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render-mix input.mid midi-mix.json midi-song.wav
```

## Assisted WAV recovery

The native `evil-imports` crate and CLI analyze user-owned WAVs and emit JSON reports and editable `.evil` onset drafts. Reports include duration, active range, peak/RMS, all detected onsets, and three tempo candidates. Drafts quantize onsets into a reviewable guide track; they are not claims that every onset is a drum note or a complete transcription.

```bash
cargo run -p evil-imports -- inspect Dreams.wav
cargo run -p evil-imports -- draft Dreams.wav dreams.evil --bpm 110 --grid 16
```

`--bpm` overrides the top tempo candidate, and `--grid` accepts 1, 2, 4, 8, 16, or 32. The current native projects for **Dreams**, **Foolish Thoughts**, and **The Dagobah Vision** are under `target/recovery/`. Waveform/spectrogram inspection, loop detection, bass/lead pitch tracking, confidence-based editing, patch matching, and A/B rendering remain staged work. Recovered projects are editable approximations, not exact master restores.

## Original synth and sound effects

The `evil-synth` engine generates audio from oscillators, pitch sweeps, ADSR envelopes, filters, modal partials, and seeded noise, with optional user-imported sample oscillators. Native project rendering produces stereo output; the standalone synth also supports mono PCM16 WAV.

### Evil Synth instrument editor

Use **Open Evil Synth** on a track or the **Synth** tab to open the instrument editor. Its controls are grouped into **Voice & Filter**, **Samples**, **Modulation**, and **Insert FX**:

- **Voice & Filter:** 36 basic waveforms, frequency sweeps, detune, pulse width, mix/sync/ring modes, poly/mono/legato voices, unison, glide/accent, modal partials, independent amplitude/filter ADSRs, and LP/BP/HP filters.
- **Samples:** WAV/AIFF import, playback bounds, loops, key zones, and keyboard slicing. The Samples toolbar records the microphone or resamples the transport section into Osc A.
- **Modulation:** two auxiliary envelopes with signed routes, plus up to eight LFO lanes with free or tempo-synced rates and selectable modulation targets.
- **Insert FX:** waveshaper followed by mono/stereo-cross delay. The piano-roll automation controller menu names their six CC lanes (20–25).

A three-octave keyboard stays at the bottom while the controls scroll. Play by mouse, computer keys, or connected MIDI; adjust octave, velocity, and sustain, or use **All notes off / Escape**. Voice edits apply to the next note, and filter/effect edits update live. The separate **Keyboard** page shares the same draft and adds XY pads and a pitch wheel.

**New**, **Open**, **Duplicate**, **Reset**, and **Save As** manage presets. Expand **Preset / Output** for the patch name, gain, pitch tracking, noise seed, and MIDI defaults. **Save As** assigns the custom JSON patch to its track and preserves the relative path and optional General MIDI program in the project.

The built-in **Max Nasty** (`nasty`) preset recreates the character of NanoStudio's A17.Nasty using original synthesis: reference-matched 1.25 oscillator sync and middle-C tuning, independent vibrato, saw-up/saw-down sync-ratio LFO lanes with tempo-map-aware `beats_per_cycle` timing, phase-continuous square-wave LFO4-to-LFO3 rate modulation, calibrated saw-down amplitude articulation, pitch-dependent gain for the original's stronger low notes, a near-open 24 dB key-tracked low-pass filter, and 20% wet stereo-cross delay with reference-calibrated timing and feedback. It uses no copied samples or original NanoStudio audio. The source preset's Hard waveshaper is bypassed and is therefore not needed in the recreation. Mono-only CLI renders fold the delay down to a mono feedback delay; desktop preview and stereo project/mix renders preserve the stereo routing. Full-bar comparison against NanoStudio now closely matches held-note RMS, spectral centroid, and much of the rhythmic envelope, but oscillator/LFO/envelope behavior and the delay tail are still approximate; see `docs/a17-nasty.md` for measurements and remaining differences.

The built-in **Evil-303** (`nano-303`) preset adds original 303-style saw synthesis, resonant filter-envelope modulation, previous-note pitch glide, and velocity accents with filter/gain boosts. Patch JSON exposes `glide_seconds`, `accent_velocity_threshold`, `accent_filter_octaves`, and `accent_gain` for calibration and custom patches.

The built-in `glocken` preset introduces modal synthesis for GlockenBlocks: independently tuned inharmonic partials with separate exponential decay times and pitch tracking. Patch JSON supports up to 16 `modal_partials` with `ratio`, `amplitude`, and `decay_seconds`.

```bash
# List the original SFX and instrument presets
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth list

# Render a preset directly
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render laser laser.wav

# Save an editable patch, customize it, then render the result
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth init laser my-laser.json
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render my-laser.json my-laser.wav
```

Patch JSON includes the sample rate, duration, gain, optional General MIDI `midi_program`, pitch-tracking mode, optional `pitch_gain_per_octave` scaling around middle C, random seed, oscillator settings, optional oscillator sync and unison, envelope, and optional filter, primary LFO with optional `beats_per_cycle` timing, additional modulation lanes, and delay settings. Imported program changes, CC 7 volume, and CC 10 pan are preserved on export; tracks may override the patch's program with their own `midi_program`. Missing fields use documented defaults in the `evil-synth` data model. Rendering the same patch and seed produces identical audio. Repeated tonal notes also reuse a bounded voice cache to avoid redundant oscillator work; noisy patches retain per-note variation.

Render project notes with a patch, optionally selecting one source track:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render-project MySong.prj lead lead.wav --track 1
```

Omit `--track` to render every track with the same patch. Overlapping notes are independent polyphonic voices. With `pitch_tracking` enabled, oscillator frequencies follow MIDI pitch relative to note 69; disable it for fixed-pitch drums and effects. Note velocity controls voice gain.

### Evil Beats

The native `evil-beats` crate provides original sample-pad drum voices with semitone tuning, velocity-sensitive gain, one-shot or timed-gate playback, non-destructive trim ranges, forward/reverse and gated-loop modes, ADSR release behavior, and 32-voice oldest-first allocation with two choke groups. Independent per-voice LP/BP/HP filters and three filtered, panned output buses receive post-pad-gain/pre-pan sends. Offline and live playback share the same cursor, voice allocator, filter processors, and bus graph. It has no Oracle dependencies. It also provides validated 16-pad banks, mono/stereo WAV loading, a deterministic synthesized starter kit, a stereo player with sample-clocked repeat, and deterministic offline stereo kit rendering shared by native projects.

Open **Pads** for Dr. Maxwell's rhythm lab. Pads are numbered from the bottom left; play them with the mouse or **Z X C V / A S D F / Q W E R / 1 2 3 4**. Hold pads with **Repeat held pads** enabled for rates from 1/1 to 1/32 at the current song tempo. **Silence all / Escape**, leaving the page, and losing window focus stop playback/repeat.

Right-click a pad or choose **Edit Pad** to rename it, load a mono/stereo PCM or float WAV (8–192 kHz, up to 30 seconds), adjust tuning/envelopes, trim the playback frame range, crop destructively to that trim, normalize source peaks with one-step sample-edit undo, choose forward/reverse or gated-loop playback, assign an independent voice or choke group 1/2, or export the processed pad as WAV. Stereo imports are downmixed to mono; **Output** gives each pad gain, stereo pan, mute, and three bus sends, plus bank master gain and per-bus filter/gain/pan/mute controls. **New bank** starts empty; **Lab kit** restores the original synthesized sounds.

**Save bank** (or Ctrl/Cmd+S while on Pads) writes a portable `.evilkit` JSON file with embedded audio and pad settings. **Open bank** restores it. Use **+KIT** in the track rack to add the built-in Maxwell kit to the song, or assign a `.evilkit` path in a track's instrument settings. Opening Pads on a drum track loads that instrument's bank; saving automatically updates the original track's kit reference, relative to the project when possible, so subsequent native CLI/GUI rendering uses the edited bank. Save the project as well to persist the updated reference. Arm **REC** to record direct and held-repeat pad hits into the selected drum track or pattern. Bus filters preserve their tails through offline gaps and after the final hit, with a bounded quiet-tail limit.

### Stereo mixes

Native `.evil` instruments also support four ordered channel inserts and two post-channel sends into project-wide effect buses. The shared offline/live graph currently includes waveshaper, second-based and beat-synced delay, bitcrush, three-band EQ, stereo-linked compression, chorus, flanger, and stereo reverb inserts with deterministic bounded tails; older projects default to no extra inserts and zero sends. Beat-synced delays follow the complete project tempo map, while CC 23 time automation overrides beat sync with second-based timing. Channel settings are validated and preserved in project JSON, and the same routing applies to synth and drum tracks.

Generate a per-track mix configuration, edit the assignments, and render one stereo file:

```bash
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth init-mix MySong.prj mix.json
cargo run --manifest-path compat/Cargo.toml -p oracle -- synth render-mix MySong.prj mix.json song.wav
```

The open JSON mix format has a shared `sample_rate`, `master_gain`, and a `tracks` array:

```json
{
  "sample_rate": 48000,
  "master_gain": 0.8,
  "tracks": [
    { "track": 1, "patch": "lead.json", "gain": 0.8, "pan": -0.4 },
    { "track": 2, "patch": "bass", "gain": 0.9, "pan": 0.0 }
  ]
}
```

Patch names can be built-in presets or `.json` paths relative to the mix file. Gain ranges from 0 to 1; pan ranges from -1 (left) to 1 (right), using equal-power panning. The final stereo mix is soft-limited and written as PCM16 WAV.

## Design

- Keep code DRY, simple, and organized around focused responsibilities.
- Keep NanoStudio compatibility without making proprietary formats the only workflow.
- Prefer standard MIDI for performance data and open audio formats for interchange.
- Build original synthesized instruments and sound effects.
- Create UI artwork as original Aseprite masters with app-ready exports. Do not use original NanoStudio artwork in the rendered UI.

## Verify

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path compat/Cargo.toml --workspace
cargo clippy --manifest-path compat/Cargo.toml --workspace --all-targets -- -D warnings
```

Run the tracked, hardware-independent native parity audit from a clean checkout:

```bash
tools/audit-native.sh
```

It checks formatting, all native tests, strict Clippy, the focused `native_parity` end-to-end workflows, and a release desktop build. The focused workflows cover public-file MIDI edit/save/export/reimport, portable 16-pad WAV kits and dense hits, Chiptune project rendering, and deterministic full/section/scoped WAV artifacts. Keep these native checks distinct from the private compatibility/Oracle suite. Reference-calibration commands and their remaining approximation limits are documented in `docs/a17-nasty.md`, `docs/nano303.md`, `docs/glocken.md`, and `docs/chiptune.md`.

The virtual-microphone test is ignored by default rather than silently passing. On Linux, run it only with the tracked ALSA null input, which captures silence without touching a physical microphone:

```bash
ALSA_CONFIG_PATH="$PWD/tools/alsa-null.conf" \
MIDI_EVIL_VIRTUAL_AUDIO_SMOKE=1 \
cargo test -p midi-evil virtual_microphone_records_and_embeds_in_osc_a -- --ignored --nocapture
```

Interactive and physical-device release gates are separate from headless parity results: exercise direct arranger drag and editing during playback; piano-roll add/move/resize/delete and automation gestures in shared and unique contexts; physical MIDI note/velocity, bend, aftertouch, CC, disconnect/reconnect and loop recording; and real audio-output/microphone listening checks. Packaging still requires a clean public-clone build, desktop smoke, and package-content inspection. No physical MIDI device or real microphone is exercised by the automated audit.
