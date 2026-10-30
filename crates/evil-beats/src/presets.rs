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

#[cfg(test)]
mod tests {
    use super::*;
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
