use serde::{Deserialize, Serialize};

use crate::{Error, Patch};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelayMode {
    #[default]
    Mono,
    StereoCross,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Delay {
    pub time_seconds: f32,
    pub feedback: f32,
    pub mix: f32,
    pub mode: DelayMode,
}

impl Default for Delay {
    fn default() -> Self {
        Self {
            time_seconds: 0.25,
            feedback: 0.35,
            mix: 0.2,
            mode: DelayMode::Mono,
        }
    }
}

impl Delay {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.time_seconds.is_finite()
            || self.time_seconds <= 0.0
            || self.time_seconds > 2.0
            || !self.feedback.is_finite()
            || !(0.0..=0.95).contains(&self.feedback)
            || !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
        {
            return Err(Error::InvalidPatch(
                "delay requires time in (0, 2], feedback in 0..=0.95, and mix in 0..=1".to_owned(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn apply_delay(patch: &Patch, samples: &mut Vec<f32>) {
    if let Some(delay) = &patch.delay {
        apply_mono_delay(samples, patch.sample_rate, delay);
    }
}

pub(crate) fn apply_stereo_delay(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    delay: &Delay,
) {
    debug_assert_eq!(left.len(), right.len());
    if delay.mode == DelayMode::Mono {
        apply_mono_delay(left, sample_rate, delay);
        apply_mono_delay(right, sample_rate, delay);
        return;
    }

    let delay_frames = delay_frame_count(sample_rate, delay);
    let original_len = left.len();
    left.resize(original_len + delay_frames * 8, 0.0);
    right.resize(original_len + delay_frames * 8, 0.0);
    let mut left_line = vec![0.0; left.len() + delay_frames];
    let mut right_line = vec![0.0; right.len() + delay_frames];
    for frame in 0..left.len() {
        let dry_left = left[frame];
        let dry_right = right[frame];
        let delayed_left = left_line[frame];
        let delayed_right = right_line[frame];
        left[frame] = dry_left + delayed_left * delay.mix;
        right[frame] = dry_right + delayed_right * delay.mix;
        left_line[frame + delay_frames] = dry_right + delayed_right * delay.feedback;
        right_line[frame + delay_frames] = dry_left + delayed_left * delay.feedback;
    }
}

fn apply_mono_delay(samples: &mut Vec<f32>, sample_rate: u32, delay: &Delay) {
    let delay_frames = delay_frame_count(sample_rate, delay);
    let original_len = samples.len();
    samples.resize(original_len + delay_frames * 8, 0.0);
    let mut delay_line = vec![0.0; samples.len() + delay_frames];
    for frame in 0..samples.len() {
        let dry = samples[frame];
        let delayed = delay_line[frame];
        samples[frame] = dry + delayed * delay.mix;
        delay_line[frame + delay_frames] = dry + delayed * delay.feedback;
    }
}

fn delay_frame_count(sample_rate: u32, delay: &Delay) -> usize {
    ((delay.time_seconds * sample_rate as f32).round() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SynthNote, render, render_notes_dry};

    #[test]
    fn delay_extends_the_sound_with_feedback_taps() {
        let patch = Patch {
            duration_seconds: 0.01,
            envelope: crate::Envelope {
                release_seconds: 0.0,
                ..crate::Envelope::default()
            },
            delay: Some(Delay {
                time_seconds: 0.02,
                feedback: 0.5,
                mix: 0.5,
                ..Delay::default()
            }),
            ..Patch::default()
        };
        let sound = render(&patch).unwrap();
        let dry_frames = (0.01 * 48_000.0) as usize;
        assert!(sound.samples.len() > dry_frames + 480);
        assert!(
            sound.samples[dry_frames..]
                .iter()
                .any(|sample| sample.abs() > 0.001)
        );

        let dry = render_notes_dry(
            &patch,
            &[SynthNote {
                start_seconds: 0.0,
                duration_seconds: 0.01,
                pitch: 69,
                velocity: 100,
            }],
        )
        .unwrap();
        assert_eq!(dry.samples.len(), dry_frames);
        assert!(sound.samples.len() > dry.samples.len());
    }
}
