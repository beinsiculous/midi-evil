use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModalPartial {
    /// Frequency ratio to the note's fundamental.
    pub ratio: f32,
    pub amplitude: f32,
    /// Exponential amplitude-decay time in seconds.
    pub decay_seconds: f32,
}

impl Default for ModalPartial {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            amplitude: 0.25,
            decay_seconds: 1.0,
        }
    }
}

impl ModalPartial {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !self.ratio.is_finite()
            || !(0.01..=64.0).contains(&self.ratio)
            || !self.amplitude.is_finite()
            || !(0.0..=1.0).contains(&self.amplitude)
            || !self.decay_seconds.is_finite()
            || !(0.001..=60.0).contains(&self.decay_seconds)
        {
            return Err(Error::InvalidPatch("invalid modal partial".to_owned()));
        }
        Ok(())
    }
}
