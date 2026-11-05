use serde::{Deserialize, Serialize};

use crate::Error;
use evil_performance::EffectParameter;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Compressor {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_seconds: f32,
    pub release_seconds: f32,
    pub makeup_gain_db: f32,
    pub mix: f32,
}

impl Default for Compressor {
    fn default() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_seconds: 0.005,
            release_seconds: 0.1,
            makeup_gain_db: 0.0,
            mix: 1.0,
        }
    }
}

impl Compressor {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.threshold_db.is_finite()
            || !(-60.0..=0.0).contains(&self.threshold_db)
            || !self.ratio.is_finite()
            || !(1.0..=20.0).contains(&self.ratio)
            || !self.attack_seconds.is_finite()
            || !(0.000_1..=1.0).contains(&self.attack_seconds)
            || !self.release_seconds.is_finite()
            || !(0.001..=2.0).contains(&self.release_seconds)
            || !self.makeup_gain_db.is_finite()
            || !(-12.0..=24.0).contains(&self.makeup_gain_db)
            || !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
        {
            return Err(Error::InvalidPatch(
                "invalid compressor threshold, ratio, timing, makeup gain, or mix".to_owned(),
            ));
        }
        Ok(())
    }
}

pub(crate) struct CompressorProcessor {
    settings: Compressor,
    reduction_db: f32,
}

impl CompressorProcessor {
    pub(crate) fn new(settings: &Compressor) -> Self {
        Self {
            settings: settings.clone(),
            reduction_db: 0.0,
        }
    }

    pub(crate) fn set_parameter(&mut self, parameter: EffectParameter, value: f32) -> bool {
        let mut effect = crate::InsertEffect::Compressor(self.settings.clone());
        if !crate::effects::set_effect_parameter(&mut effect, parameter, value) {
            return false;
        }
        let crate::InsertEffect::Compressor(settings) = effect else {
            unreachable!()
        };
        self.settings = settings;
        true
    }

    pub(crate) fn process(&mut self, stereo: &mut [f32; 2], sample_rate: u32) {
        let peak = stereo[0].abs().max(stereo[1].abs()).max(0.000_001);
        let level_db = 20.0 * peak.log10();
        let over_db = (level_db - self.settings.threshold_db).max(0.0);
        let target_reduction = -over_db * (1.0 - self.settings.ratio.recip());
        let seconds = if target_reduction < self.reduction_db {
            self.settings.attack_seconds
        } else {
            self.settings.release_seconds
        };
        let coefficient = 1.0 - (-1.0 / (seconds * sample_rate as f32)).exp();
        self.reduction_db += coefficient * (target_reduction - self.reduction_db);
        let gain = 10.0_f32.powf((self.reduction_db + self.settings.makeup_gain_db) / 20.0);
        for sample in stereo {
            let wet = *sample * gain;
            *sample += (wet - *sample) * self.settings.mix;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_is_stereo_linked_and_reduces_hot_peaks() {
        let mut processor = CompressorProcessor::new(&Compressor::default());
        let mut stereo = [1.0, -0.5];
        processor.process(&mut stereo, 48_000);
        assert!(stereo[0].abs() < 1.0);
        assert!(stereo[1].abs() < 0.5);
        assert!((stereo[0] / stereo[1] - -2.0).abs() < 0.01);
        processor.process(&mut stereo, 48_000);
        assert!(stereo.iter().all(|sample| sample.is_finite()));
    }
}
