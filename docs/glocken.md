# A09.GlockenBlocks reference comparison

Local compatibility research only. NanoStudio-derived projects and reference WAVs remain under `target/`; the preset uses original modal synthesis and contains no copied reference audio.

## Verified fixtures and provenance

References are user-owned NanoStudio 1.42 stereo PCM16 mixdowns at 44,100 Hz with instrument/master gain 1, master effects disabled, and full release/tail rendering. Each case is one velocity-110 note at 120 BPM with a two-second gate.

| Pitch | Reference | SHA-256 | Source fixture |
| --- | --- | --- | --- |
| C2 | `target/phase6/a09-c2-nanostudio.wav` | `d448a6c3ed5c515284bd3fdb363930fd76e47c167363de340bc37654ad24f163` | `target/phase6/a09-c2.prj` |
| C3 | `target/phase6/a09-c3-nanostudio.wav` | `ee790a0cb2e273226b9d68f720a634682ad8c7c39e71d50dc6a6da4563bf55cf` | `target/phase6/a09-c3.prj` |
| C4 | `target/phase6/a09-nanostudio.wav` | `d00abe2d99dd3834346e41fc22939d77adea548a197aa5fe7add8c9168bbc2a0` | `target/phase6/a09-capture-verified.prj` |
| C5 | `target/phase6/a09-c5-nanostudio.wav` | `2c7685038448b19381083a768209f5391ca63cdf4b1717153a812f2a72ca873c` | `target/phase6/a09-c5.prj` |

`target/phase6/reference-checksums.txt` is the supplied capture manifest. The base C4 hash above was computed directly because the initial manifest listed only C2/C3/C5.

## Reproduction and analysis workflow

From the repository root:

```bash
cargo run -p evil-synth --example phase6_render -- \
  --preset glocken \
  --output-dir target/phase6/glocken-after \
  --dump-patch target/phase6/glocken-after-patch.json
python3 -I tools/calibrate_glocken.py \
  --reference-dir target/phase6 \
  --native-dir target/phase6/glocken-after \
  --output target/phase6/glocken-after.json
```

The renderer uses the built-in preset at 48 kHz and the shared stereo mixer with track/master gain 1, center pan, and no channel effects or sends. `--patch-json PATCH.json` renders a candidate and `--cases c2,c4` selects cases. The dumped patch records the exact final parameters. Baseline audio/report are `target/phase6/glocken-before/` and `target/phase6/glocken-before.json`; final outputs are `target/phase6/glocken-after/` and `target/phase6/glocken-after.json`.

For arbitrary files, use repeated `--compare LABEL REFERENCE.wav NATIVE.wav` arguments. Analysis rate, note duration, and tail windows are configurable, although the verified set uses 48 kHz and two-second gates. `tools/calibrate_glocken.py` and `tools/calibrate_phase6_audio.py` require only NumPy and the standard library.

Common measurements include checksums/source format, held/full/active RMS, peak, tuning, spectral centroid and six bands, five time-window spectra, onset, active duration, release/early/late RMS, stereo difference, waveform correlation, and held/full-tail 10 ms RMS-envelope correlation and log-envelope RMSE. References are linearly resampled per channel and then averaged to mono. Full-tail comparisons zero-pad the shorter file. JSON stores measurements and basenames, not copied audio or timestamps.

### Modal estimator

This workflow does not substitute a whole-file FFT centroid for modal analysis. For each pitch it:

1. Finds the largest early 0–2 second zero-padded Hann FFT peak within +/-4% (minimum +/-2 Hz) of candidate ratios 1, 2, 3, 4, 5, 6, 8, and 9. Peaks below -55 dB relative to the strongest mode are marked unresolved.
2. Estimates amplitude trajectories by complex demodulation in non-overlapping 100 ms Hann blocks.
3. Fits log amplitude from 0.05 through 6 seconds for blocks above both 0.5% of that mode's peak and -60 dBFS. The exponential amplitude time constant is `-1/slope`.
4. Reports measured ratio to the equal-tempered fundamental, fitted initial amplitude, decay, block count, and fit R-squared for every resolved mode at every pitch.

The estimates expose non-exponential and non-monotonic behavior rather than hiding it. In particular, weak upper modes often have low fit R-squared and must not be treated as exact identifications.

## Measured modal findings

The old native ratios were the generic bell set 1, 2.76, 5.40, 8.93, and 13.34. The verified references instead have prominent near-harmonic modes. Reference measured ratios are:

| Pitch | Resolved modal ratios, ordered by candidate mode |
| --- | --- |
| C2 | 0.9975, 1.9954, 2.8970, 3.9571, 4.9756, 5.9730, 9.0295 |
| C3 | 0.9973, 1.9785, 2.9763, 3.9554, 4.9841, 5.9417, 7.9406, 9.0408 |
| C4 | 0.9893, 1.9777, 2.9709, 3.9703, 4.9180, 5.9729 |
| C5 | 0.9938, 1.9851, 2.9829, 3.9648, 4.9715, 5.9677, 7.9657, 9.0432 |

The final preset uses ratios 1, 1.99, 2.995, 3.99, 5, 6, 8, and 9.095 with -11-cent oscillator tuning. This produces native ratios 0.9937, 1.9774, 2.9760, 3.9646, 4.9684, 5.9620, 7.9494, and 9.0374 at middle C. It intentionally does not reproduce C2's outlying third-mode estimate or every pitch-varying ratio exactly.

### Fundamental fits, reference versus final native

| Pitch | Reference amplitude / decay | Native amplitude / decay | Reference fit R² / native fit R² |
| --- | ---: | ---: | ---: |
| C2 | 0.22986 / 1.098 s | 0.25331 / 1.106 s | 0.986 / 1.000 |
| C3 | 0.25693 / 1.085 s | 0.21929 / 1.104 s | 0.982 / 1.000 |
| C4 | 0.20422 / 1.098 s | 0.18900 / 1.103 s | 0.866 / 1.000 |
| C5 | 0.14914 / 1.117 s | 0.16260 / 1.102 s | 0.944 / 1.000 |

The old generic fundamental had a fitted 1.873-second effective decay and was exactly equal-tempered. The new 1.1-second one-shot modal decay is substantially closer and continues through note-off instead of being truncated by the old four-second linear release.

Second-mode reference/native amplitudes are C2 0.11570/0.11608, C3 0.08478/0.06215, C4 0.02156/0.03319, and C5 0.02210/0.01772. Native second-mode decay is approximately 1.21 seconds versus reference estimates 1.03, 1.09, 1.30, and 1.47 seconds.

Sixth-mode reference/native amplitudes demonstrate the unresolved non-monotonic pitch response: C2 0.00502/0.00576, C3 0.02714/0.00855, C4 0.00241/0.01273, and C5 0.04068/0.01887. A monotonic per-partial pitch-gain fit cannot reproduce the C3/C5 peaks without over-driving other pitches. Complete eight-mode measurements and fit quality are in the JSON.

## Before/after metrics

Each native cell is **before → after (reference)**. RMS and centroid cover the two-second held note.

| Pitch | Held RMS | Centroid Hz | Full-tail envelope correlation | Full-tail envelope RMSE dB |
| --- | ---: | ---: | ---: | ---: |
| C2 | 0.136794 → 0.101783 (0.099891) | 243 → 195 (156) | 0.8350 → 0.9645 | 35.07 → 4.50 |
| C3 | 0.136740 → 0.083399 (0.109084) | 431 → 335 (387) | 0.8448 → 0.9360 | 35.39 → 8.68 |
| C4 | 0.136743 → 0.070424 (0.085332) | 814 → 627 (839) | 0.8191 → 0.8466 | 35.88 → 10.64 |
| C5 | 0.136750 → 0.060521 (0.073096) | 1,565 → 1,296 (2,150) | 0.8381 → 0.9316 | 35.49 → 8.17 |

C2 RMS error improves from +36.9% to +1.9%; C4 and C5 improve from +60.2%/+87.1% to -17.5%/-17.2%. C3 changes from +25.4% to -23.5%, a similar absolute level error but with substantially better modal ratios and tail shape. C2/C3 centroid errors decrease; C4/C5 remain too dark, with C5 centroid explicitly worse than baseline as a tradeoff for replacing the generic high-ratio bell partials.

Held-note waveform correlation improves from -0.1625 to +0.6798 at C2 and from -0.0149 to +0.4968 at C5. C4 improves from approximately zero to +0.2046. C3 remains low (-0.0955), consistent with its unmodeled amplitude/phase behavior. These correlations are not evidence of sample equivalence.

### Full release and late tails

Each RMS cell is **before → after (reference)**.

| Pitch | Release 2–2.5 s | Early 3–4 s | Late 6–7 s | Active seconds |
| --- | ---: | ---: | ---: | ---: |
| C2 | 0.099225 → 0.027706 (0.025550) | 0.055528 → 0.009606 (0.008737) | 0 → 0.000714 (0.000534) | 5.977 → 8.298 (7.350) |
| C3 | 0.099507 → 0.022024 (0.027026) | 0.055560 → 0.007587 (0.009442) | 0 → 0.000570 (0.000589) | 5.981 → 8.338 (7.653) |
| C4 | 0.099482 → 0.018447 (0.020667) | 0.055550 → 0.006358 (0.007561) | 0 → 0.000513 (0.000490) | 5.985 → 8.551 (7.782) |
| C5 | 0.099489 → 0.015923 (0.020157) | 0.055573 → 0.005578 (0.007057) | 0 → 0.000518 (0.000451) | 5.988 → 9.000 (7.561) |

The baseline allocated no samples beyond six seconds, hence zero measured late RMS. The final one-shot modes have real late energy at every pitch. Active duration uses each file's own peak-relative threshold, so lower native peaks can extend the measured activity even when absolute tail RMS is close; allocated file length and active duration are not claims of matching source decay exactly.

## DSP changes and rejected alternatives

- Replaced all five generic partials with eight measured near-harmonic modes and calibrated initial levels, exponential decays, and per-partial pitch gains.
- Added `ModalPartial::pitch_gain_per_octave`, defaulting to zero for bit-compatible legacy behavior.
- Added opt-in `modal_one_shot`, defaulting to false. Glocken bypasses its amplitude ADSR so natural modal decay continues after note-off; seven seconds of release allocation preserves the rendered tail. Other modal patches retain their original envelope routing.
- Removed the old sustained sine contribution by setting oscillator amplitude to zero. The oscillator still supplies the tracked fundamental frequency and -11-cent tuning for the modal bank.
- Compensated middle-C partial amplitudes by sqrt(2) for the shared mixer's constant-power center-pan gain. Without this correction, fitted native amplitudes were systematically 29% low.
- Rejected a further uniform level boost: it improved C3–C5 held RMS but worsened C2 and the already-close absolute late-tail measurements. The selected candidate keeps the best overall modal and tail balance rather than concealing missing source behavior with gain.

## Remaining differences and limitations

This is an original-synthesis recreation, **not sample equivalence or a 1:1 instrument**. C3–C5 held RMS remains 17–24% low, C4/C5 spectra remain too dark, and C5's strong sixth mode is underrepresented. C2's third mode and C4 tuning are not exact; one global -11-cent correction cannot reproduce pitch-dependent reference tuning. Reference tuning estimates are -11.4, -15.4, -20.7, and -10.4 cents versus approximately -11.0 cents native.

The references have non-exponential amplitude motion, including C3/C4 swells, which the independent exponentially decaying modes do not reproduce. Native peaks remain substantially lower, and the centered dual-mono path does not recreate reference stereo differences. Upper-mode fits with low R-squared are approximate. Reference onsets vary from 0 to 3 ms while native onset is 0 ms. Only C2–C5 velocity-110 two-second-gate fixtures at 120 BPM were measured; other velocities, gates, pitches, and tempo-dependent source behavior remain unverified.

## Verification

```bash
cargo test -p evil-synth
cargo test -p evil-render
cargo clippy -p evil-synth --all-targets -- -D warnings
cargo test --manifest-path compat/Cargo.toml -p evil-synth
cargo clippy --manifest-path compat/Cargo.toml -p evil-synth --all-targets -- -D warnings
```

New deterministic tests cover one-shot release behavior, per-partial pitch gain, and legacy defaults. On 2026-11-05, all listed checks passed: 135 `evil-synth` tests in each workspace, 26 `evil-render` tests in each workspace, and strict clippy in both workspaces. Repeating the final analysis produced byte-identical JSON (SHA-256 `eb454d9b4b06ae15c1fe52a3f06db872b35236a8609fcaf8ef109e3e6fc0b477`). `cargo check --manifest-path compat/Cargo.toml -p oracle` also passed; no native project mapping changes were needed.
