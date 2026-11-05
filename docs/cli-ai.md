# Agent-friendly CLI

`oracle` is the non-interactive MIDI Evil CLI for local compatibility work. Its crates and `compat/` workspace are intentionally not included in the public native repository. Use it for native project creation/editing, MIDI conversion, synthesis, inspection, and WAV rendering without launching the desktop app.

## Invocation and errors

```bash
cargo build --manifest-path compat/Cargo.toml -p oracle
compat/target/debug/oracle --help
compat/target/debug/oracle project --help
compat/target/debug/oracle project render --help
```

The examples below use `oracle` to mean `compat/target/debug/oracle`.

Successful commands exit 0. Invalid input and I/O failures print an error to stderr and exit nonzero. Native project edits validate before saving; failed edits do not partially modify the project. Use `--json` where available instead of parsing human-readable output. JSON goes to stdout; warnings and errors go to stderr.

## JSON commands

| Command | JSON result |
| --- | --- |
| `oracle inspect PROJECT --json` | NanoStudio project inspection |
| `oracle project show SONG.evil --json` | Complete native project document, including notes and automation |
| `oracle project notes SONG.evil TRACK --json` | One native track with notes and automation |
| `oracle midi inspect INPUT.mid --json` | Timing, duration, track, note, and automation counts |
| `oracle synth list --json` | Built-in preset name array |
| `oracle synth render PATCH OUT.wav --json` | Mono render report with patch, sample count, and sample rate |
| `oracle project render SONG.evil OUT.wav --json` | Stereo full-song or `--start-tick`/`--end-tick` section report with frames and all render-warning counts |
| `oracle project render-stems SONG.evil OUT_DIR --json` | One report per soloed track WAV, plus noteless-item skip count |
| `oracle project render-parts SONG.evil OUT_DIR --json` | One report per soloed arrangement-part WAV, plus noteless-item skip count |

Other JSON workflows already available:

- `oracle synth init PRESET patch.json` writes an editable synth patch.
- `oracle synth init-mix INPUT mix.json` writes editable per-track patch/gain/pan assignments.
- Native `.evil` files are versioned JSON and can be inspected directly when the full document is needed.

## Create and render a full-bar A17 test

Native projects default to 480 ticks per quarter, so 1,920 ticks is one complete 4/4 bar. At 120 BPM this holds the note for two seconds; the render also includes the release and delay tail.

```bash
oracle project new "A17 Whole Bar" a17.evil --tempo 120
oracle project add-track a17.evil Nasty --patch nasty --gain 1
oracle project add-note a17.evil 0 60 0 1920 --velocity 110
oracle project show a17.evil --json
oracle project render a17.evil a17.wav --json
```

The built-in `nasty` preset is an original approximation, not a claim of sample-identical NanoStudio audio. Use a NanoStudio reference render when evaluating compatibility.

## Create and render a drum track

```bash
oracle project new "Kit Test" kit.evil --tempo 120
oracle project add-drum-track kit.evil Drums --kit maxwell --base-note 36
oracle project add-note kit.evil 0 36 0 480 --velocity 110
oracle project render kit.evil kit.wav --json
oracle project export-midi kit.evil kit.mid
```

Use a relative `.evilkit` path instead of `maxwell` for a saved bank, or select the original `chiptune` recreation with `--kit chiptune`. The default mapping assigns notes 36–51 to the 16 pads. `skipped_drum_notes` reports notes outside that mapping or targeting empty pads; those notes remain preserved in the project and MIDI export.

## Render a section, track stems, or arrangement parts

```bash
# Exact section: pre-roll preserves sustained notes, modulation and effect state.
oracle project render song.evil section.wav --start-tick 480 --end-tick 960 --json

# Every note-containing track, or one zero-based original track index.
oracle project render-stems song.evil exports/stems --json
oracle project render-stems song.evil exports/lead --track 0 --json

# Every note-containing timeline part, or one part on a selected track.
oracle project render-parts song.evil exports/parts --json
oracle project render-parts song.evil exports/verse --track 0 --part 2 --json
```

Stem and part exports temporarily solo and unmute their selection without changing the project. They retain instrument/master processing and only the selected track's send contribution. Part audio starts at frame zero at the part's timeline start and includes its complete note/release/effect tail. File names use one-based original indices even though CLI selectors are zero-based, so duplicate track/pattern names cannot collide. Batch exports skip noteless items; an explicitly selected empty item or a batch with no exportable items is an error and does not create an output directory. Batches are sequential and non-transactional: earlier files remain if a later render fails. Since master saturation is applied independently to each file, summing stems is not guaranteed to reproduce the full mix sample-for-sample.

## Agent workflow

1. Discover arguments with the relevant `--help`; do not guess tick timing or track indices.
2. Inspect the source with a JSON command before editing.
3. Create or modify the project with CLI commands, keeping patch paths relative to the `.evil` file.
4. Render with `--json` and check the returned counts and output path.
5. Verify the generated WAV or MIDI with the appropriate inspection command or comparison tool. A successful command alone does not establish audio or format equivalence.
