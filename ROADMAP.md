# MIDI Evil Roadmap

Living tracker for planned native features and follow-up fixes. NanoStudio backporting is retired as an active goal; local reference renders may still guide original sound recreation and native NanoStudio 1 feature parity. The capability checklist and status ledger live in `docs/nanostudio1-parity.md`. Completed work moves to `ARCHIVE.md`. Engine-integration order follows `docs/insiculous-crossing.md`.

## Native NanoStudio 1 feature parity

- [ ] Build the complete native instrument, pattern, sampling, mixer/effects, and arrangement workflow
  - Phase 1: versioned shared instruments, reusable pattern parts, native drum-track rendering, and shared-instrument MIDI channels.
    - Completed: schema v2 with v1/legacy migration, repeated/cropped pattern expansion, 64-track shared-instrument tests, offline stereo kit rendering, `+KIT`/`add-drum-track`, and CLI WAV/MIDI smoke verification. Pattern editing and pad-performance linking remain in phase 2.
  - Phase 2: pattern/arranger editing and keyboard/pad/MIDI recording integrated with history and transport.
    - Progress: added shared/unique part arranging, move/crop/duplicate/make-unique/merge and section duplication, pattern-aware piano-roll/automation editing, explicit gridded CC ramps, quantize/transpose/copy/cleanup, one-take history, loop recording from keyboard/pads/desktop `midir` input, sample-clocked held-pad repeat capture, automatic Pads-bank loading/save linking, empty-song recording clocks, structural-edit take protection, chronological take commits, and model/parser/history tests. Desktop smoke on 2026-11-02 verified saved-project arranger operations/history, empty-song keyboard takes, explicit CC ramps/history, held-pad repeat capture/history, automatic bank linking, and virtual ALSA MIDI note/CC recording; fixed arranger drop coordinates and relative-project bank/patch references. Direct arranger drag gestures could not be driven by the smoke tool and physical hardware MIDI was unavailable, so those remain unverified.
  - Phases 3–5: Eden-style modulation/sampling, complete TRG voices/sample editing, and one shared automatable effects graph.
    - Phase 3 complete: added 36 named native basic waveforms, with 31 band-limited mip-mapped variants, high-rate normalization and shared sync-reset correction alongside the legacy oscillator paths. Added explicit Osc A/B mix, master/slave sync and ring modes plus poly/mono/legato voice allocation in the shared offline/live engine and Patch Lab. Added independent filter ADSR routing with amplitude-envelope fallback for legacy JSON, band-pass filtering, and corrected cascaded 12/24 dB LP/BP/HP coverage with clean/resonant response tests. Added two independently optional auxiliary ADSRs with independent timing and signed amplitude, pitch, pulse-width, sync-ratio, filter-cutoff, filter-resonance, oscillator-A/B-mix, and targeted LFO-rate matrix depths, plus signed LFO filter-resonance and A/B-mix routing, disabled by default for legacy audio compatibility, with validation, separate Patch Lab controls, and deterministic summing with existing modulation including LFO pulse width and sync ratio. Added one embedded sample oscillator with shared mono/stereo WAV and uncompressed AIFF/AIFC import, deterministic interpolation, root-frequency pitch tracking, custom start/exclusive-end loop points with full-source defaults and one-shot preservation, JSON round trips, and Patch Lab waveform/loop-marker controls. Added validated multi-sample key mapping with inclusive non-overlapping MIDI zones, note-on zone selection that remains fixed under pitch modulation, legacy full-keyboard equivalence, and Patch Lab zone split/add/remove, WAV replacement, key-range, root/loop, and waveform controls. Added non-destructive keyboard slicing that clones the embedded source, divides selected keys and playback frames evenly with earlier slices receiving remainders, rejects empty/invalid slices, and gives each loop-enabled slice its own editable loop range. Patch Lab supports full-sample and per-zone slicing plus non-destructive playback bounds while retaining manual zone editing. Deterministic multi-pitch tests compare offline and live output for every new mode and envelope/filter/modulation/sample combination, including fixed/tracked pitch and looped/one-shot zones and slices. Added two fixed per-instrument insert slots with symmetric drive/contour waveshaping before the existing delay, shared direct/stereo/live ordering, Patch Lab controls, and timeline CC 20–22 waveshaper plus CC 23–25 delay automation with exact static-equivalence and native-project tests. Added bounded CPAL microphone recording into Osc A with hardware-independent buffer tests and virtual ALSA CPAL-to-Patch-Lab smoke verified in both workspaces on 2026-11-03, plus exact transport-range project resampling with end-of-section silence padding, background desktop rendering, stereo downmixing, and Osc A embedding in Patch Lab. The native oscillator/filter/amplitude matrix and fixed insert automation are complete; the shared effects graph remains Phase 5.
  - Phase 6: stem/part export, favorite-sound calibration, and the final verified parity audit.
  - Track individual capabilities in `docs/nanostudio1-parity.md`; do not claim parity from format round trips alone.

## Preferred sound recreations

- [ ] Build an explicit feature matrix for the user's go-to instruments
  - **Max Nasty (A17):** accurate Osc Mix/hard-sync behavior, anti-aliasing, pitch-dependent gain, beat-synced LFOs, filter/envelope shape, and delay/release tails. Validate C2/C3/C4/C5 and multiple tempos rather than relying on a single reference note.
    - Progress: close RMS and low-end matching, 4 Hz articulation, 8 Hz LFO3 at 120 BPM, phase-continuous rate modulation, and full tempo-map beat sync. Exact Osc Mix, C3/C5 spectral shape, aliasing, and tail behavior remain open. See `docs/a17-nasty.md`.
  - **Evil-303 (B14):** resonant filter envelope, glide, accent, drive/timbre, and tempo-aware sequencing. Capture isolated filter/glide/accent references before calibrating the native patch.
    - Progress: added `glide_seconds` with previous-note tracking in offline/live voices, velocity-driven filter/gain accents, and the original `nano-303` preset with regression tests and a rendered transition demo. Reference calibration and drive/timbre work remain open.
  - **A09.GlockenBlocks:** inharmonic/modal partials, accurate tuning at several pitches, full decay, and pitch/envelope behavior. Preserve long release tails and avoid a generic short bell approximation.
    - Progress: added up to 16 modal partials per patch with independent ratio/amplitude/decay and a five-partial `glocken` preset with long tails and regression coverage. Reference calibration remains open.
  - **Chiptune drumkit:** per-pad tuning, envelopes, output filters, pan/gain, and overlapping hits. Capture pads individually and in a dense pattern, then compare the native bank and stereo mix.
  - Keep reference-guided rendering scriptable and measure changes against several pitches and complete tails; do not claim sample equivalence from matching parameters alone.

## Native synthesis and drum engine

- [ ] Complete reference-guided synthesis behaviors
  - Implement accurate Osc A/B mixing and hard sync, source-shaped filter/amplitude envelopes, glide/accent routing, filter-envelope modulation, and tempo-synchronized effects.
  - Progress: added 36 native basic waveforms with band-limited mip-mapping, `beats_per_cycle` with ordered `SynthTempoChange` events, mid-render tempo changes, pitch gain, phase-continuous LFO-rate modulation, shared offline/live sync-reset anti-aliasing, previous-note glide, velocity accents, independently decaying modal partials, true Osc A/B mix/sync/ring routing, poly/mono/legato voice modes, independent filter ADSR routing, LP/BP/HP 12/24 dB filters, two independently optional auxiliary ADSRs with independent timing and signed amplitude/pitch/pulse-width/sync-ratio/filter-cutoff/filter-resonance/oscillator-A/B-mix/targeted-LFO-rate matrix routing, signed LFO filter-resonance and A/B-mix modulation, per-instrument waveshaper-before-delay inserts with timeline CC automation, exact section resampling into embedded oscillator audio, and one embedded WAV/AIFF sample oscillator with pitch tracking, validated custom loop points, deterministic short-loop interpolation, one-shot preservation, validated non-overlapping multi-sample key zones with note-on selection fixed under modulation, non-destructive keyboard slicing with deterministic key/frame remainders and slice-local loops, and multi-pitch offline/live regression tests. A17-specific Osc Mix calibration, nonlinear source-envelope curve calibration, drive/timbre, and effect-tail matching remain open.

- [ ] Finish `evil-beats` song and rendering integration
  - Add native song-track bank references, recording into patterns, per-pad/output filters, and CLI bank/project rendering without Oracle dependencies.
  - Progress: tuned sample pads, velocity, gate/release envelopes, overlapping hits, portable `.evilkit` banks with embedded audio, WAV import/export, original 16-pad kit, stereo gain/pan/mute, the desktop Pads workspace with held-pad repeat, native song-track bank references, deterministic offline stereo kit rendering, `+KIT`, CLI `add-drum-track`/project rendering, pad recording into timeline/pattern targets, held-repeat take capture, and automatic Pads-bank loading/save linking. Per-pad/output filters remain open.

## Editor and composition workflow

- [ ] Continue polishing Evil Synth and Evil Beats
  - Keep grouped controls responsive at small and large window sizes, retain every JSON/bank parameter, and add useful ADSR/LFO/filter feedback without obstructing editing or audition.
  - Integrate pad recording with section play/repeat, piano-roll editing, and native project history.
    - Progress: physical pad hits and sample-clocked held repeats record into the selected repeating range and commit as one undoable take.

- [ ] Add arrangement conveniences for bar-based writing
  - Section duplicate, loop-aware note duplication, optional recording into the selected range, and clear playhead/selection behavior.
    - Progress: loop duplication copies unique events/controllers and cropped linked parts; shared-part duplication and quantized range recording are implemented.
  - Preserve exact section rendering and the existing audio-clocked playhead through tempo changes.

## Recover editable songs from WAV exports

- [ ] Build an assisted audio-recovery workflow for user-owned recordings
  - Import PCM WAV with documented sample-rate/channel handling, waveform/spectrogram inspection, trimming, and non-destructive source preservation.
  - Estimate tempo, meter, loop boundaries, and repeated sections; allow manual correction before generating notes.
  - Detect drum onsets and monophonic bass/lead pitch tracks first, emitting confidence scores and an editable `.evil` draft rather than pretending every mixture has a unique transcription.
  - Add polyphonic/chord transcription as a later stage with voice separation, ambiguous-note review, and manual editing.
  - Match native patches by comparing rendered candidates with the source, then render the recovered arrangement for iterative A/B listening.
  - Export `.evil` and Standard MIDI with source-audio provenance. Recovered projects are editable approximations, not exact replacements for lost master files.
  - Progress (2026-10-30): created `evil-imports` with PCM WAV loading, activity/onset detection, tempo candidates, JSON inspection, and quantized `.evil` draft generation with tempo/grid overrides. Generated validated onset projects for Dreams (110 BPM), Foolish Thoughts (120 BPM), and The Dagobah Vision (140 BPM) under `target/recovery/`. Spectrogram review, loop detection, pitch tracking, and patch matching remain open.

## Packaging and release

- [ ] Complete native package metadata and release documentation
  - Confirm artwork redistribution, license fields, descriptions, repository links, and native crate publish order without Oracle packages.
  - Verify a clean public-clone build, desktop smoke test, native tests, and package contents before publishing.
