# MIDI Evil

A Rust music workstation with NanoStudio compatibility as a validation oracle and an open native workflow. The long-term sound workflow is original synthesis and sound-effect design rather than reliance on a sample library.

## Run

```bash
cargo run -p oracle -- --help
```

Inspect the development fixture:

```bash
cargo run -p oracle -- inspect target/fixtures/Minimal.nsp
```

## Publish native crates

The `oracle`, `oracle-format`, and `oracle-midi` packages are marked `publish = false`. The desktop and its native libraries can be published without uploading the compatibility tooling. Publish the native crates in dependency order, allowing each published crate to become available before packaging the next:

```bash
cargo publish -p evil-performance
cargo publish -p evil-synth
cargo publish -p evil-project
cargo publish -p evil-render
cargo publish -p evil-midi
cargo publish -p midi-evil
```

Use `cargo package -p midi-evil --allow-dirty --list` to inspect the desktop package contents. Full registry verification requires the preceding native crates to have been published; it does not require any Oracle package.

## Native `.evil` projects

A `.evil` file is a versioned, human-readable JSON document containing the song name, tempo, meter, direct timeline notes, track patch references, gain, pan, mute state, and master audio settings. Its format identifier is `midi-evil-project`; it does not contain NanoStudio pattern records or proprietary binary data. Legacy `.rmp` files and `rusty-music-project` JSON remain readable and migrate to the new format when saved.

Create and render a song from scratch:

```bash
cargo run -p oracle -- project new "My Song" song.evil --tempo 120
cargo run -p oracle -- project add-track song.evil Lead --patch lead --gain 0.8 --pan -0.4
cargo run -p oracle -- project add-track song.evil Bass --patch bass --gain 0.9 --pan 0.2
cargo run -p oracle -- project add-note song.evil 0 69 0 128 --velocity 110
cargo run -p oracle -- project add-note song.evil 1 45 0 256 --velocity 120
cargo run -p oracle -- project show song.evil
cargo run -p oracle -- project render song.evil song.wav
cargo run -p oracle -- project export-midi song.evil song.mid
```

Track indices are returned by `add-track` and shown by `show`. Note arguments are track, MIDI pitch, start tick, and duration. Native projects default to 480 ticks per quarter note; existing files keep their stored timing. Patch references can be built-in preset names or `.json` paths relative to the `.evil` file. `project render` uses the embedded assignments to produce a stereo PCM16 WAV. `project export-midi` preserves exact track names, meter, tempo changes, gain/pan CCs, program mappings, and atomic output. Muted tracks remain as empty named tracks so engine track indices stay stable. An imported song's `source_midi` is stored in the project so Save can update the original MIDI file even after reopening it. If the MIDI file moves, **Relink MIDI** selects its new location without breaking future save-back.

Edit notes and tracks, or convert MIDI into a native project:

```bash
cargo run -p oracle -- project notes song.evil 0
cargo run -p oracle -- project move-note song.evil 0 0 --pitch 72 --start-tick 64 --duration-ticks 128
cargo run -p oracle -- project remove-note song.evil 0 0
cargo run -p oracle -- project mute song.evil 1
cargo run -p oracle -- project unmute song.evil 1
cargo run -p oracle -- project import-midi input.mid imported.evil --name "Imported Song"
```

Invalid edits leave the document unchanged. MIDI source tracks are remapped to contiguous native track indices and default to the `lead` patch for customization. Optional tempo changes after the initial tempo are preserved in the native JSON. Signed pitch-bend events, RPN 0 bend ranges, channel/polyphonic aftertouch, CC 1 modulation depth, and CC 11 expression are preserved through import, `.evil`, synthesis, and export. Other controller events are preserved and exported but explicitly counted as not rendered.

## Automation and AI agents

The non-interactive `oracle` CLI provides JSON inspection and render reports for scripting and AI agents. See `docs/cli-ai.md` for supported commands, error behavior, and a full-bar A17 test workflow.

```bash
cargo run -p oracle -- project show song.evil --json
cargo run -p oracle -- midi inspect input.mid --json
cargo run -p oracle -- synth list --json
cargo run -p oracle -- project render song.evil song.wav --json
```

## MIDI Evil desktop

Launch the native visual workspace:

```bash
cargo run -p midi-evil
```

MIDI Evil creates, opens, edits, saves, previews, and renders `.evil` projects. Its native dependency chain contains only `evil-*` crates; Standard MIDI import/export lives in `evil-midi`, while the optional `oracle-*` compatibility tooling remains separate. It includes a track panel with add/delete/duplicate/reorder and patch/gain/pan/mute controls, a full-MIDI piano roll, a note inspector, and editable automation lanes for pitch bend, channel/polyphonic aftertouch, and arbitrary CC/RPN events. Add or drag automation points, edit exact ticks and values, and delete points with the same undo/redo history as notes. CC 1 modulation and CC 11 expression are rendered; unsupported CC/RPN events remain preserved for MIDI export and are explicitly identified as not rendered in the lane, preview, and WAV-export status.

The Dr. Maxwell workspace uses charcoal/plum panels, ivory displays, red accents,
and original Aseprite control icons. Its four tabs share the same project:

- **Song** fits the complete arrangement into a track overview. Double-click a track to edit its notes.
- **Piano Roll** has a fixed keyboard and bar ruler, note labels, and an **Automation** toggle. Bar lengths follow the imported time signature.
- **Mixer** provides track gain faders, pan, mute, master gain, and output sample rate.
- **Synth** opens Evil Synth with grouped voice/filter and modulation/effects panels and an amplitude-envelope display.

The **Project** menu contains file/import/save commands; **Export** offers MIDI and WAV.
**Play / Stop** controls the rendered preview and shows elapsed playback time. The
track rack and inspectors scroll independently in smaller windows.

Ctrl/Cmd+N/O/S handles new/open/save, Ctrl/Cmd+Z/Y handles undo/redo, Space toggles preview, and New/Open/Import confirms before discarding unsaved work. Click empty space to add a note, drag notes to move them, drag the right edge to resize, and press Delete to remove the selected note. **Render WAV** uses the same shared `evil-render` engine as the CLI and produces a stereo mix with the project's embedded patches and mix settings. Preview and WAV rendering run on a worker thread; New/Open/Import cancels stale jobs. **Export MIDI** writes an engine-compatible `.mid`, while **Save** also updates the original MIDI file for imported songs.

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

## NanoStudio compatibility commands

Create a blank project and add instrument tracks:

```bash
cargo run -p oracle -- song create MySong.prj --name "My Song"
cargo run -p oracle -- song add-track MySong.prj 1
cargo run -p oracle -- song set-channel MySong.prj 0 Channel00 --volume 0.8 --pan 0.5
cargo run -p oracle -- song set-effect MySong.prj 0 Channel00 SLOT2 false

# Replace a copied sound template's sequence with MIDI note tracks
cargo run -p oracle -- song import-midi MySong.prj input.mid
```

`song import-midi` retains the project's instruments, mixer settings, banks, and assets. It assigns MIDI note tracks to non-mixer instruments in project order, converts timing to NanoStudio's 256 ticks per quarter, and writes one full-length pattern and timeline part per track. Only the initial tempo is applied; later tempo changes and automation without known NanoStudio mappings are reported as skipped. Use a copied template because this command replaces the existing sequence.

```bash
# Unpack into a new directory (the destination must not exist)
cargo run -p oracle -- unpack input.nsp MySong.prj

# Edit global song settings
cargo run -p oracle -- edit MySong.prj --tempo 128 --swing 0.5 --beats-per-bar 4

# Repack the project
cargo run -p oracle -- pack MySong.prj MySong.nsp
```

## Pattern editing

The sequencer uses 256 ticks per quarter note. New patterns default to one bar.

```bash
# Add pitch 60 at tick 0 with duration 128
cargo run -p oracle -- pattern add-note MySong.prj 2000 60 0 128 --velocity 100

cargo run -p oracle -- pattern list MySong.prj 2000
cargo run -p oracle -- pattern move-note MySong.prj 2000 0 --note 64 --start-tick 64
cargo run -p oracle -- pattern remove MySong.prj 2000 0
```

## Arrangement editing

Timeline parts may repeat a pattern for any duration; their lengths do not need to match the source pattern.

```bash
cargo run -p oracle -- song list MySong.prj
cargo run -p oracle -- song add MySong.prj 1 2000 0 512
cargo run -p oracle -- song move MySong.prj 1 4 --start-tick 4096 --length-ticks 1024
cargo run -p oracle -- song remove MySong.prj 1 4
```

## Standard MIDI export

Export either an unpacked project or an `.nsp` archive:

```bash
cargo run -p oracle -- midi export MySong.prj MySong.mid
```

The format 1 MIDI file includes tempo, 4/4-based meter, track names, and notes expanded from repeated timeline patterns. Timing remains at 256 ticks per quarter note. Notes starting within a timeline part retain their full durations, even when they extend past the part boundary. Swing does not alter exported MIDI ticks, matching NanoStudio's own exporter. See `docs/nanostudio-compatibility.md` for the reference-backed edge cases and remaining compatibility limits.

MIDI does not contain instrument audio. Known controller records are translated to signed pitch bend and instrument-specific CC mappings (Eden/mixer CC70–73 and TRG CC75–77/80–82). Initial presets and observed timeline-part preset changes are exported as CC32 bank select plus program change, MIDI channels follow instrument indices, and sequencer tracks sharing an instrument are merged into one MIDI track. Unknown record types are still counted and reported rather than silently dropped. Exact same-tick event ordering, conductor-track naming, and empty-track padding remain separate compatibility work.

## Standard MIDI import and rendering

Inspect a `.mid` file and render its notes with a preset or custom patch:

```bash
cargo run -p oracle -- midi inspect input.mid
cargo run -p oracle -- synth render-midi input.mid lead lead.wav --track 1
```

The importer supports format 0 and format 1 Standard MIDI Files with metrical timing, multiple tracks, overlapping notes, track names, tempo changes, signed pitch-bend events, RPN 0 bend ranges, channel/polyphonic aftertouch, CC 1 modulation depth, and CC 11 expression. Format 2 and SMPTE timing are rejected explicitly. Other MIDI controller events are preserved for export and counted as not rendered.

`init-mix` and `render-mix` also accept `.mid` input in place of a project:

```bash
cargo run -p oracle -- synth init-mix input.mid midi-mix.json
cargo run -p oracle -- synth render-mix input.mid midi-mix.json midi-song.wav
```

## Original synth and sound effects

The `evil-synth` engine generates audio from oscillators, pitch sweeps, ADSR envelopes, filters, and seeded noise. It does not use prerecorded instrument samples. Output is mono PCM16 WAV for playback and interchange.

### Evil Synth instrument editor

Use **Open Evil Synth** on a track or the **Synth** tab to open the instrument editor. It supports multiple oscillators, waveform selection (including an optional naive square), frequency sweeps, detune, pulse width, oscillator sync, gain-normalized unison voices, ADSR, resonant filters with envelope modulation and slope/key tracking, and up to eight LFO lanes for pitch/pulse-width/sync-ratio/filter-cutoff/amplitude modulation and LFO-to-LFO rate modulation, with mono or stereo-cross feedback delay. **New**, **Open**, **Duplicate**, **Reset**, and **Save As** manage presets without changing the original file; **Audition** plays the current unsaved patch at middle C without modifying the project. Save a custom JSON patch to assign it to the editor's original track; its relative path and optional General MIDI `midi_program` are preserved in the project.

The built-in `nasty` preset recreates the character of NanoStudio's A17.Nasty using original synthesis: reference-matched 1.25 oscillator sync and middle-C tuning, independent vibrato, saw-up/saw-down sync-ratio LFO lanes, square-wave LFO4-to-LFO3 rate modulation, a near-open 24 dB key-tracked low-pass filter, and 20% wet stereo-cross delay with reference-calibrated timing and feedback. It uses no copied samples or original NanoStudio audio. The source preset's Hard waveshaper is bypassed and is therefore not needed in the recreation. Mono-only CLI renders fold the delay down to a mono feedback delay; desktop preview and stereo project/mix renders preserve the stereo routing. Full-bar comparison against NanoStudio now closely matches held-note RMS, spectral centroid, and the late delay tail, but oscillator/LFO/envelope behavior is still approximate; see `docs/a17-nasty.md` for measurements and remaining differences.

```bash
# List the original SFX and instrument presets
cargo run -p oracle -- synth list

# Render a preset directly
cargo run -p oracle -- synth render laser laser.wav

# Save an editable patch, customize it, then render the result
cargo run -p oracle -- synth init laser my-laser.json
cargo run -p oracle -- synth render my-laser.json my-laser.wav
```

Patch JSON includes the sample rate, duration, gain, optional General MIDI `midi_program`, pitch-tracking mode, random seed, oscillator settings, optional oscillator sync and unison, envelope, and optional filter, primary LFO, additional modulation lanes, and delay settings. Imported program changes, CC 7 volume, and CC 10 pan are preserved on export; tracks may override the patch's program with their own `midi_program`. Missing fields use documented defaults in the `evil-synth` data model. Rendering the same patch and seed produces identical audio. Repeated tonal notes also reuse a bounded voice cache to avoid redundant oscillator work; noisy patches retain per-note variation.

Render project notes with a patch, optionally selecting one source track:

```bash
cargo run -p oracle -- synth render-project MySong.prj lead lead.wav --track 1
```

Omit `--track` to render every track with the same patch. Overlapping notes are independent polyphonic voices. With `pitch_tracking` enabled, oscillator frequencies follow MIDI pitch relative to note 69; disable it for fixed-pitch drums and effects. Note velocity controls voice gain.

### Stereo mixes

Generate a per-track mix configuration, edit the assignments, and render one stereo file:

```bash
cargo run -p oracle -- synth init-mix MySong.prj mix.json
cargo run -p oracle -- synth render-mix MySong.prj mix.json song.wav
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
```
