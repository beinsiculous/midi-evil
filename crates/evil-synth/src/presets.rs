use crate::{
    Delay, DelayMode, Envelope, Error, Filter, FilterMode, Lfo, LfoDestination, LfoWaveform,
    ModalPartial, Oscillator, OscillatorMode, OscillatorSync, Patch, VoiceMode, Waveform,
    Waveshaper,
};

pub const PRESET_NAMES: &[&str] = &[
    "laser",
    "jump",
    "coin",
    "explosion",
    "click",
    "kick",
    "lead",
    "nasty",
    "nano-303",
    "glocken",
    "bass",
    "pad",
];

pub fn preset(name: &str) -> Result<Patch, Error> {
    let patch = match name.to_ascii_lowercase().as_str() {
        "laser" => Patch {
            name: "Laser".to_owned(),
            duration_seconds: 0.22,
            gain: 0.75,
            oscillators: vec![
                oscillator(Waveform::Saw, 1_800.0, 120.0, 0.75),
                oscillator(Waveform::Square, 900.0, 60.0, 0.25),
            ],
            envelope: Envelope {
                attack_seconds: 0.002,
                decay_seconds: 0.06,
                sustain_level: 0.55,
                release_seconds: 0.06,
            },
            filter: Some(filter(FilterMode::LowPass, 6_000.0)),
            ..Patch::default()
        },
        "jump" => Patch {
            name: "Jump".to_owned(),
            duration_seconds: 0.24,
            gain: 0.65,
            oscillators: vec![
                oscillator(Waveform::Square, 220.0, 880.0, 0.7),
                oscillator(Waveform::Triangle, 440.0, 1_760.0, 0.2),
            ],
            envelope: Envelope {
                attack_seconds: 0.005,
                decay_seconds: 0.04,
                sustain_level: 0.8,
                release_seconds: 0.05,
            },
            filter: Some(filter(FilterMode::LowPass, 5_000.0)),
            ..Patch::default()
        },
        "coin" => Patch {
            name: "Coin".to_owned(),
            duration_seconds: 0.14,
            gain: 0.6,
            oscillators: vec![
                oscillator(Waveform::Square, 988.0, 1_976.0, 0.55),
                oscillator(Waveform::Sine, 1_976.0, 1_976.0, 0.15),
            ],
            envelope: Envelope {
                attack_seconds: 0.002,
                decay_seconds: 0.025,
                sustain_level: 0.75,
                release_seconds: 0.1,
            },
            ..Patch::default()
        },
        "lead" => Patch {
            name: "Lead".to_owned(),
            duration_seconds: 0.3,
            gain: 0.6,
            midi_program: Some(80),
            oscillators: vec![
                oscillator(Waveform::Saw, 440.0, 440.0, 0.5),
                oscillator(Waveform::Square, 220.0, 220.0, 0.2),
            ],
            envelope: Envelope {
                attack_seconds: 0.01,
                decay_seconds: 0.08,
                sustain_level: 0.75,
                release_seconds: 0.12,
            },
            filter: Some(filter(FilterMode::LowPass, 3_500.0)),
            ..Patch::default()
        },
        // A17's sync-square Osc Mix motion maps to a modulated native sync ratio.
        // Multi-pitch calibration bandlimits the added amplitude articulation,
        // retains high-frequency oscillator energy, and preserves the full delay tail.
        "nasty" => Patch {
            name: "Max Nasty".to_owned(),
            duration_seconds: 0.35,
            gain: 0.33,
            midi_program: Some(80),
            pitch_gain_per_octave: 0.3,
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                detune_cents: 15.5,
                ..Oscillator::default()
            }],
            oscillator_mode: OscillatorMode::Sync,
            sync: Some(OscillatorSync { ratio: 1.25 }),
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.18,
                sustain_level: 0.75,
                release_seconds: 0.04,
            },
            filter: Some(Filter {
                mode: FilterMode::LowPass,
                cutoff_hz: 18_000.0,
                envelope_amount: 0.0,
                resonance: 0.0,
                stages: 4,
                key_tracking: true,
            }),
            lfo: Some(Lfo {
                rate_hz: 10.4,
                depth_cents: 2.0,
                destination: LfoDestination::Pitch,
                waveform: LfoWaveform::SawUp,
                ..Lfo::default()
            }),
            modulation: vec![
                Lfo {
                    rate_hz: 4.41,
                    destination: LfoDestination::SyncRatio,
                    waveform: LfoWaveform::SawUp,
                    sync_ratio_depth: 0.15,
                    ..Lfo::default()
                },
                Lfo {
                    // Source LFO3 beat-sync mode 6: sixteenth notes at 120 BPM.
                    rate_hz: 8.0,
                    beats_per_cycle: Some(0.25),
                    destination: LfoDestination::SyncRatio,
                    waveform: LfoWaveform::SawDown,
                    sync_ratio_depth: 0.2,
                    ..Lfo::default()
                },
                Lfo {
                    rate_hz: 20.0,
                    destination: LfoDestination::LfoRate,
                    waveform: LfoWaveform::Square,
                    target_lfo: Some(2),
                    lfo_rate_depth: 0.25,
                    ..Lfo::default()
                },
                Lfo {
                    rate_hz: 4.0,
                    beats_per_cycle: Some(0.5),
                    destination: LfoDestination::Amplitude,
                    waveform: LfoWaveform::SawDown,
                    amplitude_depth: 0.95,
                    amplitude_transition_seconds: 2.0 / 48_000.0,
                    ..Lfo::default()
                },
            ],
            delay: Some(Delay {
                time_seconds: 0.5,
                beats_per_cycle: Some(1.0),
                maximum_time_seconds: Some(0.5),
                feedback: 0.56,
                mix: 0.25,
                mode: DelayMode::StereoCross,
                ..Delay::default()
            }),
            ..Patch::default()
        },
        // B14 multi-pitch calibration: independent filter envelope, low velocity
        // sensitivity, resonant drive, and a tempo-synced full feedback tail.
        "nano-303" => Patch {
            name: "Evil-303".to_owned(),
            duration_seconds: 0.4,
            gain: 0.16,
            midi_program: Some(38),
            pitch_gain_per_octave: 0.37,
            velocity_exponent: 0.0,
            velocity_filter_octaves: 0.1,
            glide_seconds: 0.05,
            voice_mode: VoiceMode::Mono,
            accent_velocity_threshold: 127,
            accent_filter_octaves: 0.0,
            accent_gain: 0.08,
            oscillators: vec![Oscillator {
                waveform: Waveform::Saw,
                detune_cents: 11.0,
                ..Oscillator::default()
            }],
            envelope: Envelope {
                attack_seconds: 0.002,
                decay_seconds: 0.5,
                sustain_level: 0.6,
                release_seconds: 0.04,
            },
            filter: Some(Filter {
                mode: FilterMode::LowPass,
                cutoff_hz: 2_200.0,
                envelope_amount: 0.5,
                resonance: 0.65,
                stages: 4,
                key_tracking: true,
            }),
            filter_envelope: Some(Envelope {
                attack_seconds: 0.1,
                decay_seconds: 0.4,
                sustain_level: 0.6,
                release_seconds: 0.05,
            }),
            waveshaper: Some(Waveshaper {
                drive: 2.0,
                contour: 0.0,
                level: 0.52,
            }),
            delay: Some(Delay {
                time_seconds: 0.125,
                beats_per_cycle: Some(0.25),
                maximum_time_seconds: None,
                feedback: 0.86,
                mix: 0.04,
                mode: DelayMode::StereoCross,
                tail_seconds: 6.0,
            }),
            ..Patch::default()
        },
        // A09's measured near-harmonic modes use independent levels, decays, and
        // pitch response; one-shot decay continues naturally after note-off.
        "glocken" => Patch {
            name: "GlockenBlocks".to_owned(),
            duration_seconds: 1.2,
            gain: 1.0,
            midi_program: Some(14),
            velocity_exponent: 0.0,
            modal_one_shot: true,
            oscillators: vec![Oscillator {
                waveform: Waveform::Sine,
                amplitude: 0.0,
                detune_cents: -11.0,
                ..Oscillator::default()
            }],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 7.0,
            },
            modal_partials: vec![
                modal(1.0, 0.269_771_22, 1.1, 0.220_36),
                modal(1.99, 0.047_897_984, 1.2, 0.913_97),
                modal(2.995, 0.016_796_285, 1.5, 0.374_51),
                modal(3.99, 0.010_662_528, 1.7, -0.349_74),
                modal(5.0, 0.005_248_956_6, 2.5, -0.277_98),
                modal(6.0, 0.018_430_915, 1.5, -0.556_46),
                modal(8.0, 0.002_121_320_4, 2.5, -0.5),
                modal(9.095, 0.004_613_572_5, 2.0, -0.505_68),
            ],
            ..Patch::default()
        },
        "bass" => Patch {
            name: "Bass".to_owned(),
            duration_seconds: 0.35,
            gain: 0.75,
            midi_program: Some(38),
            oscillators: vec![
                oscillator(Waveform::Sine, 220.0, 220.0, 0.8),
                oscillator(Waveform::Triangle, 110.0, 110.0, 0.25),
            ],
            envelope: Envelope {
                attack_seconds: 0.004,
                decay_seconds: 0.1,
                sustain_level: 0.65,
                release_seconds: 0.08,
            },
            filter: Some(filter(FilterMode::LowPass, 900.0)),
            ..Patch::default()
        },
        "pad" => Patch {
            name: "Pad".to_owned(),
            duration_seconds: 0.8,
            gain: 0.5,
            midi_program: Some(89),
            oscillators: vec![
                oscillator(Waveform::Saw, 440.0, 440.0, 0.25),
                oscillator(Waveform::Triangle, 442.0, 442.0, 0.25),
                oscillator(Waveform::Sine, 220.0, 220.0, 0.2),
            ],
            envelope: Envelope {
                attack_seconds: 0.2,
                decay_seconds: 0.25,
                sustain_level: 0.75,
                release_seconds: 0.5,
            },
            filter: Some(filter(FilterMode::LowPass, 2_200.0)),
            ..Patch::default()
        },
        "explosion" => Patch {
            name: "Explosion".to_owned(),
            duration_seconds: 0.7,
            gain: 0.8,
            pitch_tracking: false,
            oscillators: vec![
                oscillator(Waveform::Noise, 100.0, 100.0, 1.0),
                oscillator(Waveform::Sine, 100.0, 35.0, 0.6),
            ],
            envelope: Envelope {
                attack_seconds: 0.003,
                decay_seconds: 0.3,
                sustain_level: 0.25,
                release_seconds: 0.45,
            },
            filter: Some(filter(FilterMode::LowPass, 1_400.0)),
            ..Patch::default()
        },
        "click" => Patch {
            name: "Click".to_owned(),
            duration_seconds: 0.025,
            gain: 0.55,
            pitch_tracking: false,
            oscillators: vec![oscillator(Waveform::Noise, 100.0, 100.0, 1.0)],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.006,
                sustain_level: 0.15,
                release_seconds: 0.02,
            },
            filter: Some(filter(FilterMode::HighPass, 1_800.0)),
            ..Patch::default()
        },
        "kick" => Patch {
            name: "Kick".to_owned(),
            duration_seconds: 0.2,
            gain: 0.9,
            pitch_tracking: false,
            oscillators: vec![
                oscillator(Waveform::Sine, 190.0, 42.0, 1.0),
                oscillator(Waveform::Triangle, 95.0, 42.0, 0.2),
            ],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.07,
                sustain_level: 0.45,
                release_seconds: 0.15,
            },
            ..Patch::default()
        },
        _ => return Err(Error::UnknownPreset(name.to_owned())),
    };
    patch.validate()?;
    Ok(patch)
}

fn modal(
    ratio: f32,
    amplitude: f32,
    decay_seconds: f32,
    pitch_gain_per_octave: f32,
) -> ModalPartial {
    ModalPartial {
        ratio,
        amplitude,
        decay_seconds,
        pitch_gain_per_octave,
    }
}

fn oscillator(waveform: Waveform, start: f32, end: f32, amplitude: f32) -> Oscillator {
    Oscillator {
        waveform,
        start_frequency_hz: start,
        end_frequency_hz: end,
        amplitude,
        detune_cents: 0.0,
        pulse_width: 0.5,
        sample: None,
        sample_zones: Vec::new(),
    }
}

fn filter(mode: FilterMode, cutoff_hz: f32) -> Filter {
    Filter {
        mode,
        cutoff_hz,
        envelope_amount: 0.0,
        resonance: 0.0,
        stages: 1,
        key_tracking: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase6_presets_use_opt_in_controls() {
        let nano = preset("nano-303").unwrap();
        assert_eq!(nano.voice_mode, VoiceMode::Mono);
        assert_eq!(nano.velocity_exponent, 0.0);
        assert_eq!(nano.velocity_filter_octaves, 0.1);
        assert_eq!(nano.delay.unwrap().tail_seconds, 6.0);

        let glocken = preset("glocken").unwrap();
        assert!(glocken.modal_one_shot);
        assert_eq!(glocken.modal_partials.len(), 8);
        assert!(
            glocken
                .modal_partials
                .iter()
                .any(|partial| partial.pitch_gain_per_octave != 0.0)
        );

        let nasty = preset("nasty").unwrap();
        let nasty_delay = nasty.delay.unwrap();
        assert_eq!(nasty_delay.tail_seconds, 0.0);
        assert_eq!(nasty_delay.beats_per_cycle, Some(1.0));
        assert_eq!(nasty_delay.maximum_time_seconds, Some(0.5));
        assert!(
            nasty
                .modulation
                .iter()
                .any(|lfo| lfo.amplitude_transition_seconds > 0.0)
        );
    }
}
