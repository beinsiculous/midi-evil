//! Sample-pad voices for original drum kits and TRG-style performance.
//! No NanoStudio project or preset data is required by this crate.

use evil_synth::{Envelope, RenderedSound};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod kit;
mod machine;
mod presets;
mod render;
pub use kit::{DrumKit, KitPad, PAD_COUNT};
pub use machine::{DrumMachine, PreparedPad};
pub use presets::maxwell_kit;
pub use render::{KitHit, render_kit};

const MAX_RENDER_SECONDS: f64 = 3_600.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPad {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub gain: f32,
    pub tuning_semitones: f32,
    pub envelope: Envelope,
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
        }
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
    /// play the complete resampled pad; `Some(seconds)` adds an early release.
    pub fn render(&self, hits: &[DrumHit]) -> Result<RenderedSound, Error> {
        self.validate()?;
        for (index, hit) in hits.iter().enumerate() {
            hit.validate(index)?;
        }
        let rate = 2.0_f32.powf(self.tuning_semitones / 12.0);
        let source_seconds =
            self.samples.len() as f64 / f64::from(self.sample_rate) / f64::from(rate);
        let frame_count = hits.iter().try_fold(0_usize, |frames, hit| {
            let gate = hit.duration_seconds.unwrap_or(source_seconds);
            let end = hit.start_seconds + gate + f64::from(self.envelope.release_seconds);
            if end > MAX_RENDER_SECONDS {
                return Err(Error::RenderTooLong);
            }
            let start_frame = (hit.start_seconds * f64::from(self.sample_rate)).round() as usize;
            let hit_frames = ((gate + f64::from(self.envelope.release_seconds))
                * f64::from(self.sample_rate))
            .ceil() as usize;
            Ok(frames.max(start_frame + hit_frames))
        })?;
        let mut output = vec![0.0; frame_count];
        for hit in hits {
            let start_frame = (hit.start_seconds * f64::from(self.sample_rate)).round() as usize;
            let gate = hit.duration_seconds.unwrap_or(source_seconds);
            let hit_frames = ((gate + f64::from(self.envelope.release_seconds))
                * f64::from(self.sample_rate))
            .ceil() as usize;
            let velocity = f32::from(hit.velocity) / 127.0;
            for offset in 0..hit_frames {
                let source_position = offset as f64 * f64::from(rate);
                let index = source_position.floor() as usize;
                if index >= self.samples.len() {
                    break;
                }
                let fraction = source_position.fract() as f32;
                let next = (index + 1).min(self.samples.len() - 1);
                let sample =
                    self.samples[index] + (self.samples[next] - self.samples[index]) * fraction;
                let time = offset as f32 / self.sample_rate as f32;
                output[start_frame + offset] +=
                    sample * self.envelope.amplitude(time, gate as f32) * self.gain * velocity;
            }
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
    #[error("drum render exceeds the maximum length of one hour")]
    RenderTooLong,
}

#[cfg(test)]
mod tests {
    use super::*;

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
