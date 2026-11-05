use serde::{Deserialize, Serialize};

use crate::{SampleOscillator, SampleZone, wavetable};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OscillatorMode {
    /// Add Osc A, Osc B, and any auxiliary oscillators.
    #[default]
    Mix,
    /// Keep Osc A as the master and reset Osc B/slaves at its cycle boundary.
    Sync,
    /// Multiply Osc A by Osc B; auxiliary oscillators remain additive.
    Ring,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Waveform {
    #[default]
    Sine,
    Square,
    /// Unbandlimited square for compatibility with legacy synth oscillators.
    SquareNaive,
    Saw,
    Triangle,
    Noise,
    Sample,
    // Wavetable variants must remain contiguous and in this order.
    Sine2,
    Sine3,
    Sine4,
    Sine5,
    Sine6,
    Sine7,
    Sine8,
    Pulse10,
    Pulse20,
    Pulse30,
    Pulse40,
    Pulse60,
    Pulse70,
    Pulse80,
    Pulse90,
    SawFundamental,
    SawEven,
    SawOdd,
    SawSoft,
    TriangleFundamental,
    TriangleEven,
    TriangleBright,
    TriangleSoft,
    HalfSine,
    AbsoluteSine,
    SineSquared,
    SineCubed,
    SineRectified,
    Parabola,
    SawTriangle,
    SquareTriangle,
}

impl Waveform {
    pub const BASIC: [Self; 36] = [
        Self::Sine,
        Self::Square,
        Self::Saw,
        Self::Triangle,
        Self::Noise,
        Self::Sine2,
        Self::Sine3,
        Self::Sine4,
        Self::Sine5,
        Self::Sine6,
        Self::Sine7,
        Self::Sine8,
        Self::Pulse10,
        Self::Pulse20,
        Self::Pulse30,
        Self::Pulse40,
        Self::Pulse60,
        Self::Pulse70,
        Self::Pulse80,
        Self::Pulse90,
        Self::SawFundamental,
        Self::SawEven,
        Self::SawOdd,
        Self::SawSoft,
        Self::TriangleFundamental,
        Self::TriangleEven,
        Self::TriangleBright,
        Self::TriangleSoft,
        Self::HalfSine,
        Self::AbsoluteSine,
        Self::SineSquared,
        Self::SineCubed,
        Self::SineRectified,
        Self::Parabola,
        Self::SawTriangle,
        Self::SquareTriangle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sine => "Sine",
            Self::Square => "Square",
            Self::SquareNaive => "Square (naive)",
            Self::Saw => "Saw",
            Self::Triangle => "Triangle",
            Self::Noise => "Noise",
            Self::Sample => "Sample",
            Self::Sine2 => "Sine ×2",
            Self::Sine3 => "Sine ×3",
            Self::Sine4 => "Sine ×4",
            Self::Sine5 => "Sine ×5",
            Self::Sine6 => "Sine ×6",
            Self::Sine7 => "Sine ×7",
            Self::Sine8 => "Sine ×8",
            Self::Pulse10 => "Pulse 10%",
            Self::Pulse20 => "Pulse 20%",
            Self::Pulse30 => "Pulse 30%",
            Self::Pulse40 => "Pulse 40%",
            Self::Pulse60 => "Pulse 60%",
            Self::Pulse70 => "Pulse 70%",
            Self::Pulse80 => "Pulse 80%",
            Self::Pulse90 => "Pulse 90%",
            Self::SawFundamental => "Saw + fundamental",
            Self::SawEven => "Saw even",
            Self::SawOdd => "Saw odd",
            Self::SawSoft => "Saw soft",
            Self::TriangleFundamental => "Triangle + fundamental",
            Self::TriangleEven => "Triangle even",
            Self::TriangleBright => "Triangle bright",
            Self::TriangleSoft => "Triangle soft",
            Self::HalfSine => "Half sine",
            Self::AbsoluteSine => "Absolute sine",
            Self::SineSquared => "Sine squared",
            Self::SineCubed => "Sine cubed",
            Self::SineRectified => "Asymmetric sine",
            Self::Parabola => "Parabola",
            Self::SawTriangle => "Saw / triangle",
            Self::SquareTriangle => "Square / triangle",
        }
    }

    pub(crate) fn table_index(self) -> Option<usize> {
        (self as usize)
            .checked_sub(Self::Sine2 as usize)
            .filter(|index| *index < 31)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Oscillator {
    pub waveform: Waveform,
    pub start_frequency_hz: f32,
    pub end_frequency_hz: f32,
    pub amplitude: f32,
    pub detune_cents: f32,
    pub pulse_width: f32,
    /// Embedded source used only by the sample waveform.
    pub sample: Option<SampleOscillator>,
    /// Optional multi-sample keyboard mapping. Legacy `sample` audio is one full-keyboard zone.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sample_zones: Vec<SampleZone>,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            waveform: Waveform::Sine,
            start_frequency_hz: 440.0,
            end_frequency_hz: 440.0,
            amplitude: 1.0,
            detune_cents: 0.0,
            pulse_width: 0.5,
            sample: None,
            sample_zones: Vec::new(),
        }
    }
}

pub(crate) struct OscillatorState {
    phase: f32,
    sample_position: f64,
    sample_zone: Option<usize>,
    noise: XorShift32,
}

impl OscillatorState {
    pub(crate) fn reset_phase(&mut self, oscillator: &Oscillator) {
        self.phase = 0.0;
        self.sample_position = oscillator.sample_start_position(self.sample_zone);
    }

    pub(crate) fn select_sample_zone(&mut self, oscillator: &Oscillator, note_pitch: u8) {
        let selected = oscillator.sample_zone_index(note_pitch);
        if self.sample_zone != selected {
            self.sample_position = oscillator.sample_start_position(selected);
        }
        self.sample_zone = selected;
    }
}

impl Oscillator {
    pub(crate) fn state(&self, seed: u32, note_pitch: u8) -> OscillatorState {
        let sample_zone = self.sample_zone_index(note_pitch);
        OscillatorState {
            phase: 0.0,
            sample_position: self.sample_start_position(sample_zone),
            sample_zone,
            noise: XorShift32::new(seed),
        }
    }

    /// Resolve a key without depending on zone ordering. Patches reject overlapping zones.
    pub fn sample_for_key(&self, key: u8) -> Option<&SampleOscillator> {
        if self.sample_zones.is_empty() {
            return (key <= 127).then_some(self.sample.as_ref()).flatten();
        }
        self.sample_zones
            .iter()
            .find(|zone| zone.contains(key))
            .map(|zone| &zone.sample)
    }

    fn sample_zone_index(&self, key: u8) -> Option<usize> {
        self.sample_zones.iter().position(|zone| zone.contains(key))
    }

    fn sample_for_zone(&self, zone: Option<usize>) -> Option<&SampleOscillator> {
        if self.sample_zones.is_empty() {
            self.sample.as_ref()
        } else {
            zone.and_then(|index| self.sample_zones.get(index))
                .map(|zone| &zone.sample)
        }
    }

    fn sample_start_position(&self, zone: Option<usize>) -> f64 {
        self.sample_for_zone(zone)
            .map_or(0.0, |sample| sample.playback_range().start as f64)
    }

    pub(crate) fn frequency(&self, sweep_progress: f32, pitch_multiplier: f32) -> f32 {
        self.start_frequency_hz
            * (self.end_frequency_hz / self.start_frequency_hz).powf(sweep_progress)
            * 2.0_f32.powf(self.detune_cents / 1200.0)
            * pitch_multiplier
    }

    pub(crate) fn sync_correction(
        &self,
        state: &OscillatorState,
        reset_phase: f32,
        sweep_progress: f32,
        sample_rate: u32,
        pitch_multiplier: f32,
        pulse_width_offset: f32,
    ) -> f32 {
        let phase_step = self.frequency(sweep_progress, pitch_multiplier) / sample_rate as f32;
        let previous_phase = (state.phase - phase_step).fract();
        let discontinuity = match self.waveform {
            Waveform::Square => {
                let pulse_width = (self.pulse_width + pulse_width_offset).clamp(0.01, 0.99);
                let previous = if previous_phase < pulse_width {
                    1.0
                } else {
                    -1.0
                };
                1.0 - previous
            }
            Waveform::Saw => -2.0 * previous_phase,
            Waveform::Sine
            | Waveform::SquareNaive
            | Waveform::Triangle
            | Waveform::Noise
            | Waveform::Sample => {
                return 0.0;
            }
            _ => {
                let index = self.waveform.table_index().expect("wavetable waveform");
                let previous = wavetable::sample(index, previous_phase, phase_step);
                let reset = wavetable::sample(index, 0.0, phase_step);
                reset - previous
            }
        };
        -0.5 * discontinuity * poly_blep(reset_phase, phase_step) * self.amplitude
    }

    pub(crate) fn sample(
        &self,
        state: &mut OscillatorState,
        sweep_progress: f32,
        sample_rate: u32,
        pitch_multiplier: f32,
        pulse_width_offset: f32,
    ) -> f32 {
        if self.waveform == Waveform::Sample {
            let Some(source) = self.sample_for_zone(state.sample_zone) else {
                return 0.0;
            };
            let playback_rate = self.frequency(sweep_progress, pitch_multiplier)
                / source.root_frequency_hz
                * source.sample_rate as f32
                / sample_rate as f32;
            let sample = source.at(state.sample_position) * self.amplitude;
            state.sample_position = source.advance(state.sample_position, f64::from(playback_rate));
            return sample;
        }
        if matches!(self.waveform, Waveform::Noise) {
            return state.noise.next_sample() * self.amplitude;
        }

        let frequency = self.frequency(sweep_progress, pitch_multiplier);
        let phase_step = frequency / sample_rate as f32;
        let phase = state.phase;
        let pulse_width = (self.pulse_width + pulse_width_offset).clamp(0.01, 0.99);
        if let Some(index) = self.waveform.table_index() {
            let sample = wavetable::sample(index, phase, phase_step);
            state.phase = (phase + phase_step).fract();
            return sample * self.amplitude;
        }
        let sample = match self.waveform {
            Waveform::Sine => (phase * 2.0 * std::f32::consts::PI).sin(),
            Waveform::Square => {
                let naive = if phase < pulse_width { 1.0 } else { -1.0 };
                naive + poly_blep(phase, phase_step)
                    - poly_blep((phase + pulse_width).fract(), phase_step)
            }
            Waveform::SquareNaive => {
                if phase < pulse_width {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Saw => 2.0 * phase - 1.0 - poly_blep(phase, phase_step),
            Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Waveform::Noise | Waveform::Sample => unreachable!("special sources returned above"),
            _ => unreachable!("wavetable sources returned above"),
        };
        state.phase = (phase + phase_step).fract();
        sample * self.amplitude
    }
}

pub(crate) fn poly_blep(phase: f32, phase_step: f32) -> f32 {
    if phase < phase_step {
        let position = phase / phase_step;
        position + position - position * position - 1.0
    } else if phase > 1.0 - phase_step {
        let position = (phase - 1.0) / phase_step;
        position * position + position + position + 1.0
    } else {
        0.0
    }
}

struct XorShift32(u32);

impl XorShift32 {
    fn new(seed: u32) -> Self {
        let state = seed ^ 0x9e37_79b9;
        Self(if state == 0 { 0x5255_5354 } else { state })
    }

    fn next_sample(&mut self) -> f32 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.0 = value;
        let unit = (value >> 8) as f32 / 8_388_607.0;
        unit * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_zone_selection_is_deterministic_at_inclusive_boundaries() {
        let low = SampleOscillator {
            name: "low".to_owned(),
            ..SampleOscillator::new(vec![-0.25], 8_000)
        };
        let high = SampleOscillator {
            name: "high".to_owned(),
            ..SampleOscillator::new(vec![0.5], 8_000)
        };
        let oscillator = Oscillator {
            waveform: Waveform::Sample,
            sample_zones: vec![SampleZone::new(60, 127, high), SampleZone::new(0, 59, low)],
            ..Oscillator::default()
        };
        for _ in 0..4 {
            assert_eq!(oscillator.sample_for_key(0).unwrap().name, "low");
            assert_eq!(oscillator.sample_for_key(59).unwrap().name, "low");
            assert_eq!(oscillator.sample_for_key(60).unwrap().name, "high");
            assert_eq!(oscillator.sample_for_key(127).unwrap().name, "high");
        }
        assert!(oscillator.sample_for_key(128).is_none());

        let mut state = oscillator.state(1, 60);
        assert_eq!(oscillator.sample(&mut state, 0.0, 8_000, 1.0, 0.0), 0.5);
        state.select_sample_zone(&oscillator, 59);
        assert_eq!(oscillator.sample(&mut state, 0.0, 8_000, 1.0, 0.0), -0.25);
    }

    #[test]
    fn basic_waveform_inventory_has_36_unique_json_round_trips() {
        assert_eq!(Waveform::BASIC.len(), 36);
        let labels = Waveform::BASIC
            .into_iter()
            .map(Waveform::label)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(labels.len(), 36);
        for waveform in Waveform::BASIC {
            let json = serde_json::to_string(&waveform).unwrap();
            assert_eq!(serde_json::from_str::<Waveform>(&json).unwrap(), waveform);
        }
    }

    #[test]
    fn sync_correction_smooths_square_reset_transitions() {
        let oscillator = Oscillator {
            waveform: Waveform::Square,
            ..Oscillator::default()
        };
        let mut state = oscillator.state(1, 69);
        state.phase = 0.75;
        let correction = oscillator.sync_correction(&state, 0.0, 0.0, 48_000, 1.0, 0.0);
        assert!((correction - 1.0).abs() < 0.000_001);

        let naive = Oscillator {
            waveform: Waveform::SquareNaive,
            ..oscillator
        };
        assert_eq!(
            naive.sync_correction(&state, 0.0, 0.0, 48_000, 1.0, 0.0),
            0.0
        );
    }
}
