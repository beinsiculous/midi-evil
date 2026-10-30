use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    LowPass,
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
        let mut low_pass = input;
        for stage in state.one_pole.iter_mut().take(self.stages as usize) {
            *stage += alpha * (low_pass - *stage);
            low_pass = *stage;
        }
        match self.mode {
            FilterMode::LowPass => low_pass,
            FilterMode::HighPass => input - low_pass,
        }
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
}
