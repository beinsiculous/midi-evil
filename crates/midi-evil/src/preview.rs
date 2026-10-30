use std::num::NonZero;
use std::time::Duration;

use evil_synth::StereoSound;
use rodio::{DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, Player, Source};

pub struct PreviewPlayer {
    _stream: MixerDeviceSink,
    player: Player,
}

impl PreviewPlayer {
    pub fn play(sound: StereoSound) -> Result<Self, DeviceSinkError> {
        Self::play_with_loop(sound, false)
    }

    pub fn play_repeating(sound: StereoSound) -> Result<Self, DeviceSinkError> {
        Self::play_with_loop(sound, true)
    }

    fn play_with_loop(sound: StereoSound, looping: bool) -> Result<Self, DeviceSinkError> {
        let mut stream = DeviceSinkBuilder::open_default_sink()?;
        stream.log_on_drop(false);
        let player = Player::connect_new(stream.mixer());
        player.append(StereoSource::new_with_loop(sound, looping));
        Ok(Self {
            _stream: stream,
            player,
        })
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }

    pub fn position(&self) -> Duration {
        self.player.get_pos()
    }
}

struct StereoSource {
    left: Vec<f32>,
    right: Vec<f32>,
    sample_rate: u32,
    sample_index: usize,
    looping: bool,
}

impl StereoSource {
    fn new_with_loop(sound: StereoSound, looping: bool) -> Self {
        debug_assert_eq!(sound.left.len(), sound.right.len());
        Self {
            left: sound.left,
            right: sound.right,
            sample_rate: sound.sample_rate,
            sample_index: 0,
            looping,
        }
    }
}

impl Iterator for StereoSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.sample_index / 2;
        let sample = if self.sample_index.is_multiple_of(2) {
            self.left.get(frame)
        } else {
            self.right.get(frame)
        };
        if sample.is_some() {
            self.sample_index += 1;
        } else if self.looping && !self.left.is_empty() {
            self.sample_index = 0;
            return self.next();
        }
        sample.copied()
    }
}

impl Source for StereoSource {
    fn current_span_len(&self) -> Option<usize> {
        if self.looping {
            None
        } else if self.sample_index >= self.left.len() + self.right.len() {
            Some(0)
        } else {
            None
        }
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).expect("stereo has two channels")
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(self.sample_rate).expect("validated sample rates are nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        if self.looping {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.left.len() as f64 / f64::from(self.sample_rate),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_source_interleaves_channels_and_reports_duration() {
        let mut source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.1, 0.2],
                right: vec![-0.1, -0.2],
                sample_rate: 48_000,
            },
            false,
        );
        assert_eq!(source.channels().get(), 2);
        assert_eq!(source.sample_rate().get(), 48_000);
        assert_eq!(source.next(), Some(0.1));
        assert_eq!(source.next(), Some(-0.1));
        assert_eq!(source.next(), Some(0.2));
        assert_eq!(source.next(), Some(-0.2));
        assert_eq!(source.next(), None);
        assert!(source.is_exhausted());
        let source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.0; 48_000],
                right: vec![0.0; 48_000],
                sample_rate: 48_000,
            },
            false,
        );
        assert_eq!(source.total_duration(), Some(Duration::from_secs(1)));
    }

    #[test]
    fn looping_source_restarts_at_the_section_boundary() {
        let source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.1, 0.2],
                right: vec![-0.1, -0.2],
                sample_rate: 48_000,
            },
            true,
        );
        let samples: Vec<_> = source.take(8).collect();
        assert_eq!(samples, [0.1, -0.1, 0.2, -0.2, 0.1, -0.1, 0.2, -0.2]);
    }
}
