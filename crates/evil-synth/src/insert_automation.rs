use crate::delay::{
    allocated_tail_frames, delay_frame_count, maximum_delay_frames, stereo_delay_input,
    tempo_bpm_at,
};
use crate::{Delay, SynthController, SynthTempoChange, Waveshaper};

pub const WAVESHAPER_DRIVE_CC: u8 = 20;
pub const WAVESHAPER_CONTOUR_CC: u8 = 21;
pub const WAVESHAPER_LEVEL_CC: u8 = 22;
pub const DELAY_TIME_CC: u8 = 23;
pub const DELAY_FEEDBACK_CC: u8 = 24;
pub const DELAY_MIX_CC: u8 = 25;

pub fn controller_is_insert_automation(controller: u8) -> bool {
    (WAVESHAPER_DRIVE_CC..=DELAY_MIX_CC).contains(&controller)
}

pub(crate) fn has_waveshaper_automation(controllers: &[SynthController]) -> bool {
    controllers
        .iter()
        .any(|event| (WAVESHAPER_DRIVE_CC..=WAVESHAPER_LEVEL_CC).contains(&event.controller))
}

pub(crate) fn has_delay_automation(controllers: &[SynthController]) -> bool {
    controllers
        .iter()
        .any(|event| (DELAY_TIME_CC..=DELAY_MIX_CC).contains(&event.controller))
}

pub(crate) fn apply_waveshaper_automation(
    samples: &mut [f32],
    sample_rate: u32,
    base: &Waveshaper,
    controllers: &[SynthController],
) {
    for (frame, sample) in samples.iter_mut().enumerate() {
        let time = frame as f64 / f64::from(sample_rate);
        *sample = automated_waveshaper(base, controllers, time).process(*sample);
    }
}

pub(crate) fn apply_stereo_waveshaper_automation(
    left: &mut [f32],
    right: &mut [f32],
    sample_rate: u32,
    base: &Waveshaper,
    controllers: &[SynthController],
) {
    debug_assert_eq!(left.len(), right.len());
    for (frame, (left, right)) in left.iter_mut().zip(right).enumerate() {
        let time = frame as f64 / f64::from(sample_rate);
        let waveshaper = automated_waveshaper(base, controllers, time);
        *left = waveshaper.process(*left);
        *right = waveshaper.process(*right);
    }
}

pub(crate) fn apply_delay_automation(
    samples: &mut Vec<f32>,
    sample_rate: u32,
    base: &Delay,
    controllers: &[SynthController],
    tempo_changes: &[SynthTempoChange],
) {
    let maximum_delay =
        maximum_automated_delay_frames(sample_rate, base, controllers, tempo_changes);
    let original_len = samples.len();
    samples.resize(
        original_len + allocated_tail_frames(sample_rate, maximum_delay, base),
        0.0,
    );
    let mut delay_line = vec![0.0; maximum_delay + 1];
    let mut position = 0;
    for (frame, sample) in samples.iter_mut().enumerate() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay = automated_delay(base, controllers, time);
        let delay_frames =
            delay_frame_count(sample_rate, &delay, tempo_bpm_at(tempo_changes, time));
        let read = (position + delay_line.len() - delay_frames) % delay_line.len();
        let dry = *sample;
        let delayed = delay_line[read];
        delay_line[position] = dry + delayed * delay.feedback;
        *sample = dry + delayed * delay.mix;
        position = (position + 1) % delay_line.len();
    }
}

pub(crate) fn apply_stereo_delay_automation_with_tempo(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    base: &Delay,
    controllers: &[SynthController],
    tempo_changes: &[SynthTempoChange],
) {
    debug_assert_eq!(left.len(), right.len());
    let maximum_delay =
        maximum_automated_delay_frames(sample_rate, base, controllers, tempo_changes);
    let original_len = left.len();
    let tail_frames = allocated_tail_frames(sample_rate, maximum_delay, base);
    left.resize(original_len + tail_frames, 0.0);
    right.resize(original_len + tail_frames, 0.0);
    let mut left_line = vec![0.0; maximum_delay + 1];
    let mut right_line = vec![0.0; maximum_delay + 1];
    let mut position = 0;
    for frame in 0..left.len() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay = automated_delay(base, controllers, time);
        let delay_frames =
            delay_frame_count(sample_rate, &delay, tempo_bpm_at(tempo_changes, time));
        let read = (position + left_line.len() - delay_frames) % left_line.len();
        let dry_left = left[frame];
        let dry_right = right[frame];
        let delayed_left = left_line[read];
        let delayed_right = right_line[read];
        let input = stereo_delay_input(
            base.mode,
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

fn automated_waveshaper(
    base: &Waveshaper,
    controllers: &[SynthController],
    time_seconds: f64,
) -> Waveshaper {
    let mut waveshaper = base.clone();
    if let Some(value) = controller_value(controllers, WAVESHAPER_DRIVE_CC, time_seconds) {
        waveshaper.drive = 1.0 + value * 31.0;
    }
    if let Some(value) = controller_value(controllers, WAVESHAPER_CONTOUR_CC, time_seconds) {
        waveshaper.contour = value * 2.0 - 1.0;
    }
    if let Some(value) = controller_value(controllers, WAVESHAPER_LEVEL_CC, time_seconds) {
        waveshaper.level = value;
    }
    waveshaper
}

fn automated_delay(base: &Delay, controllers: &[SynthController], time_seconds: f64) -> Delay {
    let mut delay = base.clone();
    if let Some(value) = controller_value(controllers, DELAY_TIME_CC, time_seconds) {
        delay.time_seconds = 0.001 + value * 1.999;
        delay.beats_per_cycle = None;
    }
    if let Some(value) = controller_value(controllers, DELAY_FEEDBACK_CC, time_seconds) {
        delay.feedback = value * 0.95;
    }
    if let Some(value) = controller_value(controllers, DELAY_MIX_CC, time_seconds) {
        delay.mix = value;
    }
    delay
}

fn maximum_automated_delay_frames(
    sample_rate: u32,
    base: &Delay,
    controllers: &[SynthController],
    tempo_changes: &[SynthTempoChange],
) -> usize {
    let maximum_seconds = controllers
        .iter()
        .filter(|event| event.controller == DELAY_TIME_CC)
        .fold(base.time_seconds, |maximum, event| {
            maximum.max(0.001 + f32::from(event.value) / 127.0 * 1.999)
        });
    let automated_frames = ((maximum_seconds * sample_rate as f32).round() as usize).max(1);
    automated_frames.max(maximum_delay_frames(sample_rate, base, tempo_changes))
}

fn controller_value(
    controllers: &[SynthController],
    controller: u8,
    time_seconds: f64,
) -> Option<f32> {
    let index = controllers.partition_point(|event| event.time_seconds <= time_seconds);
    controllers[..index]
        .iter()
        .rev()
        .find(|event| event.controller == controller)
        .map(|event| f32::from(event.value) / 127.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_automation_overrides_beat_sync() {
        let delay = Delay {
            beats_per_cycle: Some(1.0),
            feedback: 0.0,
            mix: 1.0,
            ..Delay::default()
        };
        let controllers = [SynthController {
            time_seconds: 0.0,
            controller: DELAY_TIME_CC,
            value: 0,
        }];
        let tempo = [SynthTempoChange {
            time_seconds: 0.0,
            bpm: 60.0,
        }];
        let mut left = vec![1.0];
        let mut right = vec![0.0];
        apply_stereo_delay_automation_with_tempo(
            &mut left,
            &mut right,
            1_000,
            &delay,
            &controllers,
            &tempo,
        );
        assert_eq!(left[1], 1.0);
        assert_eq!(left[1_000], 0.0);
    }
}
