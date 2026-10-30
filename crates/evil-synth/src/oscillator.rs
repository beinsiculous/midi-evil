use serde::{Deserialize, Serialize};

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
        }
    }
}

pub(crate) struct OscillatorState {
    phase: f32,
    noise: XorShift32,
}

impl OscillatorState {
    pub(crate) fn reset_phase(&mut self) {
        self.phase = 0.0;
    }
}

impl Oscillator {
    pub(crate) fn state(&self, seed: u32) -> OscillatorState {
        OscillatorState {
            phase: 0.0,
            noise: XorShift32::new(seed),
        }
    }

    pub(crate) fn frequency(&self, sweep_progress: f32, pitch_multiplier: f32) -> f32 {
        self.start_frequency_hz
            * (self.end_frequency_hz / self.start_frequency_hz).powf(sweep_progress)
            * 2.0_f32.powf(self.detune_cents / 1200.0)
            * pitch_multiplier
    }

    pub(crate) fn sample(
        &self,
        state: &mut OscillatorState,
        sweep_progress: f32,
        sample_rate: u32,
        pitch_multiplier: f32,
        pulse_width_offset: f32,
    ) -> f32 {
        if matches!(self.waveform, Waveform::Noise) {
            return state.noise.next_sample() * self.amplitude;
        }

        let frequency = self.frequency(sweep_progress, pitch_multiplier);
        let phase_step = frequency / sample_rate as f32;
        let phase = state.phase;
        let pulse_width = (self.pulse_width + pulse_width_offset).clamp(0.01, 0.99);
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
            Waveform::Noise => unreachable!("noise returned above"),
        };
        state.phase = (phase + phase_step).fract();
        sample * self.amplitude
    }
}

fn poly_blep(phase: f32, phase_step: f32) -> f32 {
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
