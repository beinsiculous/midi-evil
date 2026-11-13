use serde::{Deserialize, Serialize};

use crate::delay::{
    allocated_tail_frames, apply_stereo_delay_with_tempo, delay_frame_count, maximum_delay_frames,
    stereo_delay_input, tempo_bpm_at,
};
use crate::dynamics::CompressorProcessor;
use crate::equalizer::EqualizerProcessor;
use crate::modulation::ModulatedDelayProcessor;
use crate::reverb::ReverbProcessor;
use crate::waveshaper::apply_stereo_waveshaper;
use crate::{
    Chorus, Compressor, Delay, Equalizer, Error, Flanger, Reverb, SynthTempoChange, Waveshaper,
};
use evil_performance::EffectParameter;

pub const INSERT_LIMIT: usize = 4;
pub const SEND_COUNT: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bitcrush {
    /// Quantization depth from 1 to 16 bits.
    pub bits: u8,
    /// Number of input frames held by each crushed sample.
    pub downsample: u8,
    pub mix: f32,
}

impl Default for Bitcrush {
    fn default() -> Self {
        Self {
            bits: 8,
            downsample: 4,
            mix: 1.0,
        }
    }
}

impl Bitcrush {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=16).contains(&self.bits)
            || !(1..=64).contains(&self.downsample)
            || !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
        {
            return Err(Error::InvalidPatch(
                "bitcrush requires 1..=16 bits, 1..=64 downsampling, and mix in 0..=1".to_owned(),
            ));
        }
        Ok(())
    }

    fn quantize(&self, input: f32) -> f32 {
        let scale = 2.0_f32.powi(i32::from(self.bits) - 1);
        (input.clamp(-1.0, 1.0) * scale).round() / scale
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InsertEffect {
    Waveshaper(Waveshaper),
    Delay(Delay),
    Bitcrush(Bitcrush),
    Equalizer(Equalizer),
    Compressor(Compressor),
    Chorus(Chorus),
    Flanger(Flanger),
    Reverb(Reverb),
}

impl InsertEffect {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Waveshaper(effect) => effect.validate(),
            Self::Delay(effect) => effect.validate(),
            Self::Bitcrush(effect) => effect.validate(),
            Self::Equalizer(effect) => effect.validate(),
            Self::Compressor(effect) => effect.validate(),
            Self::Chorus(effect) => effect.validate(),
            Self::Flanger(effect) => effect.validate(),
            Self::Reverb(effect) => effect.validate(),
        }
    }
    pub fn automatable_parameters(&self) -> &'static [EffectParameter] {
        use EffectParameter::*;
        match self {
            Self::Waveshaper(_) => &[WaveshaperDrive, WaveshaperContour, WaveshaperLevel],
            Self::Delay(_) => &[
                DelayTimeSeconds,
                DelayBeatsPerCycle,
                DelayFeedback,
                DelayMix,
            ],
            Self::Bitcrush(_) => &[BitcrushBits, BitcrushDownsample, BitcrushMix],
            Self::Equalizer(_) => &[
                EqualizerLowGainDb,
                EqualizerMidGainDb,
                EqualizerHighGainDb,
                EqualizerMix,
            ],
            Self::Compressor(_) => &[
                CompressorThresholdDb,
                CompressorRatio,
                CompressorAttackSeconds,
                CompressorReleaseSeconds,
                CompressorMakeupGainDb,
                CompressorMix,
            ],
            Self::Chorus(_) => &[
                ChorusRateHz,
                ChorusDepthSeconds,
                ChorusDelaySeconds,
                ChorusFeedback,
                ChorusMix,
            ],
            Self::Flanger(_) => &[
                FlangerRateHz,
                FlangerDepthSeconds,
                FlangerDelaySeconds,
                FlangerFeedback,
                FlangerMix,
            ],
            Self::Reverb(_) => &[ReverbRoomSize, ReverbDamping, ReverbMix],
        }
    }
}

pub fn effect_parameter(effect: &InsertEffect, parameter: EffectParameter) -> Option<f32> {
    use EffectParameter::*;
    match (effect, parameter) {
        (InsertEffect::Waveshaper(effect), WaveshaperDrive) => Some(effect.drive),
        (InsertEffect::Waveshaper(effect), WaveshaperContour) => Some(effect.contour),
        (InsertEffect::Waveshaper(effect), WaveshaperLevel) => Some(effect.level),
        (InsertEffect::Delay(effect), DelayTimeSeconds) => Some(effect.time_seconds),
        (InsertEffect::Delay(effect), DelayBeatsPerCycle) => {
            Some(effect.beats_per_cycle.unwrap_or(1.0))
        }
        (InsertEffect::Delay(effect), DelayFeedback) => Some(effect.feedback),
        (InsertEffect::Delay(effect), DelayMix) => Some(effect.mix),
        (InsertEffect::Bitcrush(effect), BitcrushBits) => Some(f32::from(effect.bits)),
        (InsertEffect::Bitcrush(effect), BitcrushDownsample) => Some(f32::from(effect.downsample)),
        (InsertEffect::Bitcrush(effect), BitcrushMix) => Some(effect.mix),
        (InsertEffect::Equalizer(effect), EqualizerLowGainDb) => Some(effect.low_gain_db),
        (InsertEffect::Equalizer(effect), EqualizerMidGainDb) => Some(effect.mid_gain_db),
        (InsertEffect::Equalizer(effect), EqualizerHighGainDb) => Some(effect.high_gain_db),
        (InsertEffect::Equalizer(effect), EqualizerMix) => Some(effect.mix),
        (InsertEffect::Compressor(effect), CompressorThresholdDb) => Some(effect.threshold_db),
        (InsertEffect::Compressor(effect), CompressorRatio) => Some(effect.ratio),
        (InsertEffect::Compressor(effect), CompressorAttackSeconds) => Some(effect.attack_seconds),
        (InsertEffect::Compressor(effect), CompressorReleaseSeconds) => {
            Some(effect.release_seconds)
        }
        (InsertEffect::Compressor(effect), CompressorMakeupGainDb) => Some(effect.makeup_gain_db),
        (InsertEffect::Compressor(effect), CompressorMix) => Some(effect.mix),
        (InsertEffect::Chorus(effect), ChorusRateHz) => Some(effect.rate_hz),
        (InsertEffect::Flanger(effect), FlangerRateHz) => Some(effect.rate_hz),
        (InsertEffect::Chorus(effect), ChorusDepthSeconds) => Some(effect.depth_seconds),
        (InsertEffect::Flanger(effect), FlangerDepthSeconds) => Some(effect.depth_seconds),
        (InsertEffect::Chorus(effect), ChorusDelaySeconds) => Some(effect.delay_seconds),
        (InsertEffect::Flanger(effect), FlangerDelaySeconds) => Some(effect.delay_seconds),
        (InsertEffect::Chorus(effect), ChorusFeedback) => Some(effect.feedback),
        (InsertEffect::Flanger(effect), FlangerFeedback) => Some(effect.feedback),
        (InsertEffect::Chorus(effect), ChorusMix) => Some(effect.mix),
        (InsertEffect::Flanger(effect), FlangerMix) => Some(effect.mix),
        (InsertEffect::Reverb(effect), ReverbRoomSize) => Some(effect.room_size),
        (InsertEffect::Reverb(effect), ReverbDamping) => Some(effect.damping),
        (InsertEffect::Reverb(effect), ReverbMix) => Some(effect.mix),
        _ => None,
    }
}

pub fn set_effect_parameter(
    effect: &mut InsertEffect,
    parameter: EffectParameter,
    value: f32,
) -> bool {
    use EffectParameter::*;
    if !value.is_finite() || !parameter.range().contains(&value) {
        return false;
    }
    match (effect, parameter) {
        (InsertEffect::Waveshaper(effect), WaveshaperDrive) => effect.drive = value,
        (InsertEffect::Waveshaper(effect), WaveshaperContour) => effect.contour = value,
        (InsertEffect::Waveshaper(effect), WaveshaperLevel) => effect.level = value,
        (InsertEffect::Delay(effect), DelayTimeSeconds) => {
            effect.time_seconds = value;
            effect.beats_per_cycle = None;
        }
        (InsertEffect::Delay(effect), DelayBeatsPerCycle) => {
            effect.beats_per_cycle = Some(value);
        }
        (InsertEffect::Delay(effect), DelayFeedback) => effect.feedback = value,
        (InsertEffect::Delay(effect), DelayMix) => effect.mix = value,
        (InsertEffect::Bitcrush(effect), BitcrushBits) => effect.bits = value.round() as u8,
        (InsertEffect::Bitcrush(effect), BitcrushDownsample) => {
            effect.downsample = value.round() as u8;
        }
        (InsertEffect::Bitcrush(effect), BitcrushMix) => effect.mix = value,
        (InsertEffect::Equalizer(effect), EqualizerLowGainDb) => effect.low_gain_db = value,
        (InsertEffect::Equalizer(effect), EqualizerMidGainDb) => effect.mid_gain_db = value,
        (InsertEffect::Equalizer(effect), EqualizerHighGainDb) => effect.high_gain_db = value,
        (InsertEffect::Equalizer(effect), EqualizerMix) => effect.mix = value,
        (InsertEffect::Compressor(effect), CompressorThresholdDb) => effect.threshold_db = value,
        (InsertEffect::Compressor(effect), CompressorRatio) => effect.ratio = value,
        (InsertEffect::Compressor(effect), CompressorAttackSeconds) => {
            effect.attack_seconds = value;
        }
        (InsertEffect::Compressor(effect), CompressorReleaseSeconds) => {
            effect.release_seconds = value;
        }
        (InsertEffect::Compressor(effect), CompressorMakeupGainDb) => {
            effect.makeup_gain_db = value;
        }
        (InsertEffect::Compressor(effect), CompressorMix) => effect.mix = value,
        (InsertEffect::Chorus(effect), ChorusRateHz) => effect.rate_hz = value,
        (InsertEffect::Flanger(effect), FlangerRateHz) => effect.rate_hz = value,
        (InsertEffect::Chorus(effect), ChorusDepthSeconds) => effect.depth_seconds = value,
        (InsertEffect::Flanger(effect), FlangerDepthSeconds) => effect.depth_seconds = value,
        (InsertEffect::Chorus(effect), ChorusDelaySeconds) => effect.delay_seconds = value,
        (InsertEffect::Flanger(effect), FlangerDelaySeconds) => effect.delay_seconds = value,
        (InsertEffect::Chorus(effect), ChorusFeedback) => effect.feedback = value,
        (InsertEffect::Flanger(effect), FlangerFeedback) => effect.feedback = value,
        (InsertEffect::Chorus(effect), ChorusMix) => effect.mix = value,
        (InsertEffect::Flanger(effect), FlangerMix) => effect.mix = value,
        (InsertEffect::Reverb(effect), ReverbRoomSize) => effect.room_size = value,
        (InsertEffect::Reverb(effect), ReverbDamping) => effect.damping = value,
        (InsertEffect::Reverb(effect), ReverbMix) => effect.mix = value,
        _ => return false,
    }
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SendBus {
    pub effect: Option<InsertEffect>,
    pub gain: f32,
}

impl Default for SendBus {
    fn default() -> Self {
        Self {
            effect: None,
            gain: 0.0,
        }
    }
}

impl SendBus {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err(Error::InvalidMix("send gain must be 0..=1".to_owned()));
        }
        if let Some(effect) = &self.effect {
            effect.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn apply_insert(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    effect: &InsertEffect,
) -> Result<(), Error> {
    apply_insert_with_tempo(left, right, sample_rate, effect, &[])
}

pub(crate) fn apply_insert_with_tempo(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    effect: &InsertEffect,
    tempo_changes: &[SynthTempoChange],
) -> Result<(), Error> {
    effect.validate()?;
    let bypass = insert_is_bypassed(effect);
    if bypass {
        return Ok(());
    }
    match effect {
        InsertEffect::Waveshaper(effect) => apply_stereo_waveshaper(left, right, effect),
        InsertEffect::Delay(effect) => {
            apply_stereo_delay_with_tempo(left, right, sample_rate, effect, tempo_changes)
        }
        InsertEffect::Bitcrush(effect) => {
            let original_len = left.len();
            left.resize(original_len + usize::from(effect.downsample) - 1, 0.0);
            right.resize(original_len + usize::from(effect.downsample) - 1, 0.0);
            let mut held = [0.0; 2];
            for (frame, (left, right)) in left.iter_mut().zip(right).enumerate() {
                if frame % usize::from(effect.downsample) == 0 {
                    held = [effect.quantize(*left), effect.quantize(*right)];
                }
                *left = *left + (held[0] - *left) * effect.mix;
                *right = *right + (held[1] - *right) * effect.mix;
            }
        }
        InsertEffect::Equalizer(_) => apply_stateful_insert(
            left,
            right,
            sample_rate,
            effect,
            sample_rate as usize / 20,
            tempo_changes,
        ),
        InsertEffect::Compressor(_) => {
            apply_stateful_insert(left, right, sample_rate, effect, 0, tempo_changes)
        }
        InsertEffect::Chorus(_) | InsertEffect::Flanger(_) | InsertEffect::Reverb(_) => {
            apply_stateful_insert(
                left,
                right,
                sample_rate,
                effect,
                sample_rate as usize * 2,
                tempo_changes,
            )
        }
    }
    Ok(())
}

/// Destructively preprocess mono sample audio with one native insert effect.
/// Effect tails are retained and beat-synced delays use `tempo_bpm` throughout.
pub fn apply_mono_insert(
    samples: &mut Vec<f32>,
    sample_rate: u32,
    effect: &InsertEffect,
    tempo_bpm: f32,
) -> Result<(), Error> {
    if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
        return Err(Error::InvalidMix(
            "sample effect tempo must be finite and positive".to_owned(),
        ));
    }
    let mut right = samples.clone();
    apply_insert_with_tempo(
        samples,
        &mut right,
        sample_rate,
        effect,
        &[SynthTempoChange {
            time_seconds: 0.0,
            bpm: tempo_bpm,
        }],
    )
}

pub(crate) fn insert_is_bypassed(effect: &InsertEffect) -> bool {
    match effect {
        InsertEffect::Bitcrush(effect) => effect.mix == 0.0,
        InsertEffect::Equalizer(effect) => effect.mix == 0.0,
        InsertEffect::Compressor(effect) => effect.mix == 0.0,
        InsertEffect::Chorus(effect) => effect.mix == 0.0,
        InsertEffect::Flanger(effect) => effect.mix == 0.0,
        InsertEffect::Reverb(effect) => effect.mix == 0.0,
        _ => false,
    }
}

pub(crate) fn insert_tail_frames(
    effect: &InsertEffect,
    sample_rate: u32,
    tempo_changes: &[SynthTempoChange],
) -> usize {
    match effect {
        InsertEffect::Waveshaper(_) | InsertEffect::Compressor(_) => 0,
        InsertEffect::Delay(effect) => allocated_tail_frames(
            sample_rate,
            maximum_delay_frames(sample_rate, effect, tempo_changes),
            effect,
        ),
        InsertEffect::Bitcrush(effect) => usize::from(effect.downsample) - 1,
        InsertEffect::Equalizer(_) => sample_rate as usize / 20,
        InsertEffect::Chorus(_) | InsertEffect::Flanger(_) | InsertEffect::Reverb(_) => {
            sample_rate as usize * 2
        }
    }
}

pub(crate) fn apply_insert_automated(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    effect: &InsertEffect,
    tempo_changes: &[SynthTempoChange],
    tail_frames: usize,
    mut automate: impl FnMut(&mut InsertProcessor, f64),
) -> Result<(), Error> {
    effect.validate()?;
    let original_len = left.len();
    left.resize(original_len + tail_frames, 0.0);
    right.resize(original_len + tail_frames, 0.0);
    let mut processor = InsertProcessor::new(effect, sample_rate);
    for (frame, (left, right)) in left.iter_mut().zip(right).enumerate() {
        let mut stereo = [*left, *right];
        let time = frame as f64 / f64::from(sample_rate);
        automate(&mut processor, time);
        processor.process_with_tempo(&mut stereo, sample_rate, tempo_bpm_at(tempo_changes, time));
        *left = stereo[0];
        *right = stereo[1];
    }
    Ok(())
}

fn apply_stateful_insert(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    effect: &InsertEffect,
    tail_frames: usize,
    tempo_changes: &[SynthTempoChange],
) {
    let original_len = left.len();
    left.resize(original_len + tail_frames, 0.0);
    right.resize(original_len + tail_frames, 0.0);
    let mut processor = InsertProcessor::new(effect, sample_rate);
    for (frame, (left, right)) in left.iter_mut().zip(right).enumerate() {
        let mut stereo = [*left, *right];
        let time = frame as f64 / f64::from(sample_rate);
        processor.process_with_tempo(&mut stereo, sample_rate, tempo_bpm_at(tempo_changes, time));
        *left = stereo[0];
        *right = stereo[1];
    }
}

pub(crate) enum InsertProcessor {
    Waveshaper(Waveshaper),
    Delay {
        settings: Delay,
        line: Vec<[f32; 2]>,
        position: usize,
    },
    Bitcrush {
        settings: Bitcrush,
        frame: u64,
        held: [f32; 2],
    },
    Equalizer(EqualizerProcessor),
    Compressor(CompressorProcessor),
    Chorus(ModulatedDelayProcessor),
    Flanger(ModulatedDelayProcessor),
    Reverb(Box<ReverbProcessor>),
}

impl InsertProcessor {
    pub(crate) fn new(effect: &InsertEffect, sample_rate: u32) -> Self {
        match effect {
            InsertEffect::Waveshaper(settings) => Self::Waveshaper(settings.clone()),
            InsertEffect::Delay(settings) => Self::Delay {
                settings: settings.clone(),
                line: vec![[0.0; 2]; sample_rate as usize * 2 + 1],
                position: 0,
            },
            InsertEffect::Bitcrush(settings) => Self::Bitcrush {
                settings: settings.clone(),
                frame: 0,
                held: [0.0; 2],
            },
            InsertEffect::Equalizer(settings) => {
                Self::Equalizer(EqualizerProcessor::new(settings, sample_rate))
            }
            InsertEffect::Compressor(settings) => {
                Self::Compressor(CompressorProcessor::new(settings))
            }
            InsertEffect::Chorus(settings) => {
                Self::Chorus(ModulatedDelayProcessor::chorus(settings, sample_rate))
            }
            InsertEffect::Flanger(settings) => {
                Self::Flanger(ModulatedDelayProcessor::flanger(settings, sample_rate))
            }
            InsertEffect::Reverb(settings) => {
                Self::Reverb(Box::new(ReverbProcessor::new(settings, sample_rate)))
            }
        }
    }

    pub(crate) fn set_parameter(
        &mut self,
        parameter: EffectParameter,
        value: f32,
        sample_rate: u32,
    ) -> bool {
        match self {
            Self::Waveshaper(settings) => {
                let mut effect = InsertEffect::Waveshaper(settings.clone());
                let updated = set_effect_parameter(&mut effect, parameter, value);
                if let InsertEffect::Waveshaper(updated_settings) = effect {
                    *settings = updated_settings;
                }
                updated
            }
            Self::Delay { settings, .. } => {
                let mut effect = InsertEffect::Delay(settings.clone());
                let updated = set_effect_parameter(&mut effect, parameter, value);
                if let InsertEffect::Delay(updated_settings) = effect {
                    *settings = updated_settings;
                }
                updated
            }
            Self::Bitcrush { settings, .. } => {
                let mut effect = InsertEffect::Bitcrush(settings.clone());
                let updated = set_effect_parameter(&mut effect, parameter, value);
                if let InsertEffect::Bitcrush(updated_settings) = effect {
                    *settings = updated_settings;
                }
                updated
            }
            Self::Equalizer(processor) => processor.set_parameter(parameter, value, sample_rate),
            Self::Compressor(processor) => processor.set_parameter(parameter, value),
            Self::Chorus(processor) => processor.set_parameter(parameter, value, true),
            Self::Flanger(processor) => processor.set_parameter(parameter, value, false),
            Self::Reverb(processor) => processor.set_parameter(parameter, value),
        }
    }

    #[cfg(test)]
    pub(crate) fn process(&mut self, stereo: &mut [f32; 2], sample_rate: u32) {
        self.process_with_tempo(stereo, sample_rate, 120.0);
    }

    pub(crate) fn process_with_tempo(
        &mut self,
        stereo: &mut [f32; 2],
        sample_rate: u32,
        tempo_bpm: f32,
    ) {
        match self {
            Self::Waveshaper(settings) => {
                for sample in stereo {
                    *sample = settings.process(*sample);
                }
            }
            Self::Delay {
                settings,
                line,
                position,
            } => {
                let frames = delay_frame_count(sample_rate, settings, tempo_bpm);
                let read = (*position + line.len() - frames) % line.len();
                let delayed = line[read];
                line[*position] =
                    stereo_delay_input(settings.mode, *stereo, delayed, settings.feedback);
                stereo[0] += delayed[0] * settings.mix;
                stereo[1] += delayed[1] * settings.mix;
                *position = (*position + 1) % line.len();
            }
            Self::Bitcrush {
                settings,
                frame,
                held,
            } => {
                if (*frame).is_multiple_of(u64::from(settings.downsample)) {
                    *held = [settings.quantize(stereo[0]), settings.quantize(stereo[1])];
                }
                for (sample, crushed) in stereo.iter_mut().zip(*held) {
                    *sample += (crushed - *sample) * settings.mix;
                }
                *frame = frame.wrapping_add(1);
            }
            Self::Equalizer(processor) => processor.process(stereo),
            Self::Compressor(processor) => processor.process(stereo, sample_rate),
            Self::Chorus(processor) | Self::Flanger(processor) => {
                processor.process(stereo, sample_rate)
            }
            Self::Reverb(processor) => processor.process(stereo),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DelayMode;

    #[test]
    fn bitcrush_quantizes_downsamples_and_mixes_deterministically() {
        let effect = Bitcrush {
            bits: 2,
            downsample: 1,
            mix: 1.0,
        };
        let mut left = vec![-1.0, -0.3, 0.0, 0.3, 1.0];
        let mut right = left.clone();
        apply_insert(
            &mut left,
            &mut right,
            48_000,
            &InsertEffect::Bitcrush(effect.clone()),
        )
        .unwrap();
        assert_eq!(left, [-1.0, -0.5, 0.0, 0.5, 1.0]);
        assert_eq!(right, left);

        let effect = Bitcrush {
            downsample: 2,
            mix: 0.5,
            ..effect
        };
        let mut left = vec![-1.0, -0.3, 0.0, 0.3];
        let mut right = left.clone();
        apply_insert(
            &mut left,
            &mut right,
            48_000,
            &InsertEffect::Bitcrush(effect),
        )
        .unwrap();
        assert_eq!(left, [-1.0, -0.65, 0.0, 0.15, 0.0]);
    }

    #[test]
    fn live_processors_match_the_complete_offline_insert_chain_and_tail() {
        let effects = vec![
            InsertEffect::Bitcrush(Bitcrush {
                bits: 5,
                downsample: 3,
                mix: 0.8,
            }),
            InsertEffect::Waveshaper(Waveshaper::default()),
            InsertEffect::Delay(Delay {
                time_seconds: 0.004,
                feedback: 0.5,
                mix: 0.3,
                mode: DelayMode::StereoCross,
                beats_per_cycle: None,
                maximum_time_seconds: None,
                tail_seconds: 0.0,
            }),
        ];
        let original: Vec<f32> = (0..64)
            .map(|frame| (frame as f32 * 0.17).sin() * 0.4)
            .collect();
        let mut offline_left = original.clone();
        let mut offline_right = original.iter().map(|sample| -sample).collect::<Vec<_>>();
        for effect in &effects {
            apply_insert(&mut offline_left, &mut offline_right, 1_000, effect).unwrap();
        }

        let mut processors: Vec<_> = effects
            .iter()
            .map(|effect| InsertProcessor::new(effect, 1_000))
            .collect();
        let mut live_left = Vec::new();
        let mut live_right = Vec::new();
        for frame in 0..offline_left.len() {
            let dry = original.get(frame).copied().unwrap_or(0.0);
            let mut stereo = [dry, -dry];
            for processor in &mut processors {
                processor.process(&mut stereo, 1_000);
            }
            live_left.push(stereo[0]);
            live_right.push(stereo[1]);
        }
        for frame in 0..offline_left.len() {
            assert_eq!(live_left[frame], offline_left[frame], "left frame {frame}");
            assert_eq!(
                live_right[frame], offline_right[frame],
                "right frame {frame}"
            );
        }
    }

    #[test]
    fn beat_synced_delay_matches_live_across_tempo_changes() {
        let effect = InsertEffect::Delay(Delay {
            beats_per_cycle: Some(1.0),
            feedback: 0.5,
            mix: 1.0,
            mode: DelayMode::Mono,
            ..Delay::default()
        });
        let tempo = [
            SynthTempoChange {
                time_seconds: 0.0,
                bpm: 120.0,
            },
            SynthTempoChange {
                time_seconds: 0.1,
                bpm: 60.0,
            },
            SynthTempoChange {
                time_seconds: 0.2,
                bpm: 240.0,
            },
        ];
        let original = vec![1.0];
        let mut offline_left = original.clone();
        let mut offline_right = original.clone();
        apply_insert_with_tempo(
            &mut offline_left,
            &mut offline_right,
            1_000,
            &effect,
            &tempo,
        )
        .unwrap();
        assert!(offline_left[250] > 0.99);
        assert!(offline_left[500] > 0.49);
        assert!(offline_left[750] > 0.24);

        let mut processor = InsertProcessor::new(&effect, 1_000);
        for frame in 0..offline_left.len() {
            let input = original.get(frame).copied().unwrap_or(0.0);
            let mut stereo = [input, input];
            let time = frame as f64 / 1_000.0;
            processor.process_with_tempo(&mut stereo, 1_000, tempo_bpm_at(&tempo, time));
            assert_eq!(stereo[0], offline_left[frame], "left frame {frame}");
            assert_eq!(stereo[1], offline_right[frame], "right frame {frame}");
        }
    }

    #[test]
    fn equalizer_dynamics_modulation_and_reverb_match_offline_and_live() {
        let effects = vec![
            InsertEffect::Equalizer(Equalizer {
                low_gain_db: 3.0,
                mid_gain_db: -2.0,
                high_gain_db: 1.0,
                mix: 0.8,
            }),
            InsertEffect::Compressor(Compressor::default()),
            InsertEffect::Chorus(Chorus::default()),
            InsertEffect::Flanger(Flanger::default()),
            InsertEffect::Reverb(Reverb::default()),
        ];
        let original: Vec<f32> = (0..256)
            .map(|frame| (frame as f32 * 0.11).sin() * 0.4)
            .collect();
        for effect in &effects {
            let mut offline_left = original.clone();
            let mut offline_right = original.iter().map(|sample| -sample).collect::<Vec<_>>();
            apply_insert(&mut offline_left, &mut offline_right, 8_000, effect).unwrap();
            let mut processor = InsertProcessor::new(effect, 8_000);
            for frame in 0..offline_left.len() {
                let dry = original.get(frame).copied().unwrap_or(0.0);
                let mut stereo = [dry, -dry];
                processor.process(&mut stereo, 8_000);
                assert_eq!(
                    stereo[0], offline_left[frame],
                    "left frame {frame} of {effect:?}"
                );
                assert_eq!(
                    stereo[1], offline_right[frame],
                    "right frame {frame} of {effect:?}"
                );
            }
            assert!(offline_left.iter().all(|sample| sample.is_finite()));
            assert!(offline_right.iter().all(|sample| sample.is_finite()));
        }

        let json = serde_json::to_string(&effects).unwrap();
        let decoded: Vec<InsertEffect> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, effects);
    }

    #[test]
    fn effect_and_send_validation_rejects_invalid_settings() {
        assert!(
            Bitcrush {
                bits: 0,
                ..Bitcrush::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            SendBus {
                gain: 1.1,
                ..SendBus::default()
            }
            .validate()
            .is_err()
        );
        for effect in [
            InsertEffect::Equalizer(Equalizer {
                low_gain_db: 25.0,
                ..Equalizer::default()
            }),
            InsertEffect::Compressor(Compressor {
                ratio: 0.0,
                ..Compressor::default()
            }),
            InsertEffect::Chorus(Chorus {
                rate_hz: 0.0,
                ..Chorus::default()
            }),
            InsertEffect::Flanger(Flanger {
                feedback: 1.0,
                ..Flanger::default()
            }),
            InsertEffect::Reverb(Reverb {
                room_size: f32::NAN,
                ..Reverb::default()
            }),
        ] {
            assert!(effect.validate().is_err());
        }
    }
}
