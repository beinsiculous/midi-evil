//! Sample-clocked pad playback and held-pad repeat, shared by audio frontends.
use std::sync::Arc;

use crate::{DrumHit, Error, KitPad, PAD_COUNT};

#[derive(Clone)]
pub struct PreparedPad {
    samples: Arc<[f32]>,
    sample_rate: u32,
    gain: f32,
    pan: f32,
    muted: bool,
}

impl PreparedPad {
    pub fn new(pad: &KitPad) -> Result<Self, Error> {
        pad.validate()?;
        let mut voice = pad.voice.clone();
        voice.gain = 1.0;
        let sound = voice.render(&[DrumHit {
            start_seconds: 0.0,
            duration_seconds: pad.gate_seconds,
            velocity: 127,
        }])?;
        Ok(Self {
            samples: sound.samples.into(),
            sample_rate: sound.sample_rate,
            gain: pad.voice.gain,
            pan: pad.pan,
            muted: pad.muted,
        })
    }
}

struct Voice {
    slot: usize,
    samples: Arc<[f32]>,
    position: f64,
    step: f64,
    velocity: f32,
}

pub struct DrumMachine {
    pads: [Option<PreparedPad>; PAD_COUNT],
    voices: Vec<Voice>,
    held: u16,
    velocity: u8,
    repeat: Option<u8>,
    tempo: f32,
    frame: u64,
    next_repeat: [f64; PAD_COUNT],
    gain: f32,
}

impl Default for DrumMachine {
    fn default() -> Self {
        Self {
            pads: std::array::from_fn(|_| None),
            voices: Vec::with_capacity(64),
            held: 0,
            velocity: 100,
            repeat: None,
            tempo: 120.0,
            frame: 0,
            next_repeat: [0.0; PAD_COUNT],
            gain: 0.8,
        }
    }
}

impl DrumMachine {
    pub const SAMPLE_RATE: u32 = 48_000;

    pub fn set_pad(&mut self, index: usize, pad: Option<PreparedPad>) {
        if index >= PAD_COUNT {
            return;
        }
        if pad.as_ref().is_none_or(|pad| pad.muted) {
            self.voices.retain(|voice| voice.slot != index);
        }
        self.pads[index] = pad;
    }

    pub fn set_gain(&mut self, gain: f32) {
        if gain.is_finite() {
            self.gain = gain.clamp(0.0, 1.0);
        }
    }

    pub fn input_velocity(&self) -> u8 {
        self.velocity
    }

    pub fn stop(&mut self) {
        self.held = 0;
        self.voices.clear();
    }

    /// A rising held bit fires immediately; held pads repeat on the audio clock.
    pub fn set_input(
        &mut self,
        held: u16,
        velocity: u8,
        repeat: Option<u8>,
        tempo: f32,
    ) -> Result<(), Error> {
        if velocity == 0
            || velocity > 127
            || !tempo.is_finite()
            || tempo <= 0.0
            || repeat.is_some_and(|rate| ![1, 2, 4, 8, 16, 32].contains(&rate))
        {
            return Err(Error::InvalidKit("invalid pad performance controls".into()));
        }
        let changed = repeat != self.repeat || tempo != self.tempo;
        self.velocity = velocity;
        self.repeat = repeat;
        self.tempo = tempo;
        for slot in 0..PAD_COUNT {
            let bit = 1 << slot;
            let rising = held & bit != 0 && self.held & bit == 0;
            if rising {
                self.trigger(slot);
            }
            if rising || changed {
                self.next_repeat[slot] = self.frame as f64 + self.period();
            }
        }
        self.held = held;
        Ok(())
    }

    fn period(&self) -> f64 {
        (f64::from(Self::SAMPLE_RATE) * 240.0
            / f64::from(self.tempo)
            / f64::from(self.repeat.unwrap_or(1)))
        .max(1.0)
    }

    fn trigger(&mut self, slot: usize) {
        let Some(pad) = &self.pads[slot] else {
            return;
        };
        if pad.muted {
            return;
        }
        if self.voices.len() == 64 {
            self.voices.remove(0);
        }
        self.voices.push(Voice {
            slot,
            samples: pad.samples.clone(),
            position: 0.0,
            step: f64::from(pad.sample_rate) / f64::from(Self::SAMPLE_RATE),
            velocity: f32::from(self.velocity) / 127.0,
        });
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        self.next_frame_with_repeats().0
    }

    /// Return the audio frame and a bitmask of hits fired by held-pad repeat.
    /// Direct rising-edge hits are excluded so frontends do not record them twice.
    pub fn next_frame_with_repeats(&mut self) -> ([f32; 2], u16) {
        let mut repeated = 0_u16;
        if self.repeat.is_some() {
            for slot in 0..PAD_COUNT {
                if self.held & (1 << slot) != 0 && self.frame as f64 >= self.next_repeat[slot] {
                    self.trigger(slot);
                    repeated |= 1 << slot;
                    self.next_repeat[slot] += self.period();
                }
            }
        }
        let mut output = [0.0; 2];
        self.voices
            .retain(|voice| voice.position < voice.samples.len() as f64);
        for voice in &mut self.voices {
            let i = voice.position as usize;
            let next = (i + 1).min(voice.samples.len() - 1);
            let sample = voice.samples[i]
                + (voice.samples[next] - voice.samples[i]) * voice.position.fract() as f32;
            if let Some(pad) = &self.pads[voice.slot] {
                let angle = (pad.pan + 1.0) * std::f32::consts::FRAC_PI_4;
                let sample = sample * pad.gain * voice.velocity * self.gain;
                output[0] += sample * angle.cos();
                output[1] += sample * angle.sin();
            }
            voice.position += voice.step;
        }
        self.frame = self.frame.wrapping_add(1);
        (output.map(f32::tanh), repeated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DrumPad;

    fn machine() -> DrumMachine {
        let mut machine = DrumMachine::default();
        let mut pad = KitPad::new(DrumPad::new(vec![1.0], 48_000));
        pad.voice.envelope.release_seconds = 0.0;
        pad.pan = -1.0;
        machine.set_pad(0, Some(PreparedPad::new(&pad).unwrap()));
        machine
    }

    #[test]
    fn repeat_uses_sample_clock_and_release_stops_new_hits() {
        let mut machine = machine();
        machine.set_input(1, 127, Some(16), 120.0).unwrap();
        let onsets: Vec<_> = (0..12001)
            .filter(|_| machine.next_frame()[0] > 0.0)
            .collect();
        assert_eq!(onsets, vec![0, 6000, 12000]);
        machine.set_input(0, 127, Some(16), 120.0).unwrap();
        assert!((0..12000).all(|_| machine.next_frame() == [0.0; 2]));
    }

    #[test]
    fn repeat_frames_report_only_automatic_retrigger_bits() {
        let mut machine = machine();
        machine.set_input(1, 127, Some(16), 120.0).unwrap();
        let repeated: Vec<_> = (0..12_001)
            .filter(|_| machine.next_frame_with_repeats().1 == 1)
            .collect();
        assert_eq!(repeated, [6_000, 12_000]);
    }

    #[test]
    fn input_updates_do_not_retrigger_and_panic_clears_every_voice() {
        let mut machine = machine();
        machine.set_input(1, 127, None, 120.0).unwrap();
        let first = machine.next_frame();
        assert!(first[0] > 0.0 && first[1].abs() < 1e-6);
        machine.set_input(1, 100, None, 130.0).unwrap();
        assert_eq!(machine.next_frame(), [0.0; 2]);
        machine.stop();
        assert!((0..100).all(|_| machine.next_frame() == [0.0; 2]));
    }

    #[test]
    fn gated_pad_resampling_pan_and_mute_reach_the_live_output() {
        let mut pad = KitPad::new(DrumPad::new(vec![0.5; 8_000], 8_000));
        pad.gate_seconds = Some(0.01);
        pad.voice.envelope.release_seconds = 0.01;
        pad.pan = 1.0;
        let mut machine = DrumMachine::default();
        machine.set_pad(0, Some(PreparedPad::new(&pad).unwrap()));
        machine.set_input(1, 127, None, 120.0).unwrap();
        for _ in 0..480 {
            let frame = machine.next_frame();
            assert!(frame[0].abs() < 1e-6 && frame[1] > 0.1);
        }
        for _ in 0..500 {
            machine.next_frame();
        }
        assert_eq!(machine.next_frame(), [0.0; 2]);
        machine.set_input(0, 127, None, 120.0).unwrap();
        machine.set_input(1, 127, Some(32), 120.0).unwrap();
        assert!(machine.next_frame()[1] > 0.0);
        pad.muted = true;
        machine.set_pad(0, Some(PreparedPad::new(&pad).unwrap()));
        assert!((0..6_000).all(|_| machine.next_frame() == [0.0; 2]));
    }
}
