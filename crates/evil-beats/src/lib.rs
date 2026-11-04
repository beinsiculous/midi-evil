//! Sample-pad voices for original drum kits and TRG-style performance.
//! No NanoStudio project or preset data is required by this crate.

use std::{ops::Range, sync::Arc};

use evil_synth::{Envelope, RenderedSound};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod kit;
mod machine;
mod presets;
mod render;
mod routing;
mod voice;
pub use kit::{DrumKit, KitPad, PAD_COUNT};
pub use machine::{DrumMachine, PreparedPad};
pub use presets::maxwell_kit;
pub use render::{KitHit, render_kit};
pub use routing::{BUS_COUNT, DrumBus, DrumFilter};
pub use voice::VOICE_LIMIT;

const MAX_RENDER_SECONDS: f64 = 3_600.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleDirection {
    #[default]
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPad {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub gain: f32,
    pub tuning_semitones: f32,
    pub envelope: Envelope,
    /// First source frame to play. Legacy banks default to the full sample.
    #[serde(default)]
    pub playback_start_frame: usize,
    /// Exclusive playback end. `None` uses the source frame count.
    #[serde(default)]
    pub playback_end_frame: Option<usize>,
    #[serde(default)]
    pub direction: SampleDirection,
    #[serde(default)]
    pub loop_enabled: bool,
    /// Independent per-voice filtering; `None` preserves legacy dry playback.
    #[serde(default)]
    pub filter: Option<DrumFilter>,
}

impl DrumPad {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            name: "Pad".to_owned(),
            samples,
            sample_rate,
            gain: 0.8,
            tuning_semitones: 0.0,
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.05,
            },
            playback_start_frame: 0,
            playback_end_frame: None,
            direction: SampleDirection::Forward,
            loop_enabled: false,
            filter: None,
        }
    }

    /// The selected source region, with an exclusive end frame.
    pub fn playback_range(&self) -> Range<usize> {
        self.playback_start_frame..self.playback_end_frame.unwrap_or(self.samples.len())
    }

    /// Destructively crop the embedded audio to the non-destructive trim range.
    /// Returns `false` when the range already covers the complete source.
    pub fn crop_to_playback_range(&mut self) -> Result<bool, Error> {
        self.validate()?;
        let playback = self.playback_range();
        if playback.start == 0 && playback.end == self.samples.len() {
            return Ok(false);
        }
        self.samples = self.samples[playback].to_vec();
        self.playback_start_frame = 0;
        self.playback_end_frame = None;
        Ok(true)
    }

    /// Peak-normalize the complete embedded source without changing pad gain.
    /// Returns `false` for silence or an already unity-peak source.
    pub fn normalize(&mut self) -> Result<bool, Error> {
        self.validate()?;
        let peak = self
            .samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        if peak == 0.0 || peak == 1.0 {
            return Ok(false);
        }
        for sample in &mut self.samples {
            *sample /= peak;
        }
        self.validate()?;
        Ok(true)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !(8_000..=192_000).contains(&self.sample_rate) {
            return Err(Error::InvalidPad(
                "sample_rate must be 8000..=192000".to_owned(),
            ));
        }
        if self.samples.is_empty() || self.samples.iter().any(|sample| !sample.is_finite()) {
            return Err(Error::InvalidPad(
                "samples must be nonempty and finite".to_owned(),
            ));
        }
        let playback = self.playback_range();
        if playback.start >= playback.end || playback.end > self.samples.len() {
            return Err(Error::InvalidPad(
                "playback points must satisfy 0 <= start < end <= source frame count".to_owned(),
            ));
        }
        if let Some(filter) = self.filter {
            filter.validate()?;
        }
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err(Error::InvalidPad("gain must be between 0 and 1".to_owned()));
        }
        if !self.tuning_semitones.is_finite() || !(-48.0..=48.0).contains(&self.tuning_semitones) {
            return Err(Error::InvalidPad(
                "tuning_semitones must be between -48 and 48".to_owned(),
            ));
        }
        for duration in [
            self.envelope.attack_seconds,
            self.envelope.decay_seconds,
            self.envelope.release_seconds,
        ] {
            if !duration.is_finite() || !(0.0..=60.0).contains(&duration) {
                return Err(Error::InvalidPad(
                    "envelope durations must be between 0 and 60".to_owned(),
                ));
            }
        }
        if !self.envelope.sustain_level.is_finite()
            || !(0.0..=1.0).contains(&self.envelope.sustain_level)
        {
            return Err(Error::InvalidPad(
                "envelope sustain_level must be between 0 and 1".to_owned(),
            ));
        }
        Ok(())
    }

    /// Render independent, overlapping hits into one mono stream. `None` gates
    /// play the complete trimmed pad; looped hits need a finite gate.
    pub fn render(&self, hits: &[DrumHit]) -> Result<RenderedSound, Error> {
        self.validate()?;
        for (index, hit) in hits.iter().enumerate() {
            hit.validate(index)?;
        }

        let pad = Arc::new(self.clone());
        let mut scheduled = Vec::with_capacity(hits.len());
        for (index, hit) in hits.iter().enumerate() {
            let voice = voice::PlaybackVoice::new(
                pad.clone(),
                0,
                0,
                self.sample_rate,
                hit.velocity,
                hit.duration_seconds,
            );
            let duration = voice.duration_seconds().ok_or(Error::UngatedLoop(index))?;
            if hit.start_seconds + duration > MAX_RENDER_SECONDS {
                return Err(Error::RenderTooLong);
            }
            let start_frame = (hit.start_seconds * f64::from(self.sample_rate)).round() as usize;
            scheduled.push((start_frame, index, voice));
        }
        scheduled.sort_by_key(|(start, index, _)| (*start, *index));

        let mut output = Vec::new();
        let mut active: Vec<voice::PlaybackVoice> = Vec::with_capacity(voice::VOICE_LIMIT);
        let mut scheduled = scheduled.into_iter().peekable();
        while !active.is_empty() || scheduled.peek().is_some() {
            active.retain(|voice| !voice.is_finished());
            if active.is_empty()
                && let Some((start, _, _)) = scheduled.peek()
                && *start > output.len()
            {
                output.resize(*start, 0.0);
            }
            let frame = output.len();
            if frame as f64 >= MAX_RENDER_SECONDS * f64::from(self.sample_rate) {
                return Err(Error::RenderTooLong);
            }
            while scheduled
                .peek()
                .is_some_and(|(start, _, _)| *start == frame)
            {
                let (_, _, voice) = scheduled.next().unwrap();
                voice::allocate_voice(&mut active, voice);
            }
            let mut mixed = 0.0;
            for voice in &mut active {
                if let Some(sample) = voice.next_sample() {
                    mixed += sample * self.gain;
                }
            }
            output.push(mixed);
            active.retain(|voice| !voice.is_finished());
        }
        Ok(RenderedSound {
            samples: output,
            sample_rate: self.sample_rate,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrumHit {
    pub start_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub velocity: u8,
}

impl DrumHit {
    fn validate(&self, index: usize) -> Result<(), Error> {
        if !self.start_seconds.is_finite() || self.start_seconds < 0.0 {
            return Err(Error::InvalidHit(index));
        }
        if self.velocity == 0 || self.velocity > 127 {
            return Err(Error::InvalidHit(index));
        }
        if self
            .duration_seconds
            .is_some_and(|duration| !duration.is_finite() || duration <= 0.0)
        {
            return Err(Error::InvalidHit(index));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid drum bank: {0}")]
    InvalidKit(String),
    #[error("could not read drum bank: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not read WAV: {0}")]
    Wav(#[from] hound::Error),
    #[error("could not synthesize drum: {0}")]
    Synth(#[from] evil_synth::Error),
    #[error("invalid drum pad: {0}")]
    InvalidPad(String),
    #[error("invalid drum hit at index {0}")]
    InvalidHit(usize),
    #[error("looped drum hit at index {0} needs a finite gate for offline rendering")]
    UngatedLoop(usize),
    #[error("drum render exceeds the maximum length of one hour")]
    RenderTooLong,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_and_normalize_edit_audio_without_changing_playback_modes() {
        let mut pad = DrumPad::new(vec![0.0, 0.25, -0.5, 0.75], 48_000);
        pad.playback_start_frame = 1;
        pad.playback_end_frame = Some(3);
        pad.direction = SampleDirection::Reverse;
        pad.loop_enabled = true;
        pad.gain = 1.0;
        pad.envelope.release_seconds = 0.0;
        let hit = DrumHit {
            start_seconds: 0.0,
            duration_seconds: Some(6.0 / 48_000.0),
            velocity: 127,
        };
        let trimmed_render = pad.render(&[hit]).unwrap();
        assert!(pad.crop_to_playback_range().unwrap());
        assert_eq!(pad.samples, [0.25, -0.5]);
        assert_eq!(pad.playback_range(), 0..2);
        assert_eq!(pad.direction, SampleDirection::Reverse);
        assert!(pad.loop_enabled);
        assert!(!pad.crop_to_playback_range().unwrap());
        assert_eq!(pad.render(&[hit]).unwrap().samples, trimmed_render.samples);

        assert!(pad.normalize().unwrap());
        assert_eq!(pad.samples, [0.5, -1.0]);
        let normalized_render = pad.render(&[hit]).unwrap();
        for (normalized, original) in normalized_render.samples.iter().zip(trimmed_render.samples) {
            assert!((normalized - 2.0 * original).abs() < 1e-6);
        }
        assert!(!pad.normalize().unwrap());
        let mut silent = DrumPad::new(vec![0.0, 0.0], 48_000);
        assert!(!silent.normalize().unwrap());
        assert_eq!(silent.samples, [0.0, 0.0]);
    }

    #[test]
    fn tiny_sources_normalize_without_overflow() {
        let mut pad = DrumPad::new(vec![0.0, f32::from_bits(1)], 48_000);
        assert!(pad.normalize().unwrap());
        assert_eq!(pad.samples, [0.0, 1.0]);
        assert!(pad.samples.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn trim_direction_and_loop_modes_share_exact_playback_boundaries() {
        let mut pad = DrumPad::new(vec![0.0, 0.25, 0.5, 0.75, 1.0], 48_000);
        pad.gain = 1.0;
        pad.envelope.release_seconds = 0.0;
        pad.playback_start_frame = 1;
        pad.playback_end_frame = Some(4);
        let hit = DrumHit {
            start_seconds: 0.0,
            duration_seconds: None,
            velocity: 127,
        };
        assert_eq!(pad.render(&[hit]).unwrap().samples, [0.25, 0.5, 0.75]);

        pad.direction = SampleDirection::Reverse;
        assert_eq!(pad.render(&[hit]).unwrap().samples, [0.75, 0.5, 0.25]);

        pad = DrumPad::new(vec![0.0, 0.5], 8_000);
        pad.gain = 1.0;
        pad.envelope.release_seconds = 0.0;
        pad.loop_enabled = true;
        let loop_hit = DrumHit {
            duration_seconds: Some(5.0 / 8_000.0),
            ..hit
        };
        assert_eq!(
            pad.render(&[loop_hit]).unwrap().samples,
            [0.0, 0.5, 0.0, 0.5, 0.0]
        );
        pad.direction = SampleDirection::Reverse;
        assert_eq!(
            pad.render(&[loop_hit]).unwrap().samples,
            [0.5, 0.0, 0.5, 0.0, 0.5]
        );
    }

    #[test]
    fn ungated_offline_loops_and_invalid_trim_ranges_are_rejected() {
        let mut pad = DrumPad::new(vec![0.0, 0.5], 8_000);
        pad.loop_enabled = true;
        let hit = DrumHit {
            start_seconds: 0.0,
            duration_seconds: None,
            velocity: 127,
        };
        assert!(matches!(pad.render(&[hit]), Err(Error::UngatedLoop(0))));
        pad.playback_start_frame = 2;
        assert!(pad.validate().is_err());
        pad.playback_start_frame = 0;
        pad.playback_end_frame = Some(3);
        assert!(pad.validate().is_err());
    }

    #[test]
    fn one_shots_keep_their_configured_gate_tail_after_the_source_ends() {
        let mut pad = DrumPad::new(vec![1.0, 1.0], 8_000);
        pad.gain = 1.0;
        pad.envelope.release_seconds = 0.0;
        let output = pad
            .render(&[DrumHit {
                start_seconds: 0.0,
                duration_seconds: Some(5.0 / 8_000.0),
                velocity: 127,
            }])
            .unwrap();
        assert_eq!(output.samples, [1.0, 1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn offline_polyphony_is_bounded_to_32_oldest_first_voices() {
        let mut pad = DrumPad::new(vec![1.0; 64], 48_000);
        pad.gain = 1.0;
        pad.envelope.release_seconds = 0.0;
        let hits = vec![
            DrumHit {
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            };
            VOICE_LIMIT + 1
        ];
        let output = pad.render(&hits).unwrap();
        assert_eq!(output.samples[0], VOICE_LIMIT as f32);
    }

    #[test]
    fn overlapping_hits_are_mixed_as_independent_voices() {
        let pad = DrumPad::new(vec![0.5; 100], 48_000);
        let hit = DrumHit {
            start_seconds: 0.0,
            duration_seconds: None,
            velocity: 127,
        };
        let one = pad.render(&[hit]).unwrap();
        let two = pad.render(&[hit, hit]).unwrap();
        assert!((two.samples[0] - 2.0 * one.samples[0]).abs() < 0.000_001);
    }

    #[test]
    fn tuning_changes_resampled_length_and_short_gates_release_early() {
        let mut pad = DrumPad::new((0..100).map(|index| index as f32 / 100.0).collect(), 48_000);
        let hit = DrumHit {
            start_seconds: 0.0,
            duration_seconds: None,
            velocity: 127,
        };
        let normal = pad.render(&[hit]).unwrap();
        pad.tuning_semitones = 12.0;
        let octave = pad.render(&[hit]).unwrap();
        assert!(octave.samples.len() < normal.samples.len());
        pad.envelope.release_seconds = 0.1;
        let gated = pad
            .render(&[DrumHit {
                duration_seconds: Some(0.0001),
                ..hit
            }])
            .unwrap();
        assert!(gated.samples.len() > octave.samples.len());
        assert!(gated.samples.contains(&0.0));
    }

    #[test]
    fn fractional_hit_times_stay_within_the_output_buffer() {
        let pad = DrumPad::new(vec![0.5; 100], 48_000);
        for index in 0..20 {
            let sound = pad
                .render(&[DrumHit {
                    start_seconds: index as f64 / 96_000.0,
                    duration_seconds: Some(1.0 / 96_000.0),
                    velocity: 127,
                }])
                .unwrap();
            assert!(sound.samples.iter().all(|sample| sample.is_finite()));
        }
    }

    #[test]
    fn invalid_pads_and_hits_are_rejected() {
        assert!(DrumPad::new(Vec::new(), 48_000).render(&[]).is_err());
        let pad = DrumPad::new(vec![0.0; 10], 48_000);
        assert!(
            pad.render(&[DrumHit {
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 0,
            }])
            .is_err()
        );
    }
}
