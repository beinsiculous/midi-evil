use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reverb {
    pub room_size: f32,
    pub damping: f32,
    pub mix: f32,
}

impl Default for Reverb {
    fn default() -> Self {
        Self {
            room_size: 0.7,
            damping: 0.3,
            mix: 0.25,
        }
    }
}

impl Reverb {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.room_size.is_finite()
            || !(0.0..=1.0).contains(&self.room_size)
            || !self.damping.is_finite()
            || !(0.0..=1.0).contains(&self.damping)
            || !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
        {
            return Err(Error::InvalidPatch(
                "reverb room size, damping, and mix must be 0..=1".to_owned(),
            ));
        }
        Ok(())
    }
}

struct Comb {
    line: Vec<f32>,
    position: usize,
    feedback: f32,
    damping_alpha: f32,
    damped: f32,
}

impl Comb {
    fn new(delay: usize, feedback: f32, damping_alpha: f32) -> Self {
        Self {
            line: vec![0.0; delay.max(1)],
            position: 0,
            feedback,
            damping_alpha,
            damped: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line[self.position];
        self.damped += self.damping_alpha * (delayed - self.damped);
        self.line[self.position] = input + self.damped * self.feedback;
        self.position = (self.position + 1) % self.line.len();
        delayed
    }
}

struct AllPass {
    line: Vec<f32>,
    position: usize,
}

impl AllPass {
    fn new(delay: usize) -> Self {
        Self {
            line: vec![0.0; delay.max(1)],
            position: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line[self.position];
        let output = delayed - input;
        self.line[self.position] = input + delayed * 0.5;
        self.position = (self.position + 1) % self.line.len();
        output
    }
}

pub(crate) struct ReverbProcessor {
    mix: f32,
    combs: [[Comb; 4]; 2],
    all_pass: [[AllPass; 2]; 2],
}

impl ReverbProcessor {
    pub(crate) fn new(settings: &Reverb, sample_rate: u32) -> Self {
        let feedback = 0.2 + settings.room_size * 0.75;
        let damping_alpha = 0.01 + (1.0 - settings.damping) * 0.99;
        let scale = sample_rate as f32 / 48_000.0;
        Self {
            mix: settings.mix,
            combs: std::array::from_fn(|channel| {
                std::array::from_fn(|index| {
                    let base = [1_499, 1_601, 1_747, 1_931][index];
                    let offset = if channel == 0 { 0 } else { 23 };
                    Comb::new(
                        ((base + offset) as f32 * scale).round() as usize,
                        feedback,
                        damping_alpha,
                    )
                })
            }),
            all_pass: std::array::from_fn(|channel| {
                std::array::from_fn(|index| {
                    let base = [337, 521][index];
                    AllPass::new(((base + channel * 17) as f32 * scale).round() as usize)
                })
            }),
        }
    }

    pub(crate) fn process(&mut self, stereo: &mut [f32; 2]) {
        let dry = *stereo;
        let inputs = [dry[0] + dry[1] * 0.2, dry[1] + dry[0] * 0.2];
        for channel in 0..2 {
            let mut wet = 0.0;
            for comb in &mut self.combs[channel] {
                wet += comb.process(inputs[channel]);
            }
            wet *= 0.25;
            for all_pass in &mut self.all_pass[channel] {
                wet = all_pass.process(wet);
            }
            stereo[channel] = dry[channel] + wet * self.mix;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_diffuses_an_impulse_into_a_finite_tail() {
        let mut processor = ReverbProcessor::new(&Reverb::default(), 1_000);
        let mut output = Vec::new();
        for frame in 0..128 {
            let mut stereo = [if frame == 0 { 1.0 } else { 0.0 }, 0.0];
            processor.process(&mut stereo);
            output.push(stereo);
        }
        assert!(output[31..].iter().any(|frame| frame[0].abs() > 0.001));
        assert!(output[31..].iter().any(|frame| frame[1].abs() > 0.001));
        assert!(output.iter().flatten().all(|sample| sample.is_finite()));
    }
}
