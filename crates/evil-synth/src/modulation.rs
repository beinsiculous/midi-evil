use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chorus {
    pub rate_hz: f32,
    pub depth_seconds: f32,
    pub delay_seconds: f32,
    pub feedback: f32,
    pub mix: f32,
}

impl Default for Chorus {
    fn default() -> Self {
        Self {
            rate_hz: 0.8,
            depth_seconds: 0.004,
            delay_seconds: 0.015,
            feedback: 0.2,
            mix: 0.25,
        }
    }
}

impl Chorus {
    pub fn validate(&self) -> Result<(), Error> {
        validate_modulation(
            self.rate_hz,
            self.depth_seconds,
            self.delay_seconds,
            self.feedback,
            self.mix,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flanger {
    pub rate_hz: f32,
    pub depth_seconds: f32,
    pub delay_seconds: f32,
    pub feedback: f32,
    pub mix: f32,
}

impl Default for Flanger {
    fn default() -> Self {
        Self {
            rate_hz: 0.5,
            depth_seconds: 0.002,
            delay_seconds: 0.003,
            feedback: 0.7,
            mix: 0.5,
        }
    }
}

impl Flanger {
    pub fn validate(&self) -> Result<(), Error> {
        validate_modulation(
            self.rate_hz,
            self.depth_seconds,
            self.delay_seconds,
            self.feedback,
            self.mix,
        )
    }
}

fn validate_modulation(
    rate_hz: f32,
    depth_seconds: f32,
    delay_seconds: f32,
    feedback: f32,
    mix: f32,
) -> Result<(), Error> {
    if !rate_hz.is_finite()
        || !(0.01..=20.0).contains(&rate_hz)
        || !depth_seconds.is_finite()
        || !(0.0..=0.03).contains(&depth_seconds)
        || !delay_seconds.is_finite()
        || !(0.001..=0.1).contains(&delay_seconds)
        || !feedback.is_finite()
        || !(0.0..=0.95).contains(&feedback)
        || !mix.is_finite()
        || !(0.0..=1.0).contains(&mix)
    {
        return Err(Error::InvalidPatch(
            "invalid chorus/flanger rate, depth, delay, feedback, or mix".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) struct ModulatedDelayProcessor {
    rate_hz: f32,
    depth_seconds: f32,
    delay_seconds: f32,
    feedback: f32,
    mix: f32,
    phase_offsets: [f32; 2],
    line: Vec<[f32; 2]>,
    position: usize,
    frame: u64,
}

impl ModulatedDelayProcessor {
    pub(crate) fn chorus(settings: &Chorus, sample_rate: u32) -> Self {
        Self::new(
            settings.rate_hz,
            settings.depth_seconds,
            settings.delay_seconds,
            settings.feedback,
            settings.mix,
            [0.0, std::f32::consts::FRAC_PI_2],
            sample_rate,
        )
    }

    pub(crate) fn flanger(settings: &Flanger, sample_rate: u32) -> Self {
        Self::new(
            settings.rate_hz,
            settings.depth_seconds,
            settings.delay_seconds,
            settings.feedback,
            settings.mix,
            [0.0; 2],
            sample_rate,
        )
    }

    fn new(
        rate_hz: f32,
        depth_seconds: f32,
        delay_seconds: f32,
        feedback: f32,
        mix: f32,
        phase_offsets: [f32; 2],
        sample_rate: u32,
    ) -> Self {
        Self {
            rate_hz,
            depth_seconds,
            delay_seconds,
            feedback,
            mix,
            phase_offsets,
            line: vec![[0.0; 2]; sample_rate as usize / 5 + 2],
            position: 0,
            frame: 0,
        }
    }

    pub(crate) fn process(&mut self, stereo: &mut [f32; 2], sample_rate: u32) {
        let dry = *stereo;
        let base_phase =
            self.frame as f32 * self.rate_hz * std::f32::consts::TAU / sample_rate as f32;
        let mut delayed = [0.0; 2];
        for (channel, output) in delayed.iter_mut().enumerate() {
            let phase = base_phase + self.phase_offsets[channel];
            let delay_seconds = self.delay_seconds + self.depth_seconds * (0.5 + 0.5 * phase.sin());
            let delay_frames = f64::from(delay_seconds * sample_rate as f32);
            let read = (self.position as f64 - delay_frames).rem_euclid(self.line.len() as f64);
            let index = read.floor() as usize;
            let next = (index + 1) % self.line.len();
            let fraction = read.fract() as f32;
            *output = self.line[index][channel]
                + (self.line[next][channel] - self.line[index][channel]) * fraction;
        }
        self.line[self.position] = [
            dry[0] + delayed[0] * self.feedback,
            dry[1] + delayed[1] * self.feedback,
        ];
        for (channel, sample) in stereo.iter_mut().enumerate() {
            *sample = dry[channel] + delayed[channel] * self.mix;
        }
        self.position = (self.position + 1) % self.line.len();
        self.frame = self.frame.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chorus_and_flanger_produce_delayed_finite_tails() {
        let mut chorus = ModulatedDelayProcessor::chorus(&Chorus::default(), 1_000);
        let mut flanger = ModulatedDelayProcessor::flanger(&Flanger::default(), 1_000);
        let mut chorus_output = Vec::new();
        let mut flanger_output = Vec::new();
        for frame in 0..128 {
            let input = if frame == 0 { 1.0 } else { 0.0 };
            let mut stereo = [input, input * 0.5];
            chorus.process(&mut stereo, 1_000);
            chorus_output.push(stereo);
            let mut stereo = [input, input * 0.5];
            flanger.process(&mut stereo, 1_000);
            flanger_output.push(stereo);
        }
        assert!(
            chorus_output[15..]
                .iter()
                .any(|frame| frame[0].abs() > 0.001)
        );
        assert!(
            flanger_output[3..]
                .iter()
                .any(|frame| frame[0].abs() > 0.001)
        );
        assert!(
            chorus_output
                .iter()
                .flatten()
                .all(|sample| sample.is_finite())
        );
        assert!(
            flanger_output
                .iter()
                .flatten()
                .all(|sample| sample.is_finite())
        );
        assert_ne!(chorus_output, flanger_output);
    }
}
