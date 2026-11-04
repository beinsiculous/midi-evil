use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoDestination {
    #[default]
    Pitch,
    PulseWidth,
    SyncRatio,
    FilterCutoff,
    FilterResonance,
    OscillatorMix,
    Amplitude,
    LfoRate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoWaveform {
    #[default]
    Sine,
    SawDown,
    SawUp,
    Triangle,
    Square,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lfo {
    pub rate_hz: f32,
    /// Quarter-note beats per cycle when beat-synced; overrides `rate_hz`.
    pub beats_per_cycle: Option<f32>,
    pub depth_cents: f32,
    pub destination: LfoDestination,
    pub waveform: LfoWaveform,
    pub pulse_width_depth: f32,
    pub sync_ratio_depth: f32,
    /// Filter cutoff modulation in octaves.
    pub filter_cutoff_depth: f32,
    /// Bipolar filter-resonance modulation.
    pub filter_resonance_depth: f32,
    /// Signed A/B balance modulation in Mix mode; positive favors Osc B.
    pub oscillator_mix_depth: f32,
    pub amplitude_depth: f32,
    /// Zero-based index into the patch's primary/additional LFO lanes.
    pub target_lfo: Option<usize>,
    /// Fractional rate modulation applied to `target_lfo`.
    pub lfo_rate_depth: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            rate_hz: 4.0,
            beats_per_cycle: None,
            depth_cents: 0.0,
            destination: LfoDestination::Pitch,
            waveform: LfoWaveform::Sine,
            pulse_width_depth: 0.5,
            sync_ratio_depth: 0.5,
            filter_cutoff_depth: 1.0,
            filter_resonance_depth: 0.5,
            oscillator_mix_depth: 0.5,
            amplitude_depth: 0.5,
            target_lfo: None,
            lfo_rate_depth: 0.0,
        }
    }
}
