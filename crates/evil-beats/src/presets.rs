use evil_synth::{Envelope, Filter, FilterMode, Oscillator, Patch, Waveform, render};

use crate::{DrumKit, DrumPad, Error, KitPad};

/// Original deterministic synthesized percussion. No third-party samples.
pub fn maxwell_kit() -> Result<DrumKit, Error> {
    use Waveform::*;
    let specs = [
        ("Reactor Kick", Sine, 160.0, 40.0, 0.38, 0.04),
        ("Lab Snare", Triangle, 220.0, 140.0, 0.20, 0.8),
        ("Closed Hat", Square, 6500.0, 6200.0, 0.055, 1.0),
        ("Open Hat", Square, 6700.0, 6000.0, 0.42, 1.0),
        ("Low Tom", Sine, 150.0, 65.0, 0.35, 0.03),
        ("Mid Tom", Sine, 230.0, 100.0, 0.28, 0.04),
        ("High Tom", Sine, 340.0, 160.0, 0.23, 0.04),
        ("Clap Trap", Noise, 1000.0, 1000.0, 0.16, 0.0),
        ("Rim Circuit", Triangle, 1700.0, 1000.0, 0.035, 0.25),
        ("Cowbell", Square, 560.0, 560.0, 0.18, 0.0),
        ("Shaker", Noise, 8000.0, 8000.0, 0.11, 0.0),
        ("Tambourine", Square, 4200.0, 4100.0, 0.22, 0.7),
        ("Bit Click", Square, 1500.0, 400.0, 0.045, 0.0),
        ("Laser Zap", Sine, 2200.0, 80.0, 0.27, 0.0),
        ("Reverse Ion", Triangle, 100.0, 1600.0, 0.32, 0.15),
        ("Crash Test", Square, 7200.0, 7100.0, 1.1, 1.0),
    ];
    let mut kit = DrumKit::empty("Maxwell / Lab Kit");
    for (index, (name, waveform, start, end, duration, noise)) in specs.into_iter().enumerate() {
        let mut oscillators = vec![Oscillator {
            waveform,
            start_frequency_hz: start,
            end_frequency_hz: end,
            amplitude: if noise > 0.5 { 0.22 } else { 0.8 },
            ..Oscillator::default()
        }];
        if noise > 0.0 {
            oscillators.push(Oscillator {
                waveform: Noise,
                amplitude: noise,
                ..Oscillator::default()
            });
        }
        if index == 9 {
            oscillators.push(Oscillator {
                waveform: Square,
                start_frequency_hz: 845.0,
                end_frequency_hz: 845.0,
                amplitude: 0.4,
                ..Oscillator::default()
            });
        }
        let mut patch = Patch {
            name: name.into(),
            duration_seconds: duration,
            oscillators,
            seed: 0x4d41_5800 + index as u32,
            gain: 0.75,
            envelope: Envelope {
                attack_seconds: 0.001,
                decay_seconds: duration,
                sustain_level: 0.0,
                release_seconds: 0.02,
            },
            ..Patch::default()
        };
        if [2, 3, 7, 10, 11, 15].contains(&index) {
            patch.filter = Some(Filter {
                mode: FilterMode::HighPass,
                cutoff_hz: if index == 7 { 900.0 } else { 4000.0 },
                ..Filter::default()
            });
        }
        let mut sound = render(&patch)?;
        if index == 7 {
            for (frame, sample) in sound.samples.iter_mut().enumerate() {
                let time = frame as f32 / sound.sample_rate as f32;
                if time < 0.045 {
                    *sample *= (1.0 - (time % 0.015) / 0.015).powi(2);
                }
            }
        }
        if index == 14 {
            sound.samples.reverse();
        }
        let mut voice = DrumPad::new(sound.samples, sound.sample_rate);
        voice.name = name.into();
        kit.pads[index] = Some(KitPad::new(voice));
    }
    Ok(kit)
}

const CHIP_SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy)]
enum ChipKind {
    Kick,
    Snare,
    Hat {
        open: bool,
    },
    Cowbell,
    Tom {
        start_hz: f32,
        end_hz: f32,
        decay: f32,
        tail: f32,
    },
    Rim,
    Ride,
    Tambourine,
    Crash,
    Clap,
    Beep {
        frequency_hz: f32,
        decay: f32,
        tail: f32,
        residual: f32,
    },
    Clave {
        frequency_hz: f32,
        decay: f32,
        tail: f32,
        residual: f32,
    },
}

struct ChipSpec {
    name: &'static str,
    frames: usize,
    target_rms: f32,
    seed: u32,
    kind: ChipKind,
}

// Durations, pitches, envelopes, levels and spectra are original synthesis
// parameters guided by local measurements. No factory sample data is embedded.
const CHIP_SPECS: [ChipSpec; 16] = [
    ChipSpec {
        name: "BD 8b",
        frames: 6_304,
        target_rms: 0.171_208,
        seed: 0x4348_0000,
        kind: ChipKind::Kick,
    },
    ChipSpec {
        name: "Snare 8b",
        frames: 8_864,
        target_rms: 0.152_063,
        seed: 0x4348_0001,
        kind: ChipKind::Snare,
    },
    ChipSpec {
        name: "HH-CL 8b",
        frames: 8_280,
        target_rms: 0.125_504,
        seed: 0x4348_0002,
        kind: ChipKind::Hat { open: false },
    },
    ChipSpec {
        name: "HH-OP 8b",
        frames: 10_944,
        target_rms: 0.189_112,
        seed: 0x4348_0003,
        kind: ChipKind::Hat { open: true },
    },
    ChipSpec {
        name: "CB 8b",
        frames: 11_090,
        target_rms: 0.089_390,
        seed: 0x4348_0004,
        kind: ChipKind::Cowbell,
    },
    ChipSpec {
        name: "TomHi 8b",
        frames: 6_704,
        target_rms: 0.235_845,
        seed: 0x4348_0005,
        kind: ChipKind::Tom {
            start_hz: 260.0,
            end_hz: 217.3,
            decay: 0.041,
            tail: 0.108,
        },
    },
    ChipSpec {
        name: "TomLo 8b",
        frames: 4_060,
        target_rms: 0.262_306,
        seed: 0x4348_0006,
        kind: ChipKind::Tom {
            start_hz: 145.0,
            end_hz: 108.7,
            decay: 0.0125,
            tail: 0.044,
        },
    },
    ChipSpec {
        name: "Rim 8b",
        frames: 5_224,
        target_rms: 0.106_090,
        seed: 0x4348_0007,
        kind: ChipKind::Rim,
    },
    ChipSpec {
        name: "Ride 8b",
        frames: 9_264,
        target_rms: 0.125_838,
        seed: 0x4348_0008,
        kind: ChipKind::Ride,
    },
    ChipSpec {
        name: "Tamb 8b",
        frames: 1_944,
        target_rms: 0.261_338,
        seed: 0x4348_0009,
        kind: ChipKind::Tambourine,
    },
    ChipSpec {
        name: "Crash 8b",
        frames: 34_368,
        target_rms: 0.137_535,
        seed: 0x4348_000a,
        kind: ChipKind::Crash,
    },
    ChipSpec {
        name: "Clap 8b",
        frames: 2_476,
        target_rms: 0.222_463,
        seed: 0x4348_000b,
        kind: ChipKind::Clap,
    },
    ChipSpec {
        name: "BeepHi 8b",
        frames: 10_264,
        target_rms: 0.111_868,
        seed: 0x4348_000c,
        kind: ChipKind::Beep {
            frequency_hz: 350.0,
            decay: 0.060,
            tail: 0.179,
            residual: 0.0,
        },
    },
    ChipSpec {
        name: "BeepLo 8b",
        frames: 17_616,
        target_rms: 0.102_618,
        seed: 0x4348_000d,
        kind: ChipKind::Beep {
            frequency_hz: 87.5,
            decay: 0.008,
            tail: 0.155,
            residual: 0.025,
        },
    },
    ChipSpec {
        name: "CL Hi 8b",
        frames: 2_692,
        target_rms: 0.240_062,
        seed: 0x4348_000e,
        kind: ChipKind::Clave {
            frequency_hz: 1_575.0,
            decay: 0.008,
            tail: 0.023,
            residual: 0.0,
        },
    },
    ChipSpec {
        name: "CL Lo 8b",
        frames: 6_656,
        target_rms: 0.221_267,
        seed: 0x4348_000f,
        kind: ChipKind::Clave {
            frequency_hz: 787.5,
            decay: 0.015,
            tail: 0.090,
            residual: 0.008,
        },
    },
];

struct ChipNoise(u32);

impl ChipNoise {
    fn next(&mut self) -> f32 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        self.0 = state;
        (state >> 8) as f32 / 8_388_607.5 - 1.0
    }
}

fn sine(phase: f32) -> f32 {
    (phase * std::f32::consts::TAU).sin()
}

fn pulse(phase: f32, width: f32) -> f32 {
    if phase.fract() < width { 1.0 } else { -1.0 }
}

fn triangle(phase: f32) -> f32 {
    1.0 - 4.0 * (phase.fract() - 0.5).abs()
}

fn swept_phase(time: f32, start_hz: f32, end_hz: f32, sweep_seconds: f32) -> f32 {
    end_hz * time + (start_hz - end_hz) * sweep_seconds * (1.0 - (-time / sweep_seconds).exp())
}

fn end_gate(time: f32, tail: f32) -> f32 {
    ((tail - time) / 0.002).clamp(0.0, 1.0)
}

fn high_pass(samples: &mut [f32], cutoff_hz: f32) {
    let dt = 1.0 / CHIP_SAMPLE_RATE as f32;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    let alpha = rc / (rc + dt);
    let mut previous_input = 0.0;
    let mut previous_output = 0.0;
    for sample in samples {
        let output = alpha * (previous_output + *sample - previous_input);
        previous_input = *sample;
        previous_output = output;
        *sample = output;
    }
}

fn low_pass(samples: &mut [f32], cutoff_hz: f32) {
    let dt = 1.0 / CHIP_SAMPLE_RATE as f32;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    let alpha = dt / (rc + dt);
    let mut previous_output = 0.0;
    for sample in samples {
        previous_output += alpha * (*sample - previous_output);
        *sample = previous_output;
    }
}

fn synthesize_chip(spec: &ChipSpec) -> Vec<f32> {
    let mut noise = ChipNoise(spec.seed);
    let mut noise: Vec<f32> = (0..spec.frames).map(|_| noise.next()).collect();
    let mut samples = vec![0.0; spec.frames];
    for (frame, sample) in samples.iter_mut().enumerate() {
        let time = frame as f32 / CHIP_SAMPLE_RATE as f32;
        *sample = match spec.kind {
            ChipKind::Kick => {
                let phase = swept_phase(time, 130.0, 62.0, 0.012);
                let body = sine(phase) + 0.25 * pulse(phase, 0.5);
                let envelope = (-(time / 0.009).powf(1.4)).exp();
                body * envelope + 0.16 * noise[frame] * (-time / 0.001).exp()
            }
            ChipKind::Snare => {
                if frame == 0 {
                    high_pass(&mut noise, 3_500.0);
                    low_pass(&mut noise, 16_000.0);
                }
                let envelope = (-time / 0.060).exp() * end_gate(time, 0.167);
                (0.85 * noise[frame]
                    + 0.03 * pulse(185.0 * time, 0.5)
                    + 0.08 * sine(1_796.0 * time))
                    * envelope
            }
            ChipKind::Hat { open } => {
                if frame == 0 {
                    high_pass(&mut noise, if open { 4_000.0 } else { 6_500.0 });
                }
                let (decay, tail, level) = if open {
                    (0.120, 0.230, 0.42)
                } else {
                    (0.020, 0.065, 0.65)
                };
                let metallic = if open {
                    0.18 * pulse(6_121.0 * time, 0.5)
                        + 0.14 * pulse(7_869.8 * time, 0.5)
                        + 0.12 * pulse(9_618.7 * time, 0.5)
                        + 0.12 * pulse(4_372.1 * time, 0.5)
                        + 0.08 * pulse(11_367.5 * time, 0.5)
                } else {
                    0.30 * pulse(6_998.5 * time, 0.5)
                        + 0.10 * pulse(8_745.4 * time, 0.5)
                        + 0.08 * pulse(5_246.2 * time, 0.5)
                };
                (level * noise[frame] + metallic) * (-time / decay).exp() * end_gate(time, tail)
            }
            ChipKind::Cowbell => {
                (pulse(1_769.7 * time, 0.25) + 0.20 * sine(1_769.7 * time))
                    * (-time / 0.012).exp()
                    * end_gate(time, 0.209)
                    + 0.012 * sine(1_769.7 * time) * end_gate(time, 0.209)
            }
            ChipKind::Tom {
                start_hz,
                end_hz,
                decay,
                tail,
            } => {
                let phase = swept_phase(time, start_hz, end_hz, 0.018);
                (pulse(phase, 0.5) + 0.50 * triangle(phase) + 0.03 * noise[frame])
                    * (-time / decay).exp()
                    * end_gate(time, tail)
            }
            ChipKind::Rim => {
                let envelope = (-time / 0.005).exp() + 0.020;
                (sine(447.7 * time)
                    + 0.70 * triangle(818.9 * time)
                    + 0.55 * sine(548.7 * time)
                    + 0.20 * noise[frame] * (-time / 0.001).exp())
                    * envelope
                    * end_gate(time, 0.080)
            }
            ChipKind::Ride | ChipKind::Tambourine | ChipKind::Crash => {
                if frame == 0 {
                    high_pass(
                        &mut noise,
                        if matches!(spec.kind, ChipKind::Crash) {
                            2_000.0
                        } else {
                            4_000.0
                        },
                    );
                }
                let (decay, tail, residual, noise_level) = match spec.kind {
                    ChipKind::Ride => (0.022, 0.196, 0.020, 0.35),
                    ChipKind::Tambourine => (0.060, 0.043, 0.0, 0.55),
                    ChipKind::Crash => (0.300, 0.727, 0.0, 0.65),
                    _ => unreachable!(),
                };
                let metallic = match spec.kind {
                    ChipKind::Crash => {
                        0.20 * pulse(4_666.9 * time, 0.5)
                            + 0.18 * pulse(4_506.5 * time, 0.5)
                            + 0.14 * pulse(2_175.0 * time, 0.5)
                            + 0.12 * pulse(3_359.3 * time, 0.5)
                    }
                    _ => {
                        0.24 * pulse(5_060.3 * time, 0.5)
                            + 0.18 * pulse(5_126.9 * time, 0.5)
                            + 0.12 * pulse(6_869.2 * time, 0.5)
                    }
                };
                let low = if matches!(spec.kind, ChipKind::Ride) {
                    0.25 * triangle(600.0 * time)
                } else if matches!(spec.kind, ChipKind::Tambourine) {
                    0.12 * triangle(567.1 * time)
                } else {
                    0.0
                };
                (noise_level * noise[frame] + metallic + low)
                    * ((-time / decay).exp() + residual)
                    * end_gate(time, tail)
            }
            ChipKind::Clap => {
                if frame == 0 {
                    high_pass(&mut noise, 1_400.0);
                    low_pass(&mut noise, 9_000.0);
                }
                let bursts = [0.0, 0.011, 0.022]
                    .into_iter()
                    .map(|offset| {
                        let local = time - offset;
                        if local >= 0.0 {
                            (-local / 0.004).exp()
                        } else {
                            0.0
                        }
                    })
                    .sum::<f32>();
                let tones = 0.01 * sine(1_407.1 * time)
                    + 0.015 * sine(1_852.3 * time)
                    + 0.025 * sine(3_099.1 * time)
                    + 0.03 * sine(4_880.2 * time);
                (0.55 * noise[frame] * (bursts + 0.35 * (-time / 0.035).exp()) + tones)
                    * end_gate(time, 0.0545)
            }
            ChipKind::Beep {
                frequency_hz,
                decay,
                tail,
                residual,
            } => {
                (pulse(frequency_hz * time, 0.35) + 0.20 * sine(frequency_hz * time))
                    * ((-time / decay).exp() + residual)
                    * end_gate(time, tail)
            }
            ChipKind::Clave {
                frequency_hz,
                decay,
                tail,
                residual,
            } => {
                (pulse(frequency_hz * time, 0.5) + 0.25 * sine(frequency_hz * time))
                    * ((-time / decay).exp() + residual)
                    * end_gate(time, tail)
            }
        };
    }

    match spec.kind {
        ChipKind::Crash => low_pass(&mut samples, 5_000.0),
        ChipKind::Ride | ChipKind::Tambourine => low_pass(&mut samples, 6_500.0),
        ChipKind::Clap => low_pass(&mut samples, 7_500.0),
        ChipKind::Hat { open: true } => low_pass(&mut samples, 15_000.0),
        _ => {}
    }
    let drive = match spec.kind {
        ChipKind::Kick => 1.4,
        ChipKind::Hat { open: true } => 1.8,
        ChipKind::Hat { open: false } => 1.3,
        ChipKind::Rim => 1.6,
        ChipKind::Clap => 1.5,
        _ => 1.0,
    };
    if drive != 1.0 {
        for sample in &mut samples {
            *sample = (*sample * drive).tanh();
        }
    }

    let peak = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if peak > 0.0 {
        for sample in &mut samples {
            *sample *= 0.98 / peak;
        }
    }
    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    if rms > 0.0 {
        let scale = (spec.target_rms / rms).min(0.99 / 0.98);
        for sample in &mut samples {
            *sample = (*sample * scale).clamp(-0.99, 0.99);
        }
    }
    for sample in &mut samples {
        *sample = (*sample * 127.0).round() / 127.0;
    }
    samples
}

/// Original deterministic 8-bit-style percussion guided by measured Chiptune
/// character. All audio is synthesized locally; no reference WAVs are required.
pub fn chiptune_kit() -> Result<DrumKit, Error> {
    let mut kit = DrumKit::empty("MIDI Evil / Chiptune");
    for (index, spec) in CHIP_SPECS.iter().enumerate() {
        let mut voice = DrumPad::new(synthesize_chip(spec), CHIP_SAMPLE_RATE);
        voice.name = spec.name.into();
        voice.envelope.release_seconds = 0.0;
        let mut pad = KitPad::new(voice);
        if matches!(spec.kind, ChipKind::Hat { .. }) {
            pad.choke_group = 1;
        }
        kit.pads[index] = Some(pad);
    }
    kit.validate()?;
    Ok(kit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use sha2::{Digest, Sha256};

    const TICKS_PER_SECOND: f64 = 512.0;

    fn dense_start_tick(index: usize) -> u32 {
        (index as f64 * 1.536).round() as u32
    }

    fn dense_chiptune_hits() -> Vec<crate::KitHit> {
        (0..256)
            .map(|index| crate::KitHit {
                pad_index: (index * 7 + index / 16) % 16,
                start_seconds: f64::from(dense_start_tick(index)) / TICKS_PER_SECOND,
                duration_seconds: None,
                velocity: 72 + (index % 56) as u8,
            })
            .collect()
    }

    fn below_limit_chiptune_hits() -> Vec<crate::KitHit> {
        dense_chiptune_hits()
            .into_iter()
            .enumerate()
            .map(|(index, mut hit)| {
                hit.start_seconds = index as f64 * 64.0 / TICKS_PER_SECOND;
                hit
            })
            .collect()
    }

    #[test]
    fn chiptune_kit_is_original_deterministic_complete_and_source_independent() {
        let kit = chiptune_kit().unwrap();
        assert_eq!(kit, chiptune_kit().unwrap());
        kit.validate().unwrap();
        assert!(kit.pads.iter().all(Option::is_some));
        assert!(kit.buses.iter().all(|bus| bus.filter.is_none()));
        for (index, (pad, spec)) in kit.pads.iter().flatten().zip(&CHIP_SPECS).enumerate() {
            assert_eq!(pad.voice.name, spec.name);
            assert_eq!(pad.voice.samples.len(), spec.frames);
            assert!(pad.voice.samples.iter().any(|sample| sample.abs() > 0.001));
            assert!(
                pad.voice
                    .samples
                    .iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
            );
            assert_eq!(pad.voice.sample_rate, CHIP_SAMPLE_RATE);
            assert_eq!(pad.voice.tuning_semitones, 0.0);
            assert_eq!(pad.pan, 0.0);
            assert_eq!(pad.choke_group, u8::from(matches!(index, 2 | 3)));
            assert_eq!(pad.bus_sends, [0.0; 3]);
            assert_eq!(pad.voice.filter, None);
            assert_eq!(pad.voice.playback_range(), 0..spec.frames);
            assert_eq!(pad.voice.direction, crate::SampleDirection::Forward);
            assert!(!pad.voice.loop_enabled);
            assert_eq!(pad.voice.envelope.attack_seconds, 0.0);
            assert_eq!(pad.voice.envelope.decay_seconds, 0.0);
            assert_eq!(pad.voice.envelope.sustain_level, 1.0);
            assert_eq!(pad.voice.envelope.release_seconds, 0.0);
        }
        let serialized = kit.to_json().unwrap();
        assert!(!serialized.contains(concat!(".", "wav")));
        let source = include_str!("presets.rs");
        for needle in [
            concat!(".", "wine"),
            concat!("Program", "Data"),
            concat!("TRG ", "Banks"),
            concat!("/", "home", "/"),
            concat!("Package", ".trg"),
        ] {
            assert!(!serialized.contains(needle));
            assert!(!source.contains(needle));
        }
    }

    #[test]
    fn chiptune_dense_overlapping_hits_are_bounded_finite_and_reproducible() {
        let kit = chiptune_kit().unwrap();
        let hits = dense_chiptune_hits();
        let first = crate::render_kit(&kit, &hits, CHIP_SAMPLE_RATE).unwrap();
        let second = crate::render_kit(&kit, &hits, CHIP_SAMPLE_RATE).unwrap();
        assert_eq!(first.sample_rate, second.sample_rate);
        assert_eq!(first.left, second.left);
        assert_eq!(first.right, second.right);
        assert!(first.left.len() > CHIP_SAMPLE_RATE as usize);
        assert!(first.left.iter().any(|sample| sample.abs() > 0.001));
        assert!(first.left.iter().chain(&first.right).all(
            |sample| sample.is_finite() && sample.abs() <= crate::VOICE_LIMIT as f32 * kit.gain
        ));
        assert_eq!(first.left, first.right);
    }

    #[test]
    fn chiptune_dense_variants_share_ticks_and_below_limit_stays_below_eight_voices() {
        let stress = dense_chiptune_hits();
        let below_limit = below_limit_chiptune_hits();
        assert_eq!(stress.len(), 256);
        assert_eq!(below_limit.len(), 256);
        let expected_ticks = [0, 2, 3, 5, 6, 8];
        for (index, expected) in expected_ticks.into_iter().enumerate() {
            assert_eq!(dense_start_tick(index), expected);
        }
        for (index, (stress_hit, below_hit)) in stress.iter().zip(&below_limit).enumerate() {
            assert_eq!(stress_hit.pad_index, below_hit.pad_index);
            assert_eq!(stress_hit.velocity, below_hit.velocity);
            assert_eq!(stress_hit.duration_seconds, None);
            assert_eq!(below_hit.duration_seconds, None);
            assert_eq!(
                stress_hit.start_seconds * TICKS_PER_SECOND,
                dense_start_tick(index) as f64
            );
            assert_eq!(
                below_hit.start_seconds * TICKS_PER_SECOND,
                index as f64 * 64.0
            );
        }
        let maximum_active = below_limit
            .iter()
            .enumerate()
            .map(|(index, hit)| {
                below_limit[..index]
                    .iter()
                    .filter(|previous| {
                        previous.start_seconds
                            + CHIP_SPECS[previous.pad_index].frames as f64
                                / f64::from(CHIP_SAMPLE_RATE)
                            > hit.start_seconds
                    })
                    .count()
                    + 1
            })
            .max()
            .unwrap();
        assert!(maximum_active < 8);
    }

    fn write_dense_artifact(
        root: &Path,
        kit: &DrumKit,
        variant: &str,
        render_name: &str,
        report_name: &str,
        hits: &[crate::KitHit],
    ) {
        let mut sound = crate::render_kit(kit, hits, CHIP_SAMPLE_RATE).unwrap();
        let peak = sound
            .left
            .iter()
            .chain(&sound.right)
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        let scale = (0.99 / peak).min(1.0);
        for sample in sound.left.iter_mut().chain(&mut sound.right) {
            *sample *= scale;
        }
        let render_path = root.join(render_name);
        sound.write_wav(&render_path).unwrap();
        let render_hash = format!("{:x}", Sha256::digest(std::fs::read(&render_path).unwrap()));
        let report = serde_json::json!({
            "schema": "midi-evil-chiptune-dense-native-v2",
            "variant": variant,
            "kit": "chiptune.evilkit",
            "native_render": render_name,
            "native_render_sha256": render_hash,
            "sample_rate_hz": CHIP_SAMPLE_RATE,
            "tempo_bpm": 120.0,
            "swing": 0.5,
            "ticks_per_quarter_note": 256,
            "pattern_length_ticks": if variant == "stress" { 1024 } else { 16384 },
            "unclipped_mix_peak": peak,
            "wav_normalization_scale": scale,
            "hits": hits.iter().map(|hit| serde_json::json!({
                "pad_index": hit.pad_index,
                "note": 48 + hit.pad_index,
                "start_tick": (hit.start_seconds * TICKS_PER_SECOND).round() as u32,
                "duration_ticks": 1,
                "start_seconds": hit.start_seconds,
                "duration_seconds": hit.duration_seconds,
                "velocity": hit.velocity,
            })).collect::<Vec<_>>(),
        });
        std::fs::write(
            root.join(report_name),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "writes reproducible local Chiptune calibration artifacts"]
    fn write_chiptune_comparison_artifacts() {
        use evil_synth::RenderedSound;

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/chiptune");
        let pads_directory = root.join("pads");
        std::fs::create_dir_all(&pads_directory).unwrap();
        let kit = chiptune_kit().unwrap();
        let kit_path = root.join("chiptune.evilkit");
        std::fs::write(&kit_path, kit.to_json().unwrap()).unwrap();
        let portable_kit = DrumKit::read(&kit_path).unwrap();
        assert_eq!(portable_kit, kit);
        for (index, pad) in kit.pads.iter().flatten().enumerate() {
            RenderedSound {
                samples: pad.voice.samples.clone(),
                sample_rate: pad.voice.sample_rate,
            }
            .write_wav(pads_directory.join(format!("pad-{index:02}.wav")))
            .unwrap();
        }
        write_dense_artifact(
            &root,
            &portable_kit,
            "stress",
            "dense-native.wav",
            "dense-pattern.json",
            &dense_chiptune_hits(),
        );
        write_dense_artifact(
            &root,
            &portable_kit,
            "below_limit",
            "below-limit-native.wav",
            "below-limit-pattern.json",
            &below_limit_chiptune_hits(),
        );
    }

    #[test]
    fn starter_kit_is_original_deterministic_and_all_pads_sound() {
        let a = maxwell_kit().unwrap();
        assert_eq!(a, maxwell_kit().unwrap());
        a.validate().unwrap();
        for pad in a.pads.iter().flatten() {
            assert!(pad.voice.samples.iter().any(|sample| sample.abs() > 0.001));
        }
    }
}
