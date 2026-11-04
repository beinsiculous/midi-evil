use serde::{Deserialize, Serialize};

use crate::delay::{apply_stereo_delay_with_tempo, delay_frame_count, tempo_bpm_at};
use crate::dynamics::CompressorProcessor;
use crate::equalizer::EqualizerProcessor;
use crate::modulation::ModulatedDelayProcessor;
use crate::reverb::ReverbProcessor;
use crate::waveshaper::apply_stereo_waveshaper;
use crate::{
    Chorus, Compressor, Delay, Equalizer, Error, Flanger, Reverb, SynthTempoChange, Waveshaper,
};

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
    let bypass = match effect {
        InsertEffect::Bitcrush(effect) => effect.mix == 0.0,
        InsertEffect::Equalizer(effect) => effect.mix == 0.0,
        InsertEffect::Compressor(effect) => effect.mix == 0.0,
        InsertEffect::Chorus(effect) => effect.mix == 0.0,
        InsertEffect::Flanger(effect) => effect.mix == 0.0,
        InsertEffect::Reverb(effect) => effect.mix == 0.0,
        _ => false,
    };
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
                line[*position] = if settings.mode == crate::DelayMode::StereoCross {
                    [
                        stereo[1] + delayed[1] * settings.feedback,
                        stereo[0] + delayed[0] * settings.feedback,
                    ]
                } else {
                    [
                        stereo[0] + delayed[0] * settings.feedback,
                        stereo[1] + delayed[1] * settings.feedback,
                    ]
                };
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
