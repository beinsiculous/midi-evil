use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OscillatorSync {
    /// Slave oscillator frequency relative to the reset/master frequency.
    pub ratio: f32,
}

impl Default for OscillatorSync {
    fn default() -> Self {
        Self { ratio: 1.5 }
    }
}

impl OscillatorSync {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.ratio.is_finite() || !(0.01..=16.0).contains(&self.ratio) {
            return Err(Error::InvalidPatch(
                "oscillator sync ratio must be between 0.01 and 16".to_owned(),
            ));
        }
        Ok(())
    }
}
