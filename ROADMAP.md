# MIDI Evil Roadmap

Living tracker for planned native features and follow-up fixes. NanoStudio backporting is retired as an active goal; local reference renders may still guide original sound recreation. Completed work moves to `ARCHIVE.md`. Engine-integration order follows `docs/insiculous-crossing.md`.

## Preferred sound recreations

- [ ] Build an explicit feature matrix for the user's go-to instruments
  - **A17.Nasty:** accurate Osc Mix/hard-sync behavior, anti-aliasing, pitch-dependent gain, beat-synced LFOs, filter/envelope shape, and delay/release tails. Validate C2/C3/C4/C5 and multiple tempos rather than relying on a single reference note.
    - Progress: close RMS and low-end matching, 4 Hz articulation, 8 Hz LFO3 at 120 BPM, phase-continuous rate modulation, and full tempo-map beat sync. Exact Osc Mix, C3/C5 spectral shape, aliasing, and tail behavior remain open. See `docs/a17-nasty.md`.
  - **B14.Nano-303:** resonant filter envelope, glide, accent, drive/timbre, and tempo-aware sequencing. Capture isolated filter/glide/accent references before calibrating the native patch.
    - Progress: added `glide_seconds` with previous-note tracking in offline/live voices, velocity-driven filter/gain accents, and the original `nano-303` preset with regression tests and a rendered transition demo. Reference calibration and drive/timbre work remain open.
  - **A09.GlockenBlocks:** inharmonic/modal partials, accurate tuning at several pitches, full decay, and pitch/envelope behavior. Preserve long release tails and avoid a generic short bell approximation.
  - **Chiptune drumkit:** per-pad tuning, envelopes, output filters, pan/gain, and overlapping hits. Capture pads individually and in a dense pattern, then compare the native bank and stereo mix.
  - Keep reference-guided rendering scriptable and measure changes against several pitches and complete tails; do not claim sample equivalence from matching parameters alone.

## Native synthesis and drum engine

- [ ] Complete reference-guided synthesis behaviors
  - Implement accurate Osc A/B mixing and hard sync, source-shaped filter/amplitude envelopes, glide/accent routing, filter-envelope modulation, and tempo-synchronized effects.
  - Progress: added `beats_per_cycle` with ordered `SynthTempoChange` events, mid-render tempo changes, pitch gain, phase-continuous LFO-rate modulation, shared offline/live sync-reset anti-aliasing, previous-note glide, and velocity accents. True Osc Mix, filter-envelope curves, drive/timbre, and effect-tail matching remain open.

- [ ] Finish `evil-beats` song and rendering integration
  - Add native song-track bank references, recording into patterns, per-pad/output filters, and CLI bank/project rendering without Oracle dependencies.
  - Progress: tuned sample pads, velocity, gate/release envelopes, overlapping hits, portable `.evilkit` banks with embedded audio, WAV import/export, original 16-pad kit, stereo gain/pan/mute, and the desktop Pads workspace with held-pad repeat.

## Editor and composition workflow

- [ ] Continue polishing Evil Synth and Evil Beats
  - Keep grouped controls responsive at small and large window sizes, retain every JSON/bank parameter, and add useful ADSR/LFO/filter feedback without obstructing editing or audition.
  - Integrate pad recording with section play/repeat, piano-roll editing, and native project history.

- [ ] Add arrangement conveniences for bar-based writing
  - Section duplicate, loop-aware note duplication, optional recording into the selected range, and clear playhead/selection behavior.
  - Preserve exact section rendering and the existing audio-clocked playhead through tempo changes.

## Recover editable songs from WAV exports

- [ ] Build an assisted audio-recovery workflow for user-owned recordings
  - Import PCM WAV with documented sample-rate/channel handling, waveform/spectrogram inspection, trimming, and non-destructive source preservation.
  - Estimate tempo, meter, loop boundaries, and repeated sections; allow manual correction before generating notes.
  - Detect drum onsets and monophonic bass/lead pitch tracks first, emitting confidence scores and an editable `.evil` draft rather than pretending every mixture has a unique transcription.
  - Add polyphonic/chord transcription as a later stage with voice separation, ambiguous-note review, and manual editing.
  - Match native patches by comparing rendered candidates with the source, then render the recovered arrangement for iterative A/B listening.
  - Export `.evil` and Standard MIDI with source-audio provenance. Recovered projects are editable approximations, not exact replacements for lost master files.

## Packaging and release

- [ ] Complete native package metadata and release documentation
  - Confirm artwork redistribution, license fields, descriptions, repository links, and native crate publish order without Oracle packages.
  - Verify a clean public-clone build, desktop smoke test, native tests, and package contents before publishing.
