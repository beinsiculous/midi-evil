use serde::{Deserialize, Serialize};

use crate::{Error, Patch, SynthTempoChange};

const DELAY_TAIL_TAPS: usize = 16;

pub(crate) fn delay_tail_frames(maximum_delay_frames: usize) -> usize {
    maximum_delay_frames * DELAY_TAIL_TAPS
}

pub(crate) fn allocated_tail_frames(
    sample_rate: u32,
    maximum_delay_frames: usize,
    delay: &Delay,
) -> usize {
    if delay.tail_seconds > 0.0 {
        (delay.tail_seconds * sample_rate as f32).round() as usize
    } else {
        delay_tail_frames(maximum_delay_frames)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelayMode {
    #[default]
    Mono,
    StereoCross,
    PingPong,
}

pub(crate) fn stereo_delay_input(
    mode: DelayMode,
    dry: [f32; 2],
    delayed: [f32; 2],
    feedback: f32,
) -> [f32; 2] {
    match mode {
        DelayMode::Mono => [
            dry[0] + delayed[0] * feedback,
            dry[1] + delayed[1] * feedback,
        ],
        DelayMode::StereoCross => [
            dry[1] + delayed[1] * feedback,
            dry[0] + delayed[0] * feedback,
        ],
        DelayMode::PingPong => [
            delayed[1] * feedback,
            (dry[0] + dry[1]) * 0.5 + delayed[0] * feedback,
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Delay {
    pub time_seconds: f32,
    pub feedback: f32,
    pub mix: f32,
    pub mode: DelayMode,
    /// Optional musical delay length. CC 23 automation overrides it with seconds.
    pub beats_per_cycle: Option<f32>,
    /// Optional upper bound for fixed or beat-synced delay time; `None` preserves legacy behavior.
    pub maximum_time_seconds: Option<f32>,
    /// Additional rendered tail in seconds; zero retains the default 16-tap allocation.
    pub tail_seconds: f32,
}

impl Default for Delay {
    fn default() -> Self {
        Self {
            time_seconds: 0.25,
            feedback: 0.35,
            mix: 0.2,
            mode: DelayMode::Mono,
            beats_per_cycle: None,
            maximum_time_seconds: None,
            tail_seconds: 0.0,
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
            || self
                .beats_per_cycle
                .is_some_and(|beats| !beats.is_finite() || !(0.001..=64.0).contains(&beats))
            || self
                .maximum_time_seconds
                .is_some_and(|seconds| !seconds.is_finite() || !(0.001..=2.0).contains(&seconds))
            || !self.tail_seconds.is_finite()
            || !(0.0..=60.0).contains(&self.tail_seconds)
        {
            return Err(Error::InvalidPatch(
                "delay requires time in (0, 2], optional maximum in 0.001..=2, feedback in 0..=0.95, mix in 0..=1, optional beats in 0.001..=64, and tail seconds in 0..=60"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn apply_delay(
    patch: &Patch,
    samples: &mut Vec<f32>,
    tempo_changes: &[SynthTempoChange],
) {
    if let Some(delay) = &patch.delay {
        apply_mono_delay(samples, patch.sample_rate, delay, tempo_changes);
    }
}

pub(crate) fn apply_stereo_delay_with_tempo(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    delay: &Delay,
    tempo_changes: &[SynthTempoChange],
) {
    debug_assert_eq!(left.len(), right.len());
    if delay.mode == DelayMode::Mono {
        apply_mono_delay(left, sample_rate, delay, tempo_changes);
        apply_mono_delay(right, sample_rate, delay, tempo_changes);
        return;
    }

    let maximum_delay = maximum_delay_frames(sample_rate, delay, tempo_changes);
    let original_len = left.len();
    let tail_frames = allocated_tail_frames(sample_rate, maximum_delay, delay);
    left.resize(original_len + tail_frames, 0.0);
    right.resize(original_len + tail_frames, 0.0);
    let mut left_line = vec![0.0; maximum_delay + 1];
    let mut right_line = vec![0.0; maximum_delay + 1];
    let mut position = 0;
    for frame in 0..left.len() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay_frames = delay_frame_count(sample_rate, delay, tempo_bpm_at(tempo_changes, time));
        let read = (position + left_line.len() - delay_frames) % left_line.len();
        let dry_left = left[frame];
        let dry_right = right[frame];
        let delayed_left = left_line[read];
        let delayed_right = right_line[read];
        let input = stereo_delay_input(
            delay.mode,
            [dry_left, dry_right],
            [delayed_left, delayed_right],
            delay.feedback,
        );
        left_line[position] = input[0];
        right_line[position] = input[1];
        left[frame] = dry_left + delayed_left * delay.mix;
        right[frame] = dry_right + delayed_right * delay.mix;
        position = (position + 1) % left_line.len();
    }
}

fn apply_mono_delay(
    samples: &mut Vec<f32>,
    sample_rate: u32,
    delay: &Delay,
    tempo_changes: &[SynthTempoChange],
) {
    let maximum_delay = maximum_delay_frames(sample_rate, delay, tempo_changes);
    let original_len = samples.len();
    samples.resize(
        original_len + allocated_tail_frames(sample_rate, maximum_delay, delay),
        0.0,
    );
    let mut delay_line = vec![0.0; maximum_delay + 1];
    let mut position = 0;
    for (frame, sample) in samples.iter_mut().enumerate() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay_frames = delay_frame_count(sample_rate, delay, tempo_bpm_at(tempo_changes, time));
        let read = (position + delay_line.len() - delay_frames) % delay_line.len();
        let dry = *sample;
        let delayed = delay_line[read];
        delay_line[position] = dry + delayed * delay.feedback;
        *sample = dry + delayed * delay.mix;
        position = (position + 1) % delay_line.len();
    }
}

pub(crate) fn delay_frame_count(sample_rate: u32, delay: &Delay, tempo_bpm: f32) -> usize {
    let seconds = delay.beats_per_cycle.map_or(delay.time_seconds, |beats| {
        (beats * 60.0 / tempo_bpm).clamp(0.001, 2.0)
    });
    let seconds = delay
        .maximum_time_seconds
        .map_or(seconds, |maximum| seconds.min(maximum));
    ((seconds * sample_rate as f32).round() as usize).max(1)
}

pub(crate) fn maximum_delay_frames(
    sample_rate: u32,
    delay: &Delay,
    tempo_changes: &[SynthTempoChange],
) -> usize {
    tempo_changes.iter().fold(
        delay_frame_count(sample_rate, delay, tempo_bpm_at(tempo_changes, 0.0)),
        |maximum, event| maximum.max(delay_frame_count(sample_rate, delay, event.bpm)),
    )
}

pub(crate) fn tempo_bpm_at(tempo_changes: &[SynthTempoChange], time_seconds: f64) -> f32 {
    let index = tempo_changes.partition_point(|event| event.time_seconds <= time_seconds);
    tempo_changes
        .get(index.saturating_sub(1))
        .map_or(120.0, |event| event.bpm)
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
        let delay_frames = (0.02 * 48_000.0) as usize;
        assert_eq!(sound.samples.len(), dry_frames + 16 * delay_frames);
        assert!(sound.samples.len() > dry_frames + 480);
        assert!(
            sound.samples[dry_frames..]
                .iter()
                .any(|sample| sample.abs() > 0.001)
        );
        assert!(
            sound.samples[dry_frames + 8 * delay_frames..dry_frames + 9 * delay_frames]
                .iter()
                .any(|sample| sample.abs() > 0.000_001)
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

    #[test]
    fn explicit_tail_seconds_extends_allocation_without_changing_the_legacy_default() {
        let legacy = Delay {
            time_seconds: 0.02,
            feedback: 0.88,
            ..Delay::default()
        };
        let extended = Delay {
            tail_seconds: 1.0,
            ..legacy.clone()
        };
        assert_eq!(allocated_tail_frames(48_000, 960, &legacy), 16 * 960);
        assert_eq!(allocated_tail_frames(48_000, 960, &extended), 48_000);
        assert!(
            Delay {
                tail_seconds: -0.01,
                ..legacy
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn beat_sync_follows_tempo_changes_and_retains_seconds_by_default() {
        let delay = Delay {
            beats_per_cycle: Some(1.0),
            ..Delay::default()
        };
        assert_eq!(delay_frame_count(1_000, &delay, 120.0), 500);
        assert_eq!(delay_frame_count(1_000, &delay, 60.0), 1_000);
        assert_eq!(delay_frame_count(1_000, &delay, 240.0), 250);
        let legacy = Delay::default();
        assert_eq!(delay_frame_count(1_000, &legacy, 60.0), 250);

        let mut left = vec![1.0];
        let mut right = vec![1.0];
        apply_stereo_delay_with_tempo(
            &mut left,
            &mut right,
            1_000,
            &delay,
            &[SynthTempoChange {
                time_seconds: 0.0,
                bpm: 120.0,
            }],
        );
        assert_eq!(left[500], 0.2);
        assert_eq!(right[500], 0.2);

        let json = serde_json::to_string(&delay).unwrap();
        assert_eq!(serde_json::from_str::<Delay>(&json).unwrap(), delay);
        let legacy: Delay = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy, Delay::default());
        assert!(
            Delay {
                beats_per_cycle: Some(0.0),
                ..Delay::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn maximum_time_caps_fixed_and_beat_synced_delay_audio() {
        let capped = Delay {
            time_seconds: 1.0,
            beats_per_cycle: Some(1.0),
            maximum_time_seconds: Some(0.5),
            ..Delay::default()
        };
        assert_eq!(delay_frame_count(1_000, &capped, 60.0), 500);
        assert_eq!(delay_frame_count(1_000, &capped, 120.0), 500);
        assert_eq!(delay_frame_count(1_000, &capped, 240.0), 250);
        let fixed = Delay {
            beats_per_cycle: None,
            ..capped.clone()
        };
        assert_eq!(delay_frame_count(1_000, &fixed, 240.0), 500);
        let uncapped_short = Delay {
            time_seconds: 0.00025,
            ..Delay::default()
        };
        assert_eq!(delay_frame_count(8_000, &uncapped_short, 120.0), 2);

        let mut left = vec![1.0];
        let mut right = vec![1.0];
        apply_stereo_delay_with_tempo(
            &mut left,
            &mut right,
            1_000,
            &capped,
            &[SynthTempoChange {
                time_seconds: 0.0,
                bpm: 60.0,
            }],
        );
        assert_eq!(left[250], 0.0);
        assert_eq!(left[500], 0.2);
        assert_eq!(right[500], 0.2);

        let mut fast_left = vec![1.0];
        let mut fast_right = vec![1.0];
        apply_stereo_delay_with_tempo(
            &mut fast_left,
            &mut fast_right,
            1_000,
            &capped,
            &[SynthTempoChange {
                time_seconds: 0.0,
                bpm: 240.0,
            }],
        );
        assert_eq!(fast_left[250], 0.2);
        assert_eq!(fast_right[250], 0.2);

        let json = serde_json::to_string(&capped).unwrap();
        assert_eq!(serde_json::from_str::<Delay>(&json).unwrap(), capped);
        for maximum in [0.001, 2.0] {
            assert!(
                Delay {
                    maximum_time_seconds: Some(maximum),
                    ..Delay::default()
                }
                .validate()
                .is_ok()
            );
        }
        for maximum in [0.0, 0.000_9, 2.001, f32::NAN] {
            assert!(
                Delay {
                    maximum_time_seconds: Some(maximum),
                    ..Delay::default()
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn ping_pong_delay_alternates_centered_echoes() {
        let delay = Delay {
            time_seconds: 0.05,
            feedback: 0.5,
            mix: 1.0,
            mode: DelayMode::PingPong,
            ..Delay::default()
        };
        let mut left = vec![1.0];
        let mut right = vec![1.0];
        apply_stereo_delay_with_tempo(&mut left, &mut right, 1_000, &delay, &[]);

        assert_eq!(left[50], 0.0);
        assert_eq!(right[50], 1.0);
        assert_eq!(left[100], 0.5);
        assert_eq!(right[100], 0.0);
        assert_eq!(left[150], 0.0);
        assert_eq!(right[150], 0.25);
        assert_eq!(left[200], 0.125);
        assert_eq!(right[200], 0.0);
    }
}
