use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    LowPass,
    BandPass,
    HighPass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub mode: FilterMode,
    pub cutoff_hz: f32,
    /// Envelope modulation in octaves; zero disables it.
    pub envelope_amount: f32,
    /// Resonant emphasis from clean (0) to extreme (1).
    pub resonance: f32,
    /// Number of cascaded 6 dB/octave stages.
    pub stages: u8,
    pub key_tracking: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            mode: FilterMode::LowPass,
            cutoff_hz: 4_000.0,
            envelope_amount: 0.0,
            resonance: 0.0,
            stages: 1,
            key_tracking: false,
        }
    }
}

#[derive(Default)]
pub(crate) struct FilterState {
    one_pole: [f32; 4],
    band_pass: [[f32; 2]; 4],
    svf: [[f32; 2]; 4],
}

impl Filter {
    pub(crate) fn process(
        &self,
        input: f32,
        state: &mut FilterState,
        sample_rate: u32,
        envelope: f32,
        lfo_octaves: f32,
    ) -> f32 {
        let cutoff = (self.cutoff_hz * 2.0_f32.powf(self.envelope_amount * envelope + lfo_octaves))
            .clamp(0.001, sample_rate as f32 * 0.49);
        if self.resonance > 0.0 {
            self.process_resonant(input, state, sample_rate, cutoff)
        } else {
            self.process_one_pole(input, state, sample_rate, cutoff)
        }
    }

    fn process_one_pole(
        &self,
        input: f32,
        state: &mut FilterState,
        sample_rate: u32,
        cutoff: f32,
    ) -> f32 {
        let alpha = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / sample_rate as f32).exp();
        if self.mode == FilterMode::BandPass {
            let mut output = input;
            for [high, low] in state
                .band_pass
                .iter_mut()
                .take(self.stages.div_ceil(2) as usize)
            {
                *high += alpha * (output - *high);
                let high_pass = output - *high;
                *low += alpha * (high_pass - *low);
                output = *low;
            }
            return output;
        }
        let mut output = input;
        for stage in state.one_pole.iter_mut().take(self.stages as usize) {
            *stage += alpha * (output - *stage);
            output = match self.mode {
                FilterMode::LowPass => *stage,
                FilterMode::HighPass => output - *stage,
                FilterMode::BandPass => unreachable!("band-pass uses its two-pole state"),
            };
        }
        output
    }

    fn process_resonant(
        &self,
        input: f32,
        state: &mut FilterState,
        sample_rate: u32,
        cutoff: f32,
    ) -> f32 {
        // Topology-preserving-transform state-variable filter. Unlike the
        // Chamberlin form, this remains stable as cutoff and resonance change.
        let g = (std::f32::consts::PI * cutoff / sample_rate as f32).tan();
        let q = 0.5 + self.resonance * 12.0;
        let k = q.recip();
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let mut output = input;
        // The patch stores 6 dB/octave slope units; each resonant SVF stage
        // provides two of them (12 dB/octave).
        let stage_count = self.stages.div_ceil(2) as usize;
        for stage in state.svf.iter_mut().take(stage_count) {
            let stage_input = output;
            let v3 = stage_input - stage[1];
            let v1 = a1 * stage[0] + a2 * v3;
            let v2 = stage[1] + a2 * stage[0] + a3 * v3;
            stage[0] = 2.0 * v1 - stage[0];
            stage[1] = 2.0 * v2 - stage[1];
            output = match self.mode {
                FilterMode::LowPass => v2,
                FilterMode::BandPass => v1,
                FilterMode::HighPass => stage_input - k * v1 - v2,
            };
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resonant_stages_use_the_patchs_six_db_slope_units() {
        let output_for = |stages| {
            let filter = Filter {
                cutoff_hz: 500.0,
                resonance: 0.5,
                stages,
                ..Filter::default()
            };
            let mut state = FilterState::default();
            (0..128)
                .map(|frame| {
                    let input = if frame == 0 { 1.0 } else { 0.0 };
                    filter.process(input, &mut state, 48_000, 1.0, 0.0)
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(output_for(3), output_for(4));
        assert_ne!(output_for(2), output_for(4));
    }

    #[test]
    fn all_filter_modes_cover_12_and_24_db_slopes() {
        for mode in [
            FilterMode::LowPass,
            FilterMode::BandPass,
            FilterMode::HighPass,
        ] {
            for resonance in [0.0, 0.2] {
                for (stages, expected) in [(2, 12.0), (4, 24.0)] {
                    let (slope, expected_slope) = match mode {
                        FilterMode::LowPass => (
                            response_db(mode, resonance, stages, 2_000.0)
                                - response_db(mode, resonance, stages, 4_000.0),
                            expected,
                        ),
                        FilterMode::HighPass => (
                            response_db(mode, resonance, stages, 500.0)
                                - response_db(mode, resonance, stages, 250.0),
                            expected,
                        ),
                        FilterMode::BandPass => {
                            // A 12/24 dB band-pass has 6/12 dB slopes on each side.
                            let expected_side = expected / 2.0;
                            let low = response_db(mode, resonance, stages, 40.0)
                                - response_db(mode, resonance, stages, 20.0);
                            let high = response_db(mode, resonance, stages, 2_000.0)
                                - response_db(mode, resonance, stages, 4_000.0);
                            assert!(
                                (low - expected_side).abs() < 1.75,
                                "{mode:?} {stages}-stage low-side slope was {low:.2} dB/octave at resonance {resonance}"
                            );
                            (high, expected_side)
                        }
                    };
                    assert!(
                        (slope - expected_slope).abs() < 1.75,
                        "{mode:?} {stages}-stage slope was {slope:.2} dB/octave at resonance {resonance}"
                    );
                }
            }
        }
    }

    fn response_db(mode: FilterMode, resonance: f32, stages: u8, frequency: f32) -> f32 {
        let filter = Filter {
            mode,
            cutoff_hz: match mode {
                FilterMode::LowPass => 500.0,
                FilterMode::BandPass => 200.0,
                FilterMode::HighPass => 4_000.0,
            },
            resonance,
            stages,
            ..Filter::default()
        };
        let mut state = FilterState::default();
        let sample_rate = 48_000;
        let settled_frame = 24_000;
        let mut energy = 0.0;
        for frame in 0..sample_rate * 2 {
            let phase = 2.0 * std::f64::consts::PI * f64::from(frequency) * frame as f64
                / f64::from(sample_rate);
            let output = filter.process(phase.sin() as f32, &mut state, sample_rate, 0.0, 0.0);
            if frame >= settled_frame {
                energy += output * output;
            }
        }
        20.0 * (energy / (sample_rate * 2 - settled_frame) as f32)
            .sqrt()
            .log10()
    }
}
