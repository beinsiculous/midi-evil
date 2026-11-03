use serde::{Deserialize, Serialize};

use crate::Envelope;

/// One auxiliary ADSR with signed depths for the bounded modulation matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuxEnvelope {
    pub envelope: Envelope,
    /// Bipolar amplitude modulation added to the existing LFO contribution.
    pub amplitude_depth: f32,
    /// Pitch modulation in cents.
    pub pitch_depth_cents: f32,
    /// Bipolar pulse-width offset added to the existing LFO contribution.
    pub pulse_width_depth: f32,
    /// Signed oscillator sync-ratio offset added to the existing LFO contribution.
    pub sync_ratio_depth: f32,
    /// Filter cutoff modulation in octaves.
    pub filter_cutoff_depth_octaves: f32,
}

impl Default for AuxEnvelope {
    fn default() -> Self {
        Self {
            envelope: Envelope::default(),
            amplitude_depth: 0.0,
            pitch_depth_cents: 0.0,
            pulse_width_depth: 0.0,
            sync_ratio_depth: 0.0,
            filter_cutoff_depth_octaves: 0.0,
        }
    }
}
