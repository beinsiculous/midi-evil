use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Unison {
    pub voices: u8,
    pub detune_cents: f32,
}

impl Default for Unison {
    fn default() -> Self {
        Self {
            voices: 2,
            detune_cents: 12.0,
        }
    }
}

impl Unison {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=8).contains(&self.voices)
            || !self.detune_cents.is_finite()
            || !(0.0..=100.0).contains(&self.detune_cents)
        {
            return Err(Error::InvalidPatch(
                "unison requires 1..=8 voices and detune between 0 and 100 cents".to_owned(),
            ));
        }
        Ok(())
    }
}
