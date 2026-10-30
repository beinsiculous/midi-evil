# MIDI Evil Archive

Completed tasks and fixes moved out of `ROADMAP.md`. Keep entries concise and include verification notes where useful; do not keep active work here.

## 2026-10-30

- Improved A17.Nasty against the dry and full-bar references with 1.25 base sync, 15.5-cent tuning, a near-open 20 kHz filter, and recalibrated stereo gain. Spectral centroid improved from **3.40 to 4.13 kHz** (reference **4.20**), waveform correlation from **0.0294 to 0.0448**, and 6–7 second tail RMS from **0.000397 to 0.000447** (reference **0.000455**). Added high-note clamping for key-tracked cutoffs. Verified **106 workspace tests**, formatting, and strict Clippy.
- Extracted Standard MIDI import/export into `evil-midi`, removing every Oracle dependency from the desktop crate and its package contents. `oracle-midi` now reuses the shared native implementation for compatibility workflows. Marked all Oracle packages `publish = false`, versioned native path dependencies, and documented the native publish order. Renamed the instrument window **Evil Synth**. Verified **106 workspace tests**, formatting, and strict Clippy.
- Added phase-continuous LFO-rate modulation and improved A17.Nasty's rhythmic behavior using decoded sixteenth-note LFO3 timing and calibrated 8 Hz amplitude articulation. Ten-millisecond envelope correlation improved from **0.0789 to 0.4841**, with held-note RMS **0.1459** and spectral centroid **4.24 kHz** versus NanoStudio **0.1473 / 4.20 kHz**. Added a phase-continuity regression test; verified **107 workspace tests**, formatting, and strict Clippy.
- Captured verified NanoStudio C2/C3/C5 Nasty references and added multi-pitch calibration. Slowed the added amplitude articulation to **4 Hz** and added `pitch_gain_per_octave` with a Nasty value of **0.3**, lifting C2 RMS from **0.143 to 0.211** (reference **0.224**) and matching C2 0–200 Hz energy at **78.52 versus 78.46 dB**. Added pitch-gain regression coverage; verified **110 workspace tests**, formatting, and strict Clippy.
- Added tempo-map-aware `beats_per_cycle` LFO timing through native project, mono, and stereo-mix rendering, with fixed-rate equivalence and mid-render tempo-change tests. Added shared offline/live sync-reset anti-aliasing for square and saw oscillators. Created `evil-beats` with tuned sample pads, velocity, gate/release envelopes, and overlapping-hit rendering, all without Oracle dependencies. Verified **123 workspace tests**, formatting, and strict Clippy.
- Added reliable section playback and repeat for composing. Timeline-range rendering preserves sustained-note/LFO/effect state before exact frame cropping, one-shot playback continues through the tail, and repeat restarts at the section boundary. Added start/end bar controls, clickable Song-ruler sections, stale-preview cancellation, and an audio-clocked playhead with tempo-map inverse conversion. Desktop smoke-tested bar selection and loop wrapping; verified **133 workspace tests**, formatting, and strict Clippy.
- Split the public native workspace from the ignored local compatibility workspace. Fresh clones build only `evil-*` crates with no Oracle sources or lockfile entries, while `compat/Cargo.toml` links private compatibility crates for continued differential work. Removed local absolute asset paths and ignored Python caches. Verified **104 native tests**, **133 compatibility-workspace tests**, strict Clippy on both workspaces, and formatting.
- Retired NanoStudio backporting as an active roadmap goal and prioritized native sound recreation plus assisted recovery of user-owned WAV exports. Added previous-note `glide_seconds`, velocity-driven filter/gain accents, and the original `nano-303` preset with regression coverage and a rendered transition demo. Verified **106 native tests**, **135 compatibility-workspace tests**, strict Clippy on both workspaces, and formatting.
- Created `evil-recovery` for Claude-friendly audio reconstruction: PCM WAV loading, activity/onset detection, tempo candidates, JSON reports, and quantized `.evil` guide projects with `--bpm`/`--grid` overrides. Generated validated onset drafts for **Dreams** (110 BPM, 548 notes), **Foolish Thoughts** (120 BPM, 542 notes), and **The Dagobah Vision** (140 BPM, 655 notes) under `target/recovery/`. Verified **109 native tests**, **135 compatibility-workspace tests**, strict Clippy on both workspaces, and formatting.

## 2026-10-29

- Added LFO-to-LFO rate routing, square LFOs, optional naive-square oscillators, and sync-scoped DC removal through final mono/stereo limiting. Captured stricter A17 dry/modulation controls and added A17's LFO4→LFO3 rate route at calibrated depth 0.25. Held-note RMS is **0.1461** versus NanoStudio **0.1473**, and waveform correlation improved from **0.0038 to 0.0294**; filter/LFO shape, envelope, and tail work remains open. Verified **105 workspace tests**, formatting, and strict Clippy.
- Added agent-friendly JSON output for native project show/notes/render, MIDI inspect, preset list, and mono synth render, plus `docs/cli-ai.md` with non-interactive workflows and a full-bar A17 recipe. Validated all six JSON response types and the `oracle` test/strict-Clippy suite.
- Captured A17.Nasty in NanoStudio for a full 4/4 bar with **Render tail**, isolated from master effects. Corrected resonant 6 dB slope-unit handling and middle-C filter tracking, then calibrated native gain, cutoff, delay time, and feedback. Held-note RMS improved from **0.541 to 0.146** (reference **0.147**) and spectral centroid from **1.75 to 3.93 kHz** (reference **4.20 kHz**); exact waveform/filter/envelope/tail equivalence remains open. Measurements and provenance are in `docs/a17-nasty.md`. Verified **103 workspace tests** and formatting.
- Merged NanoStudio sequencer tracks that share an instrument into one MIDI track while retaining every source track's bank/program initialization. Verified matching non-empty track grouping and order plus all **7,572 notes** and **1,980 automation/setup events** across the five official exports; conductor naming, empty-track padding, and exact same-tick ordering remain open. Verified **102 workspace tests**, formatting, and strict Clippy.

## 2026-10-28

- Added five local NanoStudio 1.42 demo projects and application-generated MIDI references with documented provenance and checksums.
- Corrected NanoStudio conversion from 128 to **256 ticks per quarter**, preserved full note durations beyond timeline-part boundaries, and matched exact duplicate handling at repeat boundaries.
- Verified all **7,572 notes / 15,144 note events** against NanoStudio's DnB, Jazz Lounge, Laid Back, Minimal, and Rock On exports with zero note-event differences; controller, program, channel, meta-event, and audio equivalence remain separate work.
- Regenerated the local Bach/Eden audition variants as `*-256` projects with unchanged XML/banks/assets and verified **634 notes and 101.25 seconds** after round trip. Verified **95 workspace tests**, formatting, and strict Clippy.
- Decoded NanoStudio pitch bend as big-endian signed 16-bit divided by four, mapped Eden/mixer automation to CC70–73 and TRG automation to CC75–77/80–82, and handled the observed `0xff` CC value as zero.
- Verified all **1,896 mapped automation events** (1,537 CC and 359 pitch bend) against the five official exports with zero value differences; bank/program initialization and exact same-tick ordering remain documented follow-ups. Verified **97 workspace tests**, formatting, and strict Clippy.
- Decoded Eden `AM SingleI` initial programs and timeline-part preset flags, including bank 7/0 selection; assigned compatibility MIDI channels by instrument index and rejected unrepresentable programs explicitly.
- Matched all **1,980 automation/setup events** (1,579 CC, 359 pitch bend, 42 program changes) and all **15,144 note events** by tick, value, and channel across the five official exports. Exact same-tick ordering and physical instrument-track merging remain open. Verified **101 workspace tests**, formatting, and strict Clippy.

## 2026-10-27

- Added `song import-midi` for replacing a NanoStudio sequence with MIDI note tracks while retaining template instruments, banks, mixer settings, and assets; added atomic-on-validation-failure conversion and serialization tests.
- Corrected NanoStudio performance conversion to its actual 128-tick quarter-note timebase. Verified the Bach fixture round trip at **634 notes, two tracks, and 101.25 seconds**, and generated local baseline plus one-float `Eden.bnk` differential variants for NanoStudio audition.

- Added editable pitch-bend, channel/polyphonic-aftertouch, and CC/RPN lanes with snapped point creation/dragging, numeric editing, deletion, shared zoom/timeline, undo/redo, and explicit unsupported-render status.
- Verified **90 workspace tests**, strict Clippy, formatting, live point creation/dragging, and the unsupported CC2 editor warning.

- Added Patch Lab New/Open/Duplicate/Reset/Save As actions, original-track assignment, reset points, scrollable controls, and worker-thread middle-C auditioning of unsaved patches.
- Added pulse-width LFOs, selectable filter slope/key tracking, feedback delay, and full JSON/audio regression coverage.
- Recreated NanoStudio A17.Nasty as the built-in `nasty` preset from the installed factory bank's actual settings, using original oscillator-sync synthesis and per-track stereo-cross delay rather than copied audio.
- Added stable resonant filters that preserve the legacy zero-resonance path, plus gain-normalized unison with up to eight symmetrically detuned voices.
- Added eight-lane LFO editing with pitch, pulse-width, sync-ratio, cutoff, and amplitude destinations, new waveforms, and backward-compatible single-LFO JSON.
- Added signed pitch-bend import, native JSON storage, per-track ranges, tempo-mapped synthesis through note releases, and standard MIDI export; aftertouch and unknown controllers remain explicitly reported.
- Added channel and polyphonic aftertouch import, native JSON storage, tempo-mapped per-voice amplitude, and standard MIDI export with note/channel scoping tests.
- Added RPN 0 bend ranges, CC 1 modulation depth, CC 11 expression, and preservation/export of unsupported CCs with explicit not-rendered reporting.
- Verified **84 workspace tests**, strict Clippy, formatting, and mono/stereo Nasty WAV renders with feedback-delay tails.
- Corrected native MIDI export to keep muted tracks as empty named tracks, preserving engine track indices and channel allocation; covered with export tests.
- Validated exported Bach, asteroids, and frogger MIDI files through the engine's `SongData::from_smf` loader with no unsupported events or overlap warnings.
- Verified the Bach fixture pair through the engine's parsed track model: exactly one track differs, `one` → `one-renamed`.
- Verified same-pitch overlap export through the engine's overlap report and added an independent-voice synthesis test.
- Ran the engine's `music_midi` suite: **133 tests passed**.
- Added track deletion, duplication, and reordering with selection repair and model tests.
- Added Ctrl/Cmd+N/O/S shortcuts and Space preview/stop, with text-input guards.
- Added unsaved-change confirmation for New, Open, and MIDI import.
- Added Ctrl/Cmd+Z/Y undo/redo with centralized project snapshots, selection repair, and reset-on-new/open/import; also added Ctrl/Cmd+Shift+Z redo.
- Added right-edge note resizing with grid snapping and minimum duration, sharing the same undo history as note moves.
- Added worker-thread preview and WAV rendering with cancellation and stale-job protection, so large projects do not block the editor.
- Preserved General MIDI program changes, CC 7 volume, and CC 10 pan through MIDI import, `.evil`, and native export; added per-track `midi_program` override.
- Added a bounded tonal voice cache for repeated pitch/duration/velocity combinations, reducing repeated oscillator work while preserving per-note seeded noise and identical output.
- Added blank NanoStudio project creation with a Mixer and Eden starter instrument, sequence generation, and `song add-track` with `TrackMap`-preserving XML serialization.
- Added **Relink MIDI** for projects whose original MIDI file moved, preserving the source path in `.evil` and subsequent save-back behavior.
- Added an Eden-like Patch Lab with multi-oscillator editing, detune, pulse width, ADSR, filter-envelope modulation, LFO pitch modulation, and JSON patch assignment.
- Added NanoStudio channel volume/pan and insert-effect enable/disable editing with XML-preserving saves.
- Renamed compatibility packages to `oracle`, `oracle-format`, and `oracle-midi`; verified the renamed CLI against `Minimal.nsp`.
- Verified **61 workspace tests** and strict Clippy.

## 2026-10-26

- Added native `.evil` JSON projects with direct notes, tracks, patches, gain, pan, mute, and master settings.
- Preserved legacy `.rmp` / `rusty-music-project` loading and migration to `.evil` / `midi-evil-project`.
- Added atomic project saves and validated native note/track editing.
- Added Standard MIDI import/export, tempo maps, overlapping-note handling, and controller reporting.
- Added original oscillators, pitch sweeps, ADSR envelopes, filters, seeded noise, polyphony, stereo mixing, and WAV rendering.
- Extracted shared `evil-performance` and `evil-render` crates so CLI and GUI use the same note conversion and rendering paths.
- Added the `midi-evil` desktop app with track controls, a full-MIDI piano roll, note inspector, MIDI import, and Preview/Stop.
- Added Dr. Maxwell / MIDI Evil icon and splash branding with editable Aseprite masters and documented sources.
- Added the public-domain Bach Invention No. 2 example and documented its source and checksum.
- Fixed the piano-roll C0 label crash and added full-MIDI octave-label coverage.
- Fixed drag hit-testing by anchoring note selection to the original mouse-down position.
- Renamed the workspace and native crates to MIDI Evil / `evil-*`; retained the oracle crates as compatibility oracles.
- Added `CLAUDE.md` and the `AGENTS.md` relative symlink for future agent guidance.
- Added native `.evil` → `.mid` export with exact track names, full meter and tempo maps, gain/pan CCs, muted tracks as empty named tracks, program changes, percussion-safe channels, engine size limits, and atomic writes.
- Added persistent source-MIDI identity in `.evil`, so Save updates both the native project and original MIDI file; Save As starts a new native identity.
- Added desktop positional MIDI import, `--track`, `--project`, single-instance forwarding, and source-MIDI save-back.
- Added `project export-midi`, `midi rename-track`, and the public-domain Bach original/renamed fixture pair for engine diff tests.
- Added full time-signature denominators and 480-tick defaults for new native projects while preserving existing files' timing.
- Verified 55 workspace tests, native Bach export, 5/4 asteroids and 2/4 frogger imports/exports, and desktop launch/help behavior.
