use std::sync::Arc;

use evil_synth::FilterProcessor;

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
    filter: Option<[FilterProcessor; 2]>,
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
            filter: pad
                .filter
                .map(|filter| [filter.processor(), filter.processor()]),
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
        let frame = self.next_frame()?;
        Some(if self.pad.is_stereo() {
            (frame[0] + frame[1]) * 0.5
        } else {
            frame[0]
        })
    }

    pub(crate) fn next_frame(&mut self) -> Option<[f32; 2]> {
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
        let raw = self.frame_at_position().unwrap_or([0.0, 0.0]);
        let filtered = match self.filter.as_mut() {
            Some([left, right]) => {
                let left_out = left.process(raw[0], self.output_sample_rate);
                let right_out = if self.pad.is_stereo() {
                    right.process(raw[1], self.output_sample_rate)
                } else {
                    left_out
                };
                [left_out, right_out]
            }
            None => raw,
        };
        let amplitude = self
            .pad
            .envelope
            .amplitude(time_seconds as f32, self.gate_seconds as f32);
        let gain = amplitude * self.velocity;
        self.frame = self.frame.wrapping_add(1);
        self.advance();
        Some([filtered[0] * gain, filtered[1] * gain])
    }

    fn frame_at_position(&self) -> Option<[f32; 2]> {
        let (index, next, fraction) = self.interpolation_points()?;
        let left =
            self.pad.samples[index] + (self.pad.samples[next] - self.pad.samples[index]) * fraction;
        let right = match &self.pad.right_samples {
            Some(right) => right[index] + (right[next] - right[index]) * fraction,
            None => left,
        };
        Some([left, right])
    }

    fn interpolation_points(&self) -> Option<(usize, usize, f32)> {
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
        Some((index, next, self.position.fract() as f32))
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
