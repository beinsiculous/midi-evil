use std::sync::Arc;

use crate::{DrumPad, SampleDirection};

/// The shared offline/live polyphony limit. The oldest voice is stolen first.
pub const VOICE_LIMIT: usize = 32;

pub(crate) struct PlaybackVoice {
    pub(crate) slot: usize,
    pub(crate) choke_group: u8,
    pad: Arc<DrumPad>,
    output_sample_rate: u32,
    position: f64,
    step: f64,
    frame: u64,
    gate_seconds: f64,
    velocity: f32,
    finished: bool,
}

impl PlaybackVoice {
    pub(crate) fn new(
        pad: Arc<DrumPad>,
        slot: usize,
        choke_group: u8,
        output_sample_rate: u32,
        velocity: u8,
        gate_seconds: Option<f64>,
    ) -> Self {
        let playback = pad.playback_range();
        let tuning_rate = 2.0_f64.powf(f64::from(pad.tuning_semitones) / 12.0);
        let source_seconds =
            (playback.end - playback.start) as f64 / f64::from(pad.sample_rate) / tuning_rate;
        let gate_seconds = gate_seconds.unwrap_or(if pad.loop_enabled {
            f64::INFINITY
        } else {
            source_seconds
        });
        let position = match pad.direction {
            SampleDirection::Forward => playback.start as f64,
            SampleDirection::Reverse => (playback.end - 1) as f64,
        };
        let direction = match pad.direction {
            SampleDirection::Forward => 1.0,
            SampleDirection::Reverse => -1.0,
        };
        Self {
            slot,
            choke_group,
            step: direction * tuning_rate * f64::from(pad.sample_rate)
                / f64::from(output_sample_rate),
            pad,
            output_sample_rate,
            position,
            frame: 0,
            gate_seconds,
            velocity: f32::from(velocity) / 127.0,
            finished: false,
        }
    }

    /// Total gate plus release, or `None` for an ungated live loop.
    pub(crate) fn duration_seconds(&self) -> Option<f64> {
        (self.gate_seconds.is_finite())
            .then_some(self.gate_seconds + f64::from(self.pad.envelope.release_seconds))
    }

    pub(crate) fn is_finished(&self) -> bool {
        if self.finished {
            return true;
        }
        let time_seconds = self.frame as f64 / f64::from(self.output_sample_rate);
        self.duration_seconds()
            .is_some_and(|duration| time_seconds >= duration)
    }

    pub(crate) fn next_sample(&mut self) -> Option<f32> {
        if self.finished {
            return None;
        }
        let time_seconds = self.frame as f64 / f64::from(self.output_sample_rate);
        if self
            .duration_seconds()
            .is_some_and(|duration| time_seconds >= duration)
        {
            self.finished = true;
            return None;
        }
        let sample = self.sample_at_position().unwrap_or(0.0);
        let amplitude = self
            .pad
            .envelope
            .amplitude(time_seconds as f32, self.gate_seconds as f32);
        self.frame = self.frame.wrapping_add(1);
        self.advance();
        Some(sample * amplitude * self.velocity)
    }

    fn sample_at_position(&self) -> Option<f32> {
        let playback = self.pad.playback_range();
        if self.position < playback.start as f64 || self.position >= playback.end as f64 {
            return None;
        }
        let index = self.position.floor() as usize;
        let next = if index + 1 >= playback.end {
            if self.pad.loop_enabled {
                playback.start
            } else {
                index
            }
        } else {
            index + 1
        };
        let fraction = self.position.fract() as f32;
        Some(
            self.pad.samples[index] + (self.pad.samples[next] - self.pad.samples[index]) * fraction,
        )
    }

    fn advance(&mut self) {
        self.position += self.step;
        if self.pad.loop_enabled {
            let playback = self.pad.playback_range();
            let start = playback.start as f64;
            let length = (playback.end - playback.start) as f64;
            if self.position < start || self.position >= playback.end as f64 {
                self.position = start + (self.position - start).rem_euclid(length);
            }
        }
    }
}

/// Apply choke-group exclusion and deterministic oldest-first voice stealing.
pub(crate) fn allocate_voice(voices: &mut Vec<PlaybackVoice>, voice: PlaybackVoice) {
    if voice.choke_group != 0 {
        voices.retain(|active| active.choke_group != voice.choke_group);
    }
    if voices.len() == VOICE_LIMIT {
        voices.remove(0);
    }
    voices.push(voice);
}
