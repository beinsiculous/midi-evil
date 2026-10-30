use crate::{
    Delay, DelayMode, Envelope, Error, Filter, FilterMode, Lfo, LfoDestination, LfoWaveform,
    Oscillator, OscillatorSync, Patch, Waveform,
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
        // The dry reference matches a 1.25 base ratio and an almost-open filter;
        // tuning, gain, and delay are calibrated against the full-bar render.
        "nasty" => Patch {
            name: "A17 Nasty".to_owned(),
            duration_seconds: 0.35,
            gain: 0.31,
            midi_program: Some(80),
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                detune_cents: 15.5,
                ..Oscillator::default()
            }],
            sync: Some(OscillatorSync { ratio: 1.25 }),
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.18,
                sustain_level: 0.75,
                release_seconds: 0.135,
            },
            filter: Some(Filter {
                mode: FilterMode::LowPass,
                cutoff_hz: 20_000.0,
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
                    sync_ratio_depth: 0.35,
                    ..Lfo::default()
                },
                Lfo {
                    rate_hz: 4.41,
                    destination: LfoDestination::SyncRatio,
                    waveform: LfoWaveform::SawDown,
                    sync_ratio_depth: 0.5,
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
            ],
            delay: Some(Delay {
                time_seconds: 0.5,
                feedback: 0.55,
                mode: DelayMode::StereoCross,
                ..Delay::default()
            }),
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

fn oscillator(waveform: Waveform, start: f32, end: f32, amplitude: f32) -> Oscillator {
    Oscillator {
        waveform,
        start_frequency_hz: start,
        end_frequency_hz: end,
        amplitude,
        detune_cents: 0.0,
        pulse_width: 0.5,
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
