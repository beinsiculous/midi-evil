use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OscillatorSync {
    /// Slave oscillator frequency relative to the reset/master frequency.
    pub ratio: f32,
    /// Interpret sync-ratio modulation offsets as octaves (`ratio * 2^offset`) rather than linear offsets.
    pub modulation_octaves: bool,
    /// Optional per-voice high-pass cutoff in Hz applied before amplitude shaping; zero disables it.
    pub dc_block_hz: f32,
}

impl Default for OscillatorSync {
    fn default() -> Self {
        Self {
            ratio: 1.5,
            modulation_octaves: false,
            dc_block_hz: 0.0,
        }
    }
}

impl OscillatorSync {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.ratio.is_finite() || !(0.01..=16.0).contains(&self.ratio) {
            return Err(Error::InvalidPatch(
                "oscillator sync ratio must be between 0.01 and 16".to_owned(),
            ));
        }
        if !self.dc_block_hz.is_finite() || !(0.0..=200.0).contains(&self.dc_block_hz) {
            return Err(Error::InvalidPatch(
                "oscillator sync dc_block_hz must be between 0 and 200".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_sync_json_defaults_opt_in_fields_and_round_trips() {
        let legacy: OscillatorSync = serde_json::from_str(r#"{"ratio": 1.25}"#).unwrap();
        assert_eq!(legacy.ratio, 1.25);
        assert!(!legacy.modulation_octaves);
        assert_eq!(legacy.dc_block_hz, 0.0);
        legacy.validate().unwrap();
        let legacy_patch = crate::Patch {
            sync: Some(legacy),
            ..crate::Patch::default()
        };
        assert!(legacy_patch.uses_sync());
        assert!(legacy_patch.uses_output_dc_block());

        let configured = OscillatorSync {
            ratio: 1.25,
            modulation_octaves: true,
            dc_block_hz: 15.0,
        };
        let json = serde_json::to_string(&configured).unwrap();
        assert_eq!(
            serde_json::from_str::<OscillatorSync>(&json).unwrap(),
            configured
        );
        configured.validate().unwrap();
        let configured_patch = crate::Patch {
            sync: Some(configured),
            ..crate::Patch::default()
        };
        assert!(configured_patch.uses_sync());
        assert!(!configured_patch.uses_output_dc_block());
    }

    #[test]
    fn validates_sync_dc_block_cutoff_range() {
        for dc_block_hz in [0.0, 15.0, 200.0] {
            assert!(
                OscillatorSync {
                    dc_block_hz,
                    ..OscillatorSync::default()
                }
                .validate()
                .is_ok()
            );
        }
        for dc_block_hz in [-0.01, 200.01, f32::NAN, f32::INFINITY] {
            assert!(
                OscillatorSync {
                    dc_block_hz,
                    ..OscillatorSync::default()
                }
                .validate()
                .is_err()
            );
        }
    }
}
