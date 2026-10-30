# A17.Nasty reference comparison

Local compatibility research only. NanoStudio-derived banks and reference WAVs stay under `target/` and are not redistributed as MIDI Evil assets.

## Fixture

- NanoStudio 1.42, Global A record 17 (`Nasty`), with the factory 636-byte preset record as the parameter source.
- One C4 note, velocity 110, at 120 BPM, held for one complete 4/4 bar (2 seconds; 1,920 ticks at 480 ticks/quarter or 1,024 at 256).
- **Entire Song** mixdown with **Render tail** enabled.
- Clean comparison: channel/master gain 1 and master compressor, EQ, reverb, and delay disabled so the render isolates A17 and its insert delay.
- Reference: `target/nasty/a17-wholebar-clean-nanostudio.wav`, stereo PCM16 at 44,100 Hz, 8.705 seconds, SHA-256 `7db8817fde630832c8b730d87d147842ff3ecaa3a4043deb30a89c0aba6b5359`.
- Native: `target/nasty/beatsync-c4.wav`, stereo PCM16 at 48,000 Hz, 6.135 seconds, SHA-256 `691c687af91e3187e60747826c3b0fdc5c9a33d098a5e9269526d667fd638905`.
- Reproduce the native side with `docs/cli-ai.md`'s full-bar workflow, using gain 1 for both track and master. Resample before comparing numeric metrics.

## Measured on 2026-10-30

Mono mean of stereo channels, with the reference linearly resampled to 48 kHz. Spectral metrics cover the two-second held note. Tail RMS covers 3–4 seconds and 6–7 seconds respectively.

| Metric | Initial native | Calibrated native | With LFO4 routing | Open-filter native | Rhythmic native | Multi-pitch native | NanoStudio reference |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Held-note RMS | 0.540583 | 0.146235 | 0.146146 | 0.144813 | 0.145897 | 0.146639 | 0.147276 |
| Spectral centroid | 1,752 Hz | 3,930 Hz | 3,400 Hz | 4,133 Hz | 4,243 Hz | 3,992 Hz | 4,198 Hz |
| Waveform correlation | -0.0283 | 0.0038 | 0.0294 | 0.0448 | 0.0460 | 0.0499 | 1.0 |
| 10 ms RMS envelope correlation | — | — | — | 0.0789 | 0.4841 | 0.4689 | 1.0 |
| RMS at 3–4 seconds | 0.001685 | 0.008993 | 0.009408 | 0.010710 | 0.013297 | 0.013412 | 0.016845 |
| RMS at 6–7 seconds | 0 | 0.000373 | 0.000397 | 0.000447 | 0.000612 | 0.000679 | 0.000455 |

Changes:

- Interpret resonant filter `stages` as 6 dB/octave units: four units now use two 12 dB SVF stages, not four.
- Anchor filter key tracking at middle C, independently of the oscillators' A4 pitch reference.
- Calibrate A17 to gain 0.27, cutoff 5.5 kHz, 24 dB slope, 0.5-second delay at 120 BPM, and 55% feedback. The delay remains 20% wet.
- Add A17's fourth LFO as a 20 Hz square lane that modulates LFO3's rate, calibrated to 0.25 relative depth. This improves RMS and correlation but reduces the centroid versus the previous approximation, so it is not uniformly closer on every metric.
- Remove limiter-induced DC only for sync-based sounds, including their final stereo mixes. The strict dry reference has essentially no sub-200 Hz energy; the native sync path previously had a 0.0394 DC offset.
- Add explicit naive-square and square-LFO options without changing existing bandlimited oscillators.
- Match the dry reference's harmonic structure with a 1.25 base sync ratio, 15.5-cent tuning correction, and a near-open 20 kHz non-resonant 24 dB filter. Set gain to 0.31 for the shared stereo project renderer. This removes the previous exaggerated 4–8 kHz resonance and restores much of the missing 8–16 kHz energy, improving both centroid and waveform correlation.
- Clamp key-tracked cutoff to the processing limit before per-note patch validation, so the near-open filter also renders notes above middle C instead of rejecting them.
- Decode LFO3's source sync mode 6 as sixteenth-note timing, giving 8 Hz at the 120 BPM reference tempo. Make LFO-to-LFO rate modulation phase-continuous, use 0.15/0.20 sync-ratio depths, and add an 8 Hz saw-down amplitude-articulation lane at 0.95 depth. Jointly calibrate gain to 0.34 and cutoff to 15 kHz with the original delay. This raises 10 ms envelope correlation from 0.0789 to 0.4841 while retaining close RMS, centroid, and waveform-correlation matches.
- Slow the added amplitude articulation to 4 Hz after multi-pitch listening and comparison, while retaining LFO3 at 8 Hz. Add a 0.3-per-octave pitch-gain control around middle C to match the source's much stronger low-note output and quieter high-note output. C2's 0–200 Hz band now measures 78.52 dB versus the reference's 78.46 dB.
- Add `beats_per_cycle` and ordered tempo-change events through native project, mono, and stereo-mix rendering. Nasty's LFO3 now uses 0.25 beats/cycle and its amplitude lane 0.5, so both follow 60/120/240 BPM projects and mid-song tempo changes rather than remaining fixed at the reference rate. Add shared offline/live sync-reset anti-aliasing for square and saw voices.

## Isolation fixtures

`target/nasty/isolation-v2/` contains bank variants for dry, vibrato, LFO2, LFO3, LFO4, and all modulation. `MANIFEST.json` records the preset fields and enabled lanes. Delay wet mix is zero for these captures. The strict dry reference is `target/nasty/a17-strict-dry-nanostudio.wav`; the all-modulation/no-delay control is `target/nasty/a17-all-nanostudio.wav`.

Earlier captures under `target/nasty/isolation/` left LFO4 active and must not be treated as fully isolated references. Maximum-cutoff and mode-zero experiments did not provide a true filter bypass, so raw-oscillator claims are intentionally avoided.

## Multi-pitch fixtures

C2, C3, and C5 references use the same NanoStudio-saved whole-bar project as the clean C4 control, changing only the single note through the binary pattern editor. Each capture includes the release/effect tail. The corrected C2 capture has a verified 66 Hz fundamental; an earlier archive-import attempt that still rendered C4 was discarded.

| Pitch | Native RMS | Reference RMS | Native centroid | Reference centroid | 10 ms envelope correlation |
| --- | ---: | ---: | ---: | ---: | ---: |
| C2 | 0.210825 | 0.224185 | 1,298 Hz | 1,233 Hz | 0.3962 |
| C3 | 0.177461 | 0.179252 | 2,029 Hz | 2,437 Hz | 0.3045 |
| C4 | 0.146639 | 0.147276 | 3,992 Hz | 4,198 Hz | 0.4689 |
| C5 | 0.119829 | 0.129100 | 5,899 Hz | 5,576 Hz | 0.5411 |

References: `target/nasty/a17-c2-nanostudio.wav` (SHA-256 `80a2ab2a8d646b0bb020c68abcc204f73f96cfd8847bd778f975801f7e0ccea0`), `target/nasty/a17-c3-nanostudio.wav` (`76fbbd16d4a5200a5c524595fda24555e91e581440c42eae3d71513a8145626c`), and `target/nasty/a17-c5-nanostudio.wav` (`62295036c51c2fa6fcd71daf9b817e65eaa86e35050c43de8f9a492524040c1f`). Native outputs are `target/nasty/beatsync-{c2,c3,c4,c5}.wav`.

C2's 0–200 Hz energy is now effectively matched, but its 16–24 kHz band remains too strong (28.8 dB versus 7.48 dB), and C3/C5 still have different spectral shapes. These differences prevent a 1:1 claim despite close pitch-dependent RMS.

## Rhythmic modulation

The source UI and preset record confirm that LFO3 has **Beat Sync** enabled with sync mode **6**. At the 120 BPM reference tempo this maps to sixteenth notes, or 8 Hz; LFO2 and LFO4 remain free-running. The built-in preset now uses that LFO3 rate, phase-continuous LFO-rate modulation, and a calibrated 4 Hz amplitude-articulation lane. Both lanes use `beats_per_cycle` and follow the project's complete tempo map.

## Remaining differences

This is a closer original-synthesis recreation, **not a 1:1 or sample-identical instrument**. Direct held-note waveform correlation remains low and the high-frequency band shape is still approximate. The native early release/delay tail remains quieter, its late tail is stronger, and the file-level tail is shorter. Exact LFO phase/depth behavior, envelope curves, and feedback mapping still need differential work. Matching RMS, spectral centroid, and rhythmic-envelope correlation alone must not be treated as audio equivalence.

The blank `oracle song create` template also failed NanoStudio 1.42's project-open acceptance check. The reference fixture therefore uses the official Minimal project structure with its sequence replaced, which reopened successfully with the full-bar part.
