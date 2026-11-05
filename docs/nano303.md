# B14.Nano-303 / Evil-303 reference comparison

Local compatibility research only. NanoStudio-derived projects and reference WAVs remain under `target/`; no reference samples are copied into MIDI Evil assets or the original-synthesis preset.

## Verified fixtures and provenance

All references are user-owned NanoStudio 1.42 stereo PCM16 mixdowns at 44,100 Hz, with instrument/master gain 1, master effects disabled, full release/effect tails, and 120 BPM. Single notes have a two-second gate. C2/C3/C4/C5 use velocity 110; soft C4 uses 64 and accent C4 uses 127. Glide is C4 velocity 100 for one second followed by E4 velocity 120 for one second.

| Case | Reference | SHA-256 | Source fixture |
| --- | --- | --- | --- |
| C2 | `target/phase6/b14-c2-nanostudio.wav` | `521034dbb0e8949614d376d968d60b037da2c58ed126e7d329248a3dccf3eb5a` | `target/phase6/b14-c2.prj` |
| C3 | `target/phase6/b14-c3-nanostudio.wav` | `d9f7a96865867feec8ae6584fa282b0b901cc5be99ab620b62c5fefd052a3718` | `target/phase6/b14-c3.prj` |
| C4 | `target/phase6/b14-nanostudio.wav` | `ba4983418eaf55b9d0007480f6a8f376b8bd17f2caafd349753b5ad7e334115c` | `target/phase6/b14-capture-verified.prj` |
| C5 | `target/phase6/b14-c5-nanostudio.wav` | `efc713cee5b69f39179d0b3e321eadfe9904f825b7deb49bdb3b413627e0d898` | `target/phase6/b14-c5.prj` |
| Soft | `target/phase6/b14-soft-nanostudio.wav` | `947bddda70ca6662bba449631e06fc197c6adbe7ecfdb1abd871d9ce857b3021` | `target/phase6/b14-soft.prj` |
| Accent | `target/phase6/b14-accent-nanostudio.wav` | `c46a4ecb5c9c6f3c72d014d1efc9906d3dc965e8223c3d6485271a29670a13e2` | `target/phase6/b14-accent.prj` |
| Glide | `target/phase6/b14-glide-nanostudio.wav` | `63910d3d8b80ef96394190444d0b7f5db75b5865475ea2d6208f676cbee58932` | `target/phase6/b14-glide.prj` |

`target/phase6/reference-checksums.txt` is the capture manifest supplied with the references. The base C4 hash above was computed directly because the initial manifest listed only the alternate cases.

## Reproduction

From the repository root:

```bash
cargo run -p evil-synth --example phase6_render -- \
  --preset nano-303 \
  --output-dir target/phase6/nano303-after \
  --dump-patch target/phase6/nano303-after-patch.json
python3 -I tools/calibrate_nano303.py \
  --reference-dir target/phase6 \
  --native-dir target/phase6/nano303-after \
  --output target/phase6/nano303-after.json
```

The example uses the current built-in preset at 48 kHz, renders dry voices, and passes them through the shared stereo mixer with track gain 1, pan 0, master gain 1, and no channel inserts or sends. Instrument waveshaper/delay remain enabled. `--patch-json PATCH.json` renders an explicit candidate and `--cases c2,c4,soft` selects cases. The saved JSON patch is the exact final preset parameters.

The baseline report is `target/phase6/nano303-before.json`; final audio and report are in `target/phase6/nano303-after/` and `target/phase6/nano303-after.json`. For arbitrary files, use repeated `--compare LABEL REFERENCE.wav NATIVE.wav` arguments. `--sample-rate`, `--note-seconds`, and release/early/late windows are configurable, although the verified comparisons require 48 kHz analysis and a two-second gate.

`tools/calibrate_nano303.py` and `tools/calibrate_phase6_audio.py` require only NumPy and the standard library. They linearly resample each channel to 48 kHz, take the stereo mean, and report source checksums/format, RMS, peak, tuning, magnitude-weighted spectral centroid, six FFT energy bands, five time-window spectra, onset and active duration, release/early/late RMS, waveform correlation, and held/full-tail 10 ms RMS-envelope correlation and log-envelope RMSE. Activity uses 1 ms blocks above max(0.1% of peak block RMS, -80 dBFS). Tuning uses a zero-padded Hann FFT peak near the expected fundamental at 0.25–0.75 seconds. Full-tail envelopes zero-pad the shorter file. JSON contains basenames and measurements, not audio or timestamps, and is reproducible.

Glide uses a 100 ms Hann STFT every 10 ms, searches 230–360 Hz, and reports the trajectory plus 10/50/90% crossings. Soft/accent sections report RMS, peak, centroid, and per-band differences relative to the C4 control.

## Measured before/after results

Each native cell is **before → after (reference)**. RMS covers the two-second held note; centroid covers the same window; correlation covers the complete allocated/active tail.

| Case | Held RMS | Centroid Hz | Full-tail 10 ms envelope correlation |
| --- | ---: | ---: | ---: |
| C2 | 0.256443 → 0.272244 (0.296632) | 664 → 1,008 (958) | 0.9585 → 0.9692 |
| C3 | 0.256153 → 0.239094 (0.249190) | 1,368 → 1,757 (1,826) | 0.9798 → 0.9874 |
| C4 | 0.253786 → 0.206532 (0.208230) | 2,695 → 3,090 (3,328) | 0.9861 → 0.9951 |
| C5 | 0.243140 → 0.163667 (0.167684) | 4,962 → 5,454 (5,285) | 0.9886 → 0.9972 |
| Soft C4 | 0.223218 → 0.192189 (0.195290) | 1,288 → 2,789 (2,591) | 0.9850 → 0.9893 |
| Accent C4 | 0.281608 → 0.209413 (0.210246) | 2,806 → 3,108 (3,568) | 0.9874 → 0.9947 |
| Glide | 0.302074 → 0.206234 (0.202242) | 6,551 → 3,902 (2,416) | 0.9826 → 0.9928 |

C2–C5 RMS errors are now -8.2%, -4.1%, -0.8%, and -2.4%. C2–C5 centroid errors are +5.2%, -3.8%, -7.1%, and +3.2%. These are balanced multi-pitch improvements, not isolated C4 fitting.

Soft/control held-RMS ratio improves from 0.880 to 0.930 (reference 0.938). Accent/control improves from 1.110 to 1.014 (reference 1.010). Soft/control centroid ratio improves from 0.478 to 0.902 (reference 0.779); accent/control changes from 1.041 to 1.006 (reference 1.072), so the remaining accent brightness error is explicit.

Reference tuning is approximately +10.5 to +11.7 cents at C2–C5. The final oscillator's +11-cent calibration measures within about 1 cent of those references. The baseline was approximately 0 cents.

### Release and feedback tail

Each RMS cell is **before → after (reference)**. The old preset became silent after its 0.1-second release; the final preset has an actual measured tail.

| Case | Release 2–2.5 s RMS | Early 3–4 s RMS | Late 6–7 s RMS |
| --- | ---: | ---: | ---: |
| C2 | 0.168876 → 0.049932 (0.063253) | 0 → 0.002579 (0.007241) | 0 → 0.000070 (0.000313) |
| C3 | 0.173591 → 0.043858 (0.040486) | 0 → 0.002973 (0.002029) | 0 → 0.000080 (0.000078) |
| C4 | 0.175818 → 0.037938 (0.035653) | 0 → 0.003634 (0.002387) | 0 → 0.000098 (0.000078) |
| C5 | 0.171833 → 0.027025 (0.027265) | 0 → 0.001312 (0.002579) | 0 → 0.000036 (0.000095) |
| Soft | 0.139200 → 0.035581 (0.031312) | 0 → 0.002697 (0.002333) | 0 → 0.000073 (0.000075) |
| Accent | 0.197393 → 0.040685 (0.036759) | 0 → 0.004175 (0.002384) | 0 → 0.000112 (0.000078) |
| Glide | 0.187470 → 0.032672 (0.034020) | 0 → 0.002652 (0.003883) | 0 → 0.000072 (0.000150) |

The final delay is one sixteenth note (125 ms at 120 BPM), feedback 0.86, wet mix 0.04, stereo-cross mode, with an explicitly allocated six-second tail. Its `beats_per_cycle` follows the project tempo map. The optional `tail_seconds` field defaults to zero, preserving the legacy 16-tap allocation used by A17 and all other existing patches.

### Glide

The final 50 ms glide parameter produces a measured 10–90% transition of **0.040 s**, midpoint **1.030 s**, exactly matching the reference's quantized STFT measurements (baseline: 0.060 s and 1.040 s). Start/target frequencies are approximately 263.27/331.66 Hz native versus 263.17/331.29 Hz reference. The complete trajectory is in the JSON; 100 ms window smoothing and 10 ms hop resolution limit this comparison.

## DSP changes and rejected alternatives

- Corrected the normal/accent split: velocity 110 no longer triggers accent; only the verified velocity-127 case does.
- Added independent filter ADSR, calibrated 24 dB resonant low-pass cutoff/key tracking, +11-cent tuning, pitch-dependent gain, and mono 50 ms glide.
- Added opt-in `velocity_exponent` (legacy default 1) and `velocity_filter_octaves` (legacy default 0). Evil-303 uses near-constant amplitude with slight velocity-filter tracking and a small explicit accent gain.
- Added a calibrated drive-2 waveshaper at output level 0.52. Lower gain without that output trim was rejected because it removed harmonic energy; drive 1.5 was also spectrally worse. A no-drive candidate had acceptable level at some pitches but insufficient high-frequency energy.
- Rejected candidates that matched only C4, made soft response too dark, or improved peaks while losing multi-pitch RMS. The selected preset retains the best balanced measured result.

## Remaining differences and limitations

This is an original-synthesis recreation, **not sample equivalence, a 1:1 match, or copied NanoStudio audio**. Direct held-note waveform correlation remains low (approximately -0.02 to +0.05 across cases). Native peaks remain lower than references, and the native centered dual-mono path does not reproduce the references' small stereo difference.

C2 and C5 early/late delay tails remain too quiet, while C3/C4 and accent early tails are too strong; one feedback/wet configuration does not reproduce pitch-dependent source tail spectra. C2 RMS remains 8.2% low, soft centroid 7.7% high, accent centroid 12.9% low, and glide's whole-sequence centroid remains high despite matching transition timing. Reference onsets are approximately 2 ms; native onset is 0 ms. Exact nonlinear filter/drive topology, echo filtering, stereo behavior, and velocity curves remain unresolved. Only the listed 120 BPM fixtures were measured; beat-synced behavior at other tempos is tested as native DSP behavior, not reference parity.

## Verification

Run the focused native and integration checks from the repository root:

```bash
cargo test -p evil-synth
cargo test -p evil-render
cargo clippy -p evil-synth --all-targets -- -D warnings
cargo test --manifest-path compat/Cargo.toml -p evil-synth
cargo clippy --manifest-path compat/Cargo.toml -p evil-synth --all-targets -- -D warnings
```

The opt-in velocity, live-routing, and delay-tail tests verify deterministic behavior and unchanged legacy defaults. On 2026-11-05, all listed checks passed: 135 `evil-synth` tests in each workspace, 26 `evil-render` tests in each workspace, and strict clippy in both workspaces. Repeating the final analysis produced byte-identical JSON (SHA-256 `17965772fa01277fc993c256650f5665778571830313b167d44145f1af678075`). `cargo check --manifest-path compat/Cargo.toml -p oracle` also passed; no native project mapping changes were needed.
