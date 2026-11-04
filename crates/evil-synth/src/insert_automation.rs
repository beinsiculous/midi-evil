use crate::{Delay, SynthController, Waveshaper};

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
) {
    let maximum_delay = maximum_delay_frames(sample_rate, base, controllers);
    let original_len = samples.len();
    samples.resize(original_len + maximum_delay * 8, 0.0);
    let mut delay_line = vec![0.0; samples.len() + maximum_delay];
    for frame in 0..samples.len() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay = automated_delay(base, controllers, time);
        let dry = samples[frame];
        let delayed = delay_line[frame];
        samples[frame] = dry + delayed * delay.mix;
        delay_line[frame + delay_frames(sample_rate, &delay)] = dry + delayed * delay.feedback;
    }
}

pub(crate) fn apply_stereo_delay_automation(
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
    sample_rate: u32,
    base: &Delay,
    controllers: &[SynthController],
) {
    debug_assert_eq!(left.len(), right.len());
    let maximum_delay = maximum_delay_frames(sample_rate, base, controllers);
    let original_len = left.len();
    left.resize(original_len + maximum_delay * 8, 0.0);
    right.resize(original_len + maximum_delay * 8, 0.0);
    let mut left_line = vec![0.0; left.len() + maximum_delay];
    let mut right_line = vec![0.0; right.len() + maximum_delay];
    for frame in 0..left.len() {
        let time = frame as f64 / f64::from(sample_rate);
        let delay = automated_delay(base, controllers, time);
        let dry_left = left[frame];
        let dry_right = right[frame];
        let delayed_left = left_line[frame];
        let delayed_right = right_line[frame];
        left[frame] = dry_left + delayed_left * delay.mix;
        right[frame] = dry_right + delayed_right * delay.mix;
        let write = frame + delay_frames(sample_rate, &delay);
        if base.mode == crate::DelayMode::StereoCross {
            left_line[write] = dry_right + delayed_right * delay.feedback;
            right_line[write] = dry_left + delayed_left * delay.feedback;
        } else {
            left_line[write] = dry_left + delayed_left * delay.feedback;
            right_line[write] = dry_right + delayed_right * delay.feedback;
        }
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
    }
    if let Some(value) = controller_value(controllers, DELAY_FEEDBACK_CC, time_seconds) {
        delay.feedback = value * 0.95;
    }
    if let Some(value) = controller_value(controllers, DELAY_MIX_CC, time_seconds) {
        delay.mix = value;
    }
    delay
}

fn maximum_delay_frames(sample_rate: u32, base: &Delay, controllers: &[SynthController]) -> usize {
    let maximum_seconds = controllers
        .iter()
        .filter(|event| event.controller == DELAY_TIME_CC)
        .fold(base.time_seconds, |maximum, event| {
            maximum.max(0.001 + f32::from(event.value) / 127.0 * 1.999)
        });
    ((maximum_seconds * sample_rate as f32).round() as usize).max(1)
}

fn delay_frames(sample_rate: u32, delay: &Delay) -> usize {
    ((delay.time_seconds * sample_rate as f32).round() as usize).max(1)
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
