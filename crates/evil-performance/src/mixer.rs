use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// Native, continuous mixer automation stored alongside MIDI performance events.
/// Values use the parameter's natural units rather than a 7-bit MIDI value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MixerAutomationEvent {
    pub tick: u64,
    pub parameter: MixerParameter,
    pub value: f32,
}

impl MixerAutomationEvent {
    pub fn is_valid(&self) -> bool {
        self.parameter.is_valid_value(self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixerParameter {
    Gain,
    Pan,
    SendGain {
        bus: u8,
    },
    Insert {
        slot: u8,
        parameter: EffectParameter,
    },
}

impl MixerParameter {
    pub fn has_valid_target(self) -> bool {
        match self {
            Self::Gain | Self::Pan => true,
            Self::SendGain { bus } => bus < 2,
            Self::Insert { slot, .. } => slot < 4,
        }
    }

    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::Gain | Self::SendGain { .. } => 0.0..=1.0,
            Self::Pan => -1.0..=1.0,
            Self::Insert { parameter, .. } => parameter.range(),
        }
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Insert {
                parameter: EffectParameter::BitcrushBits | EffectParameter::BitcrushDownsample,
                ..
            }
        )
    }

    pub fn is_valid_value(self, value: f32) -> bool {
        self.has_valid_target()
            && value.is_finite()
            && self.range().contains(&value)
            && (!self.is_integer() || value.fract() == 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectParameter {
    WaveshaperDrive,
    WaveshaperContour,
    WaveshaperLevel,
    DelayTimeSeconds,
    DelayBeatsPerCycle,
    DelayFeedback,
    DelayMix,
    BitcrushBits,
    BitcrushDownsample,
    BitcrushMix,
    EqualizerLowGainDb,
    EqualizerMidGainDb,
    EqualizerHighGainDb,
    EqualizerMix,
    CompressorThresholdDb,
    CompressorRatio,
    CompressorAttackSeconds,
    CompressorReleaseSeconds,
    CompressorMakeupGainDb,
    CompressorMix,
    ChorusRateHz,
    ChorusDepthSeconds,
    ChorusDelaySeconds,
    ChorusFeedback,
    ChorusMix,
    FlangerRateHz,
    FlangerDepthSeconds,
    FlangerDelaySeconds,
    FlangerFeedback,
    FlangerMix,
    ReverbRoomSize,
    ReverbDamping,
    ReverbMix,
}

impl EffectParameter {
    pub const ALL: [Self; 33] = [
        Self::WaveshaperDrive,
        Self::WaveshaperContour,
        Self::WaveshaperLevel,
        Self::DelayTimeSeconds,
        Self::DelayBeatsPerCycle,
        Self::DelayFeedback,
        Self::DelayMix,
        Self::BitcrushBits,
        Self::BitcrushDownsample,
        Self::BitcrushMix,
        Self::EqualizerLowGainDb,
        Self::EqualizerMidGainDb,
        Self::EqualizerHighGainDb,
        Self::EqualizerMix,
        Self::CompressorThresholdDb,
        Self::CompressorRatio,
        Self::CompressorAttackSeconds,
        Self::CompressorReleaseSeconds,
        Self::CompressorMakeupGainDb,
        Self::CompressorMix,
        Self::ChorusRateHz,
        Self::ChorusDepthSeconds,
        Self::ChorusDelaySeconds,
        Self::ChorusFeedback,
        Self::ChorusMix,
        Self::FlangerRateHz,
        Self::FlangerDepthSeconds,
        Self::FlangerDelaySeconds,
        Self::FlangerFeedback,
        Self::FlangerMix,
        Self::ReverbRoomSize,
        Self::ReverbDamping,
        Self::ReverbMix,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::WaveshaperDrive => "Drive",
            Self::WaveshaperContour => "Contour",
            Self::WaveshaperLevel => "Level",
            Self::DelayTimeSeconds => "Time / s",
            Self::DelayBeatsPerCycle => "Time / beats",
            Self::DelayFeedback => "Feedback",
            Self::DelayMix => "Wet mix",
            Self::BitcrushBits => "Bits",
            Self::BitcrushDownsample => "Downsample",
            Self::BitcrushMix => "Wet mix",
            Self::EqualizerLowGainDb => "Low / dB",
            Self::EqualizerMidGainDb => "Mid / dB",
            Self::EqualizerHighGainDb => "High / dB",
            Self::EqualizerMix => "Wet mix",
            Self::CompressorThresholdDb => "Threshold / dB",
            Self::CompressorRatio => "Ratio",
            Self::CompressorAttackSeconds => "Attack / s",
            Self::CompressorReleaseSeconds => "Release / s",
            Self::CompressorMakeupGainDb => "Makeup / dB",
            Self::CompressorMix => "Wet mix",
            Self::ChorusRateHz => "Rate / Hz",
            Self::ChorusDepthSeconds => "Depth / s",
            Self::ChorusDelaySeconds => "Delay / s",
            Self::ChorusFeedback => "Feedback",
            Self::ChorusMix => "Wet mix",
            Self::FlangerRateHz => "Rate / Hz",
            Self::FlangerDepthSeconds => "Depth / s",
            Self::FlangerDelaySeconds => "Delay / s",
            Self::FlangerFeedback => "Feedback",
            Self::FlangerMix => "Wet mix",
            Self::ReverbRoomSize => "Room size",
            Self::ReverbDamping => "Damping",
            Self::ReverbMix => "Wet mix",
        }
    }

    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::WaveshaperDrive => 1.0..=32.0,
            Self::WaveshaperContour => -1.0..=1.0,
            Self::WaveshaperLevel => 0.0..=1.0,
            Self::DelayTimeSeconds => 0.001..=2.0,
            Self::DelayBeatsPerCycle => 0.001..=64.0,
            Self::DelayFeedback => 0.0..=0.95,
            Self::DelayMix => 0.0..=1.0,
            Self::BitcrushBits => 1.0..=16.0,
            Self::BitcrushDownsample => 1.0..=64.0,
            Self::BitcrushMix => 0.0..=1.0,
            Self::EqualizerLowGainDb | Self::EqualizerMidGainDb | Self::EqualizerHighGainDb => {
                -24.0..=24.0
            }
            Self::EqualizerMix => 0.0..=1.0,
            Self::CompressorThresholdDb => -60.0..=0.0,
            Self::CompressorRatio => 1.0..=20.0,
            Self::CompressorAttackSeconds => 0.000_1..=1.0,
            Self::CompressorReleaseSeconds => 0.001..=2.0,
            Self::CompressorMakeupGainDb => -12.0..=24.0,
            Self::CompressorMix => 0.0..=1.0,
            Self::ChorusRateHz | Self::FlangerRateHz => 0.01..=20.0,
            Self::ChorusDepthSeconds | Self::FlangerDepthSeconds => 0.0..=0.03,
            Self::ChorusDelaySeconds | Self::FlangerDelaySeconds => 0.001..=0.1,
            Self::ChorusFeedback | Self::FlangerFeedback => 0.0..=0.95,
            Self::ChorusMix | Self::FlangerMix => 0.0..=1.0,
            Self::ReverbRoomSize | Self::ReverbDamping | Self::ReverbMix => 0.0..=1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_events_validate_targets_units_and_integer_parameters() {
        for parameter in EffectParameter::ALL {
            let target = MixerParameter::Insert { slot: 3, parameter };
            let value = *parameter.range().start();
            assert!(
                MixerAutomationEvent {
                    tick: 0,
                    parameter: target,
                    value
                }
                .is_valid()
            );
            assert!(
                !MixerAutomationEvent {
                    tick: 0,
                    parameter: target,
                    value: f32::NAN,
                }
                .is_valid()
            );
        }
        assert!(
            !MixerAutomationEvent {
                tick: 0,
                parameter: MixerParameter::Insert {
                    slot: 0,
                    parameter: EffectParameter::BitcrushBits,
                },
                value: 7.5,
            }
            .is_valid()
        );
        assert!(
            !MixerAutomationEvent {
                tick: 0,
                parameter: MixerParameter::SendGain { bus: 2 },
                value: 0.5,
            }
            .is_valid()
        );
    }
}
