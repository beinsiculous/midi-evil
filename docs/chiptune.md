# Chiptune drum kit (Phase 6)

## Scope and provenance

This is an **original deterministic synthesized approximation**, not a sample-identical recreation and not a redistribution of the factory bank. The only reference material was the user-owned local bank at:

`/home/jedi/.wine/drive_c/ProgramData/NanoStudio/Documents/User/TRG Banks/Chiptune.trg/`

That directory contains 16 mono PCM16 44.1 kHz WAVs and `Package.trg`. The files were read only for measurement. No factory audio, sample arrays, or NanoStudio format dependency is present in `evil-beats`; all native samples are generated from oscillators, seeded xorshift32 noise, filters, envelopes, soft clipping, and 8-bit-style quantization.

`Package.trg` SHA-256: `eb748205190905291def84afe5db38bcca1f7df3a4efc366ede1126e14bed523`.

The package maps the pads in the order below. Every pad has volume 0.5, pan 0.5, transpose 0, fine pitch 0, output bus 0, full-source forward triggers, and a zero-attack/zero-decay/unity-sustain/zero-release amp envelope. Pads 2 and 3 alone share choke group 1. The native kit preserves the identities and order, centered pan, zero tuning, full-source forward playback, default unity-sustain envelope with zero release, disabled filters/sends, and hat choke group 1.

### Source identities and checksums

| Pad | Source file | SHA-256 |
| ---: | --- | --- |
| 0 | BD 8b.wav | `edd55a1e45bc1e57811625bb6d0858ecf7bf637559919ebb086471192f459052` |
| 1 | Snare 8b.wav | `4f435fe0b51a93e2ad9254c2c6d6a3e6cfa2edf5739728280ef7fbcd9bd25534` |
| 2 | HH-CL 8b.wav | `4c712c315ef557df64d1b9f60dbbdaa4071b77e94c171357c781e21ca5b19ab3` |
| 3 | HH-OP 8b.wav | `a90ec96767632b345f49b7721b9ab3bd91dccc9252e2b3a11d3fdc0abc967c29` |
| 4 | CB 8b.wav | `ae77157e7a53e0db772911ae7987fd3f5b02d4b88b6ff78430bad94924a3e1db` |
| 5 | TomHi 8b.wav | `02ece6e2875ffd224c22b38e0342179df3d8b30bf3c7891a5a23854917e95a5e` |
| 6 | TomLo 8b.wav | `b127b6dd8e203d2717f9c90f6a84ee50588fd1f2e7aaf091fce7fd64c3c3115b` |
| 7 | Rim 8b.wav | `f52a3e57a15ff2b2e1bec9b42595241724c606274ed2c40edb94ac03a72a4c53` |
| 8 | Ride 8b.wav | `5b2c6b9a54debbe999fcbdfba2899c4d926e646a19d3a078b5005533c8ea48b8` |
| 9 | Tamb 8b.wav | `28e936888199613e4a35674699185828924037776e6c55e76053bf9fa5a20bbf` |
| 10 | Crash 8b.wav | `56e563bb419d8fc1c33c52001863dc26d60d0efa9d6e275d97191fece1cbbec5` |
| 11 | Clap 8b.wav | `028ece3cb3ff1cc36890c665136e1afbae59a2c86b2b45f1b7475e470fb0613d` |
| 12 | BeepHi 8b.wav | `e171fd3258245bb80e29bdc9c4654e0fc3bbf3fc78a3b315ed20d1d90766de64` |
| 13 | BeepLo 8b.wav | `85e71cfe65f1c990b5e7d3695452b59898d6c90273df9b58b181f7067add0af7` |
| 14 | CL Hi 8b.wav | `3eaff026ce0cfeac09c40db204d7881d153cc1b8a1c85a9b6938b8cf7a3623d2` |
| 15 | CL Lo 8b.wav | `f6e59df76e78683afe749fff38b464c0c5a8680c1707a84a0d55eab04bf25c27` |

## Reproducible measurement

Run from the repository root, substituting the local source directory:

```bash
python3 -I tools/calibrate_chiptune.py SOURCE_DIRECTORY target/chiptune/source-measurements.json
```

The script requires Python 3 and NumPy, not SciPy. Its JSON contains no timestamps or absolute paths. It validates mono PCM16 44.1 kHz input, reads all 16 files named by `Package.trg`, and records source/package checksums and settings. Amplitudes are normalized by 32768. Onset is the first absolute sample at least max(1% of peak, one LSB). Attack is onset to absolute peak. Decay uses non-overlapping approximately 1 ms RMS blocks: `T10` and `T1` are times from onset to the first post-peak block below 10% and 1% of peak block RMS. These are first-crossing measurements, not sustained-below tests.

Spectra use a Hann-windowed FFT from onset to end. The table gives dominant bin, power centroid, and 85% power rolloff. The JSON additionally records zero-crossing rate, crest factor, exact frame counts, and energy fractions for 0–100, 100–300, 300–1,000, 1,000–3,000, 3,000–8,000, and 8,000–22,050 Hz. Autocorrelation pitch uses the first 100 ms, 40–2,000 Hz lag range, and a 0.25 correlation threshold; it is exploratory and can be implausible for noise or fast pitch sweeps. In particular, the BD and TomLo autocorrelation results are not tuning targets; their spectral fundamentals are more useful.

### Per-pad source measurements

Times are seconds; frequencies are Hz. `F0` is autocorrelation and `Dom` is the FFT dominant bin.

| Pad | Duration | RMS | Peak | Onset | Attack | T10 | T1 | F0 | Dom | Centroid | Rolloff 85% |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0.1429 | 0.171208 | 1.000000 | 0.004966 | 0.007959 | 0.0150 | 0.0230 | 2004.5 | 72.5 | 109 | 159 |
| 1 | 0.2010 | 0.152063 | 1.000000 | 0.000091 | 0.001315 | 0.1436 | 0.1665 | – | 5.0 | 10213 | 17964 |
| 2 | 0.1878 | 0.125504 | 0.998779 | 0.000181 | 0.000952 | 0.0487 | 0.0647 | 1764.0 | 10.7 | 2056 | 6995 |
| 3 | 0.2482 | 0.189112 | 0.999969 | 0.000181 | 0.000295 | 0.2273 | 0.2303 | 436.6 | 6121.4 | 8620 | 12682 |
| 4 | 0.2515 | 0.089390 | 1.000000 | 0.000023 | 0.001066 | 0.0259 | 0.2085 | 1764.0 | 1769.7 | 3234 | 3543 |
| 5 | 0.1520 | 0.235845 | 1.000000 | 0.000136 | 0.001111 | 0.0787 | 0.1076 | 218.3 | 217.3 | 367 | 224 |
| 6 | 0.0921 | 0.262306 | 1.000000 | 0.000091 | 0.001293 | 0.0298 | 0.0438 | 2004.5 | 108.7 | 166 | 120 |
| 7 | 0.1185 | 0.106090 | 1.000000 | 0.000068 | 0.001701 | 0.0119 | 0.0798 | 207.0 | 447.7 | 794 | 1056 |
| 8 | 0.2101 | 0.125838 | 1.000000 | 0.000045 | 0.001814 | 0.0538 | 0.1955 | 848.1 | 5061.4 | 5014 | 6866 |
| 9 | 0.0441 | 0.261338 | 1.000000 | 0.000023 | 0.005601 | 0.0419 | 0.0429 | 848.1 | 5038.7 | 6039 | 6877 |
| 10 | 0.7793 | 0.137535 | 0.999969 | 0.000227 | 0.136485 | 0.5216 | 0.7261 | 1520.7 | 4667.0 | 4495 | 6040 |
| 11 | 0.0561 | 0.222463 | 1.000000 | 0.000340 | 0.002222 | 0.0525 | 0.0545 | – | 4569.5 | 4922 | 7275 |
| 12 | 0.2327 | 0.111868 | 0.999969 | 0.000023 | 0.005057 | 0.1287 | 0.1786 | 350.0 | 348.1 | 769 | 357 |
| 13 | 0.3995 | 0.102618 | 0.999969 | 0.000045 | 0.016916 | 0.0189 | 0.1546 | 87.5 | 87.6 | 238 | 325 |
| 14 | 0.0610 | 0.240062 | 1.000000 | 0.000091 | 0.001315 | 0.0199 | 0.0229 | 1575.0 | 16.4 | 1739 | 1608 |
| 15 | 0.1509 | 0.221267 | 1.000000 | 0.000023 | 0.002449 | 0.0359 | 0.0898 | 787.5 | 788.6 | 1185 | 815 |

Low FFT bins dominate some very short or noise-like sources (Snare, HH-CL, CL Hi/Lo). They include window/DC/low-frequency effects and should not be interpreted as the audible fundamental. The native hats intentionally emphasize the audible metallic/high-frequency content rather than recreating those FFT artifacts.

## Original native synthesis

`chiptune_kit()` is alongside `maxwell_kit()` in `crates/evil-beats/src/presets.rs`; Maxwell is unchanged. Each pad has an independent fixed seed and exact source-matching frame count. The generator uses measured RMS as a level target, peak-limiting, and final 8-bit-style quantization. It needs no files at runtime.

| Pad | Native synthesis approach |
| ---: | --- |
| 0 BD | 130-to-62 Hz swept sine/pulse body, short noise click, shaped fast decay |
| 1 Snare | Seeded 3.5–16 kHz noise, 185 Hz pulse and 1,796 Hz ring, long noise decay |
| 2 HH-CL | High-passed seeded noise plus 5.25/7.00/8.75 kHz pulses, short gate |
| 3 HH-OP | High-passed noise plus five 4.37–11.37 kHz pulses, longer gate |
| 4 CB | 1,769.7 Hz pulse/sine cowbell with a quiet zero-mean residual tone |
| 5 TomHi | 260-to-217.3 Hz swept pulse/triangle, measured medium decay |
| 6 TomLo | 145-to-108.7 Hz swept pulse/triangle, short decay |
| 7 Rim | 447.7, 548.7, and 818.9 Hz resonances plus noise click |
| 8 Ride | 5.06/5.13/6.87 kHz metal, 600 Hz body, high-passed noise |
| 9 Tamb | Ride-like short metal/noise burst with 567 Hz body |
| 10 Crash | 2.18–4.67 kHz pulse cluster, high-passed noise, damped long decay |
| 11 Clap | Three noise bursts plus 1.41/1.85/3.10/4.88 kHz accents |
| 12 BeepHi | 350 Hz pulse/sine beep with measured decay and gate |
| 13 BeepLo | 87.5 Hz pulse/sine beep, fast main decay and quiet residual |
| 14 CL Hi | 1,575 Hz pulse/sine clave, very short gate |
| 15 CL Lo | 787.5 Hz pulse/sine clave, short body and quiet residual |

### Native approximation measurements

These rows are **native synthesized audio**, not additional source measurements. Each cell is source → native. Regenerate after the artifact step below with:

```bash
python3 -I tools/calibrate_chiptune.py SOURCE_DIRECTORY target/chiptune/comparison.json \
  --native-dir target/chiptune/pads
```

The exhaustive native checksums, formats, onset/attack, band energies, rolloffs, and pitch estimates are in `target/chiptune/comparison.json`.

| Pad | RMS | Peak | T10 (s) | T1 (s) | Dominant (Hz) | Centroid (Hz) |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 0.171 → 0.171 | 1.000 → 0.866 | 0.015 → 0.019 | 0.023 → 0.027 | 72.5 → 83.9 | 109 → 105 |
| 1 | 0.152 → 0.152 | 1.000 → 0.795 | 0.144 → 0.135 | 0.167 → 0.167 | 5.0 → 1796.0 | 10213 → 9958 |
| 2 | 0.126 → 0.126 | 0.999 → 0.905 | 0.049 → 0.046 | 0.065 → 0.065 | 10.7 → 6998.5 | 2056 → 9966 |
| 3 | 0.189 → 0.189 | 1.000 → 0.835 | 0.227 → 0.228 | 0.230 → 0.230 | 6121.4 → 6121.0 | 8620 → 7943 |
| 4 | 0.089 → 0.089 | 1.000 → 0.638 | 0.026 → 0.029 | 0.209 → 0.208 | 1769.7 → 1769.6 | 3234 → 2722 |
| 5 | 0.236 → 0.236 | 1.000 → 0.890 | 0.079 → 0.082 | 0.108 → 0.108 | 217.3 → 217.1 | 367 → 552 |
| 6 | 0.262 → 0.226 | 1.000 → 0.992 | 0.030 → 0.028 | 0.044 → 0.044 | 108.7 → 119.5 | 166 → 331 |
| 7 | 0.106 → 0.106 | 1.000 → 0.701 | 0.012 → 0.013 | 0.080 → 0.053 | 447.7 → 447.4 | 794 → 644 |
| 8 | 0.126 → 0.101 | 1.000 → 0.992 | 0.054 → 0.049 | 0.196 → 0.196 | 5061.4 → 5060.3 | 5014 → 4412 |
| 9 | 0.261 → 0.261 | 1.000 → 0.937 | 0.042 → 0.043 | 0.043 → 0.043 | 5038.7 → 5058.8 | 6039 → 5944 |
| 10 | 0.138 → 0.138 | 1.000 → 0.898 | 0.522 → 0.545 | 0.726 → 0.726 | 4667.0 → 4666.9 | 4495 → 5092 |
| 11 | 0.222 → 0.180 | 1.000 → 0.992 | 0.053 → 0.054 | 0.055 → 0.055 | 4569.5 → 4880.2 | 4922 → 5056 |
| 12 | 0.112 → 0.112 | 1.000 → 0.331 | 0.129 → 0.136 | 0.179 → 0.179 | 348.1 → 348.0 | 769 → 721 |
| 13 | 0.103 → 0.103 | 1.000 → 0.898 | 0.019 → 0.022 | 0.155 → 0.155 | 87.6 → 87.6 | 238 → 245 |
| 14 | 0.240 → 0.240 | 1.000 → 0.992 | 0.020 → 0.019 | 0.023 → 0.023 | 16.4 → 1572.7 | 1739 → 2522 |
| 15 | 0.221 → 0.211 | 1.000 → 0.992 | 0.036 → 0.036 | 0.090 → 0.084 | 788.6 → 788.4 | 1185 → 1429 |

The remaining differences are deliberate approximation limits, especially transient peak shape, low-level residual decays, the very bright closed-hat spectrum, and tom/clave harmonic balance. No sample-match claim is made.

## Local artifacts and dense pattern

Generate the native-only artifacts without changing the public crate root:

```bash
cargo test -p evil-beats presets::tests::write_chiptune_comparison_artifacts -- --ignored
```

The ignored test uses the kit serializer, public stereo WAV writer, and `render_kit` to write:

- `target/chiptune/chiptune.evilkit`: portable native kit containing only original synthesized audio.
- `target/chiptune/pads/pad-00.wav` through `pad-15.wav`: mono PCM16 native pad measurements.
- `target/chiptune/dense-native.wav`: 256-hit, 44.1 kHz stereo native render with deterministic overlapping hits.
- `target/chiptune/dense-pattern.json`: exact hit list, unclipped floating-point mix peak, WAV normalization scale, and provenance note.
- `target/chiptune/source-measurements.json` and `comparison.json`: reproducible source-only and source/native-separate reports.

The dense float render peaks at 1.71140099 and is normalized by 0.57847345 only when written to PCM16; the scale is recorded in `dense-pattern.json`. **No matching NanoStudio dense-pattern mixdown exists. `dense-native.wav` is not a comparative mixdown.** A residual capture need is a user-owned reference render of the exact 256-hit pattern in `dense-pattern.json`; only after that capture should dense source/native comparisons be claimed.

## Automated coverage

The preset tests cover all 16 non-empty pads, exact identities and playback settings, kit validation, centered pan/zero tuning, hat choke group 1, disabled filters/sends, finite bounded samples, full-kit and dense-render reproducibility, source-path independence, and 256 dense overlapping hits through `render_kit`. The artifact writer is explicitly ignored during normal tests so normal test runs do not write repository artifacts.
