use serde::{Deserialize, Serialize};

use crate::Error;
use evil_performance::EffectParameter;

/// Fixed-band, three-band equalizer with low/high shelves and a mid bell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equalizer {
    pub low_gain_db: f32,
    pub mid_gain_db: f32,
    pub high_gain_db: f32,
    pub mix: f32,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            high_gain_db: 0.0,
            mix: 1.0,
        }
    }
}

impl Equalizer {
    pub fn validate(&self) -> Result<(), Error> {
        if [self.low_gain_db, self.mid_gain_db, self.high_gain_db]
            .iter()
            .any(|gain| !gain.is_finite() || !(-24.0..=24.0).contains(gain))
            || !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
        {
            return Err(Error::InvalidPatch(
                "equalizer gains must be -24..=24 dB and mix must be 0..=1".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct BandState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

pub(crate) struct EqualizerProcessor {
    settings: Equalizer,
    coefficients: [[f32; 5]; 3],
    states: [[BandState; 3]; 2],
}

impl EqualizerProcessor {
    pub(crate) fn new(settings: &Equalizer, sample_rate: u32) -> Self {
        let maximum_frequency = sample_rate as f32 * 0.49;
        Self {
            coefficients: [
                shelf_coefficients(
                    settings.low_gain_db,
                    200.0_f32.min(maximum_frequency),
                    sample_rate,
                    true,
                ),
                bell_coefficients(
                    settings.mid_gain_db,
                    1_000.0_f32.min(maximum_frequency),
                    sample_rate,
                ),
                shelf_coefficients(
                    settings.high_gain_db,
                    4_000.0_f32.min(maximum_frequency),
                    sample_rate,
                    false,
                ),
            ],
            settings: settings.clone(),
            states: std::array::from_fn(|_| std::array::from_fn(|_| BandState::default())),
        }
    }

    pub(crate) fn set_parameter(
        &mut self,
        parameter: EffectParameter,
        value: f32,
        sample_rate: u32,
    ) -> bool {
        let mut effect = crate::InsertEffect::Equalizer(self.settings.clone());
        if !crate::effects::set_effect_parameter(&mut effect, parameter, value) {
            return false;
        }
        let crate::InsertEffect::Equalizer(settings) = effect else {
            unreachable!()
        };
        let replacement = Self::new(&settings, sample_rate);
        self.settings = settings;
        self.coefficients = replacement.coefficients;
        true
    }

    pub(crate) fn process(&mut self, stereo: &mut [f32; 2]) {
        let dry = *stereo;
        for channel in 0..2 {
            let mut sample = dry[channel];
            for band in 0..3 {
                let coefficients = self.coefficients[band];
                let state = &mut self.states[channel][band];
                let output = coefficients[0] * sample
                    + coefficients[1] * state.x1
                    + coefficients[2] * state.x2
                    - coefficients[3] * state.y1
                    - coefficients[4] * state.y2;
                state.x2 = state.x1;
                state.x1 = sample;
                state.y2 = state.y1;
                state.y1 = output;
                sample = output;
            }
            stereo[channel] = dry[channel] + (sample - dry[channel]) * self.settings.mix;
        }
    }
}

fn bell_coefficients(gain_db: f32, frequency: f32, sample_rate: u32) -> [f32; 5] {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;
    let alpha = omega.sin() * std::f32::consts::FRAC_1_SQRT_2;
    normalize(
        1.0 + alpha * a,
        -2.0 * omega.cos(),
        1.0 - alpha * a,
        1.0 + alpha / a,
        -2.0 * omega.cos(),
        1.0 - alpha / a,
    )
}

fn shelf_coefficients(gain_db: f32, frequency: f32, sample_rate: u32, low: bool) -> [f32; 5] {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;
    let cosine = omega.cos();
    let twice_root_alpha = 2.0 * a.sqrt() * omega.sin() * std::f32::consts::FRAC_1_SQRT_2;
    let (b0, b1, b2, a0, a1, a2) = if low {
        (
            a * ((a + 1.0) - (a - 1.0) * cosine + twice_root_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cosine),
            a * ((a + 1.0) - (a - 1.0) * cosine - twice_root_alpha),
            (a + 1.0) + (a - 1.0) * cosine + twice_root_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cosine),
            (a + 1.0) + (a - 1.0) * cosine - twice_root_alpha,
        )
    } else {
        (
            a * ((a + 1.0) + (a - 1.0) * cosine + twice_root_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cosine),
            a * ((a + 1.0) + (a - 1.0) * cosine - twice_root_alpha),
            (a + 1.0) - (a - 1.0) * cosine + twice_root_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cosine),
            (a + 1.0) - (a - 1.0) * cosine - twice_root_alpha,
        )
    };
    normalize(b0, b1, b2, a0, a1, a2)
}

fn normalize(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> [f32; 5] {
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_equalizer_is_transparent_and_boost_changes_the_signal() {
        let mut processor = EqualizerProcessor::new(&Equalizer::default(), 48_000);
        let mut stereo = [0.25, -0.5];
        processor.process(&mut stereo);
        assert_eq!(stereo, [0.25, -0.5]);

        let mut boosted = EqualizerProcessor::new(
            &Equalizer {
                low_gain_db: 6.0,
                ..Equalizer::default()
            },
            48_000,
        );
        let mut filtered = [0.25, -0.5];
        boosted.process(&mut filtered);
        assert_ne!(filtered, stereo);
        assert!(filtered.iter().all(|sample| sample.is_finite()));
    }
}
