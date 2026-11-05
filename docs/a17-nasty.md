# A17.Nasty reference comparison

Local compatibility research only. NanoStudio-derived banks and reference WAVs stay under `target/` and are not redistributed as MIDI Evil assets.

## Fixture

- NanoStudio 1.42, Global A record 17 (`Nasty`), with the factory 636-byte preset record as the parameter source.
- One C4 note, velocity 110, at 120 BPM, held for one complete 4/4 bar (2 seconds; 1,920 ticks at 480 ticks/quarter or 1,024 at 256).
- **Entire Song** mixdown with **Render tail** enabled.
- Clean comparison: channel/master gain 1 and master compressor, EQ, reverb, and delay disabled so the render isolates A17 and its insert delay.
- Reference: `target/nasty/a17-wholebar-clean-nanostudio.wav`, stereo PCM16 at 44,100 Hz, 8.705 seconds, SHA-256 `7db8817fde630832c8b730d87d147842ff3ecaa3a4043deb30a89c0aba6b5359`.
- Phase 6 native: `target/nasty/phase6-after/c4.wav`, stereo PCM16 at 48,000 Hz, 10.040 seconds of allocated delay tail, SHA-256 `7f4ac6675cd8c7c6beec00f43fb07631defd3fd6a22997fde99c2b834c99a30f`. Its measured active duration is 7.325 seconds; the older `beatsync-*.wav` files are stale.
- Reproduce the native side with `docs/cli-ai.md`'s full-bar workflow, using gain 1 for both track and master. Resample before comparing numeric metrics.

## Phase 6 calibration, measured on 2026-11-05

Fresh pre-change C2/C3/C4/C5 renders and post-change built-in-preset renders use the same 120 BPM, velocity-110, two-second-note projects. Reports are `target/nasty/phase6-before.json` and `target/nasty/phase6-after.json`; audio is in the corresponding directories. The final preset WAVs are byte-identical to the selected `phase6-gain33` candidate.

Reproduce the current renders and report from the repository root:

```bash
cargo build --manifest-path compat/Cargo.toml -p oracle
for pair in c2:a17-c2 c3:a17-c3 c4:a17-wholebar-clean c5:a17-c5; do
  pitch=${pair%%:*}
  project=${pair#*:}
  compat/target/debug/oracle project render \
    "target/nasty/${project}.evil" "target/nasty/phase6-after/${pitch}.wav"
done
python3 -I tools/calibrate_nasty.py \
  --reference-dir target/nasty \
  --native-dir target/nasty/phase6-after \
  --output target/nasty/phase6-after.json
```

`tools/calibrate_nasty.py` uses only NumPy and the standard library. It accepts explicit arbitrary pairs with `--compare LABEL REFERENCE.wav NATIVE.wav`, plus configurable analysis rate, note duration, and release/early/late windows. It reports checksums, file and active durations, onset, held/full-file RMS, spectral centroid, six band energies (including 16–24 kHz at every pitch), held and full-tail 10 ms envelope correlations, waveform correlation, and release/early/late RMS. The JSON contains basenames and measurements, not copied audio or timestamps. Repeating the final command produced byte-identical JSON.

Method: linearly resample each reference channel to 48 kHz, take the stereo mean, and analyze the two-second held note with a Hann FFT. Centroid is magnitude-weighted for continuity with earlier A17 reports; bands use squared magnitude. Envelopes are non-overlapping 10 ms RMS blocks. Onset/activity use 1 ms RMS blocks at least max(0.1% of peak block RMS, -80 dBFS). Tail windows are release 2.0–2.5 s, early 3–4 s, and late 6–7 s.

### Held-note before/after metrics

Each cell is **before → after (NanoStudio reference)**. Band values are FFT energy in dB, not absolute SPL.

| Pitch | Held RMS | Centroid Hz | 16–24 kHz dB | 10 ms envelope correlation |
| --- | ---: | ---: | ---: | ---: |
| C2 | 0.213691 → 0.217147 (0.224185) | 1,281 → 1,186 (1,233) | 28.79 → 19.88 (7.48) | 0.3952 → 0.3943 |
| C3 | 0.178349 → 0.181255 (0.179252) | 2,014 → 2,183 (2,437) | 27.95 → 26.71 (23.74) | 0.3032 → 0.3023 |
| C4 | 0.147006 → 0.149428 (0.147276) | 3,972 → 4,423 (4,198) | 39.70 → 42.49 (44.38) | 0.4702 → 0.4722 |
| C5 | 0.120008 → 0.121771 (0.129100) | 5,878 → 5,800 (5,576) | 47.00 → 46.84 (50.23) | 0.5426 → 0.5442 |

C2's excessive 16–24 kHz energy falls by 8.91 dB. Its residual error is still +12.40 dB, so this is a reduction, not a band match. C3–C5 RMS remains within 5.7% of the references, C3/C5 centroid error decreases, and C4 centroid error is nearly unchanged in magnitude (-226 Hz before, +225 Hz after). Full-tail 10 ms envelope correlations improve as follows: C2 0.8770 → 0.8873, C3 0.8729 → 0.8830, C4 0.8888 → 0.9006, and C5 0.9078 → 0.9177.

### Tail before/after metrics

Each RMS cell is **before → after (NanoStudio reference)**. Active duration uses the 1 ms activity definition above, not the allocated WAV length.

| Pitch | Release RMS | Early RMS | Late RMS | Active seconds |
| --- | ---: | ---: | ---: | ---: |
| C2 | 0.111652 → 0.097704 (0.095623) | 0.019839 → 0.022607 (0.027439) | 0.001004 → 0.000698 (0.000739) | 6.135 → 7.342 (7.253) |
| C3 | 0.093053 → 0.081239 (0.082620) | 0.016459 → 0.018747 (0.023048) | 0.000834 → 0.000579 (0.000585) | 6.134 → 7.323 (7.254) |
| C4 | 0.076135 → 0.066804 (0.067739) | 0.013437 → 0.015363 (0.016845) | 0.000681 → 0.000474 (0.000455) | 6.135 → 7.325 (7.254) |
| C5 | 0.061690 → 0.054394 (0.059292) | 0.010873 → 0.012472 (0.014082) | 0.000551 → 0.000385 (0.000381) | 6.135 → 7.314 (7.253) |

All four late-tail RMS values are now within 6% of their references. Early tails remain 8.8–18.7% quieter, although all improve. C5's release RMS error increases in magnitude from +4.0% to -8.2%, while the other release errors decrease substantially. Reference onsets are 2–3 ms; native onsets are 0 ms. Native allocated files are 10.040 s because they reserve 16 delay taps, versus 8.626–8.821 s reference files; active durations are the more useful comparison and do not imply matching file lengths.

### Multi-tempo delay-cap follow-up

New verified static-tempo whole-bar references are `target/phase6/a17-60-nanostudio.wav` and `target/phase6/a17-240-nanostudio.wav`, from the self-contained `a17-60-ui.prj` and `a17-240-ui.prj` projects. `target/phase6/a17-tempo-checksums.txt` records their SHA-256 hashes. Before-cap reports are `target/phase6/a17-tempo-{60,240}.json`; capped reports are `target/phase6/a17-tempo-{60,240}-cap.json`.

A fixed-time 0.25 s candidate at 240 BPM first showed that the inferred beat-synced, 0.5 s-capped behavior was worth implementing. Nasty now uses `beats_per_cycle: 1.0` and `maximum_time_seconds: 0.5`, giving delay times 0.5/0.5/0.25 s at 60/120/240 BPM. The optional cap applies to fixed or beat-synced delays; `None` preserves legacy patches and the independent `tail_seconds` control is unchanged.

| BPM | Held RMS before → after (reference) | Centroid before → after (reference) | Full-tail envelope correlation | Active seconds before → after (reference) |
| --- | ---: | ---: | ---: | ---: |
| 60 | 0.159573 → 0.159573 (0.146528) | 4,483 → 4,483 (4,639) | 0.87796 → 0.87796 | 9.166 → 9.166 (9.126) |
| 240 | 0.136356 → 0.153885 (0.166470) | 4,292 → 4,090 (3,912) | 0.92156 → 0.91928 | 6.041 → 3.662 (3.626) |

| BPM | Release RMS before → after (reference) | Early RMS before → after (reference) | Late RMS before → after (reference) |
| --- | ---: | ---: | ---: |
| 60 | 0.071299 → 0.071299 (0.060996) | 0.005197 → 0.005197 (0.005598) | 0.000160 → 0.000160 (0.000124) |
| 240 | 0.057421 → 0.061453 (0.066906) | 0.012737 → 0.003931 (0.004846) | 0.002760 → 0.000164 (0.000152) |

At 240 BPM the cap improves held RMS error from -18.1% to -7.5%, narrows centroid error, raises held-note envelope correlation from 0.37492 to 0.40431, and brings active duration within 36 ms. Late RMS drops from 18.1× to 1.075× the reference; early RMS changes from 2.63× to 0.811×, so it is now 18.9% quiet. Full-tail envelope correlation decreases slightly by 0.00228 and is reported rather than hidden. The capped built-in 240 WAV is byte-identical to the fixed-time candidate. The 60 BPM WAV is byte-identical before/after, and all C2–C5 120 BPM WAVs plus `target/nasty/phase6-cap-after.json` are byte-identical to the Phase 6 outputs above.

Reproduce the tempo reports with:

```bash
for bpm in 60 240; do
  compat/target/debug/oracle project render \
    "target/nasty/a17-tempo-${bpm}.evil" \
    "target/phase6/a17-${bpm}-cap-native.wav"
done
python3 -I tools/calibrate_nasty.py \
  --compare bpm60 target/phase6/a17-60-nanostudio.wav target/phase6/a17-60-cap-native.wav \
  --note-seconds 4 --release-tail 4:4.5 --early-tail 6:7 --late-tail 9:10 \
  --output target/phase6/a17-tempo-60-cap.json
python3 -I tools/calibrate_nasty.py \
  --compare bpm240 target/phase6/a17-240-nanostudio.wav target/phase6/a17-240-cap-native.wav \
  --note-seconds 1 --release-tail 1:1.5 --early-tail 2:3 --late-tail 3.5:4 \
  --output target/phase6/a17-tempo-240-cap.json
```

### Measured changes and rejected alternatives

- Set gain 0.33, cutoff 18 kHz, release 0.04 s, delay feedback 0.56, and delay wet mix 0.25. Retain the previous tuning, sync ratios, LFO rates/depths, and 0.3-per-octave pitch gain. Use a one-beat delay capped at 0.5 s through the new opt-in `maximum_time_seconds`; 60/120 BPM audio is unchanged and 240 BPM uses the measured 0.25 s delay.
- Add opt-in `amplitude_transition_seconds` to LFOs. Nasty's added amplitude saw uses 2/48,000 s, applying PolyBLEP over two frames at 48 kHz. Zero preserves legacy raw modulation. The transition is deterministic and shared by offline/live voice rendering.
- Reserve 16 delay taps instead of eight in mono, stereo, automated, and typed-insert paths. The old allocation cut off audible content at 6.135 s; the new allocation exposes the calibrated tail through approximately 7.3 s of activity.
- A cutoff sweep alone could not fix C2: even very low cutoffs left a roughly 25 dB 16–24 kHz floor. Removing only the amplitude saw dropped that band to 0.15 dB, identifying post-filter amplitude-reset aliasing as the dominant source. Master/voice limiter bypass experiments did not remove the floor.
- One-pole amplitude smoothing near 400–800 Hz could approach the high-band target but destroyed legitimate 8–16 kHz energy and dropped C2 centroid near 800 Hz. Wider PolyBLEP transitions had the same problem. Two frames were selected as the measured balance; they do not claim exact source-filter equivalence.

### Current dry/all-modulation controls

`target/nasty/phase6-controls.json` compares fresh current-code controls with `a17-strict-dry-nanostudio.wav` and `a17-all-nanostudio.wav`; patches and WAVs are under `target/nasty/phase6-controls/`. Delay wet mix is zero in both native controls, and strict-dry also removes all LFO lanes.

| Control | Native RMS | Reference RMS | Native centroid | Reference centroid | 10 ms envelope correlation |
| --- | ---: | ---: | ---: | ---: | ---: |
| Strict dry | 0.145597 | 0.199008 | 4,706 Hz | 7,102 Hz | -0.0788 |
| All modulation, no delay | 0.120395 | 0.156408 | 4,749 Hz | 4,170 Hz | 0.3985 |

These controls expose substantial remaining dry level/spectrum and no-tail differences. They are diagnostics, not evidence of isolated-LFO or sample parity.

Verification for Phase 6 (136 synth tests and 26 shared-renderer tests passed in each workspace, with strict package clippy clean):

```bash
cargo test -p evil-synth
cargo test -p evil-render
cargo clippy -p evil-synth --all-targets -- -D warnings
cargo clippy -p evil-render --all-targets -- -D warnings
cargo test --manifest-path compat/Cargo.toml -p evil-synth
cargo test --manifest-path compat/Cargo.toml -p evil-render
cargo clippy --manifest-path compat/Cargo.toml -p evil-synth --all-targets -- -D warnings
cargo clippy --manifest-path compat/Cargo.toml -p evil-render --all-targets -- -D warnings
```

## Historical measurements on 2026-10-30

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

`target/nasty/isolation-v2/` contains bank variants for dry, vibrato, LFO2, LFO3, LFO4, and all modulation. `MANIFEST.json` records the preset fields and enabled lanes, but its only accepted captures are the strict-dry and all-modulation/no-delay controls: `target/nasty/a17-strict-dry-nanostudio.wav` and `target/nasty/a17-all-nanostudio.wav`. Accepted isolated individual-LFO captures are absent. Delay wet mix is zero for these controls.

Earlier captures under `target/nasty/isolation/` left LFO4 active and must not be treated as fully isolated references. Maximum-cutoff and mode-zero experiments did not provide a true filter bypass, so raw-oscillator claims are intentionally avoided.

## Historical multi-pitch fixtures

The table and `beatsync-{c2,c3,c4,c5}.wav` outputs below are retained only as the 2026-10-30 baseline history; they are stale. Use the Phase 6 reports above for current code.

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

This is a closer original-synthesis recreation, **not a 1:1, sample-equivalent, or sample-identical instrument**. Direct held-note waveform correlation remains low after Phase 6 (C2 0.0006, C3 0.0363, C4 0.0525, C5 -0.0807). C2's 16–24 kHz band remains 12.40 dB too strong, early delay tails remain quieter, C5 RMS remains 5.7% low, and dry/no-delay controls retain large level, spectrum, and duration gaps. The 2–3 ms reference onset offset, exact LFO phase/depth behavior, envelope curves, and feedback mapping remain unresolved. Matching RMS, centroid, envelope correlation, or individual tail windows must not be treated as audio equivalence.

Static 60/120/240 BPM whole-bar references have now been measured, but this is not blanket multi-tempo or sample parity. At 60 BPM, native held RMS remains 8.9% high and release/late-tail differences remain; at 240 BPM, held RMS remains 7.5% low, early tail RMS is 18.9% low, and full-tail envelope correlation decreases slightly after the cap. No mid-song tempo-change reference or accepted isolated individual-LFO capture is available. The strict-dry and all-modulation controls do not fill those gaps.

The blank `oracle song create` template also failed NanoStudio 1.42's project-open acceptance check. The reference fixture therefore uses the official Minimal project structure with its sequence replaced, which reopened successfully with the full-bar part.
