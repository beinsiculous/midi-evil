use std::{ops::Range, path::Path};

use serde::{Deserialize, Serialize};

use crate::{Error, read_mono_wav};

pub const MAX_SAMPLE_SECONDS: u32 = 30;

/// One embedded sample assigned to an inclusive MIDI keyboard range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleZone {
    pub low_key: u8,
    pub high_key: u8,
    pub sample: SampleOscillator,
}

impl SampleZone {
    pub fn new(low_key: u8, high_key: u8, sample: SampleOscillator) -> Self {
        Self {
            low_key,
            high_key,
            sample,
        }
    }

    pub fn contains(&self, key: u8) -> bool {
        (self.low_key..=self.high_key).contains(&key)
    }
}

pub(crate) fn validate_sample_zones(zones: &[SampleZone]) -> Result<(), Error> {
    for zone in zones {
        if zone.low_key > zone.high_key || zone.high_key > 127 {
            return Err(Error::InvalidPatch(
                "sample zones need inclusive MIDI ranges with 0 <= low_key <= high_key <= 127"
                    .to_owned(),
            ));
        }
        zone.sample.validate()?;
    }
    let mut ordered = zones.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|zone| (zone.low_key, zone.high_key));
    if ordered
        .windows(2)
        .any(|pair| pair[0].high_key >= pair[1].low_key)
    {
        return Err(Error::InvalidPatch(
            "sample zones must not overlap".to_owned(),
        ));
    }
    Ok(())
}

/// Embedded mono audio for one sample oscillator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SampleOscillator {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Source fundamental corresponding to the oscillator's base frequency.
    pub root_frequency_hz: f32,
    pub loop_enabled: bool,
    /// First frame in the loop region. Playback still starts at frame zero.
    pub loop_start_frame: usize,
    /// Exclusive loop end; `None` preserves the default full-source loop.
    pub loop_end_frame: Option<usize>,
}

impl Default for SampleOscillator {
    fn default() -> Self {
        Self {
            name: "Sample".to_owned(),
            samples: Vec::new(),
            sample_rate: 48_000,
            root_frequency_hz: 440.0,
            loop_enabled: false,
            loop_start_frame: 0,
            loop_end_frame: None,
        }
    }
}

impl SampleOscillator {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
            ..Self::default()
        }
    }

    /// Import mono/stereo PCM or float WAV, downmixing stereo to mono.
    pub fn read_wav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let sound = read_mono_wav(path)?;
        let mut sample = Self::new(sound.samples, sound.sample_rate);
        sample.name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        sample.validate()?;
        Ok(sample)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !(8_000..=192_000).contains(&self.sample_rate)
            || self.samples.is_empty()
            || self.samples.len() > self.sample_rate as usize * MAX_SAMPLE_SECONDS as usize
            || self.samples.iter().any(|sample| !sample.is_finite())
            || !self.root_frequency_hz.is_finite()
            || self.root_frequency_hz <= 0.0
            || self.root_frequency_hz > self.sample_rate as f32 / 2.0
        {
            return Err(Error::InvalidPatch(
                "sample audio must be finite, nonempty, at most 30 seconds, and have a valid root frequency"
                    .to_owned(),
            ));
        }
        let loop_range = self.loop_range();
        if loop_range.start >= loop_range.end || loop_range.end > self.samples.len() {
            return Err(Error::InvalidPatch(
                "sample loop points must satisfy 0 <= start < end <= source frame count".to_owned(),
            ));
        }
        Ok(())
    }

    /// The validated loop region, with an exclusive end frame.
    pub fn loop_range(&self) -> Range<usize> {
        self.loop_start_frame..self.loop_end_frame.unwrap_or(self.samples.len())
    }

    pub(crate) fn advance(&self, position: f64, frames: f64) -> f64 {
        let position = position + frames;
        if self.loop_enabled {
            self.loop_position(position)
        } else {
            position.min(self.samples.len() as f64)
        }
    }

    pub(crate) fn at(&self, position: f64) -> f32 {
        let len = self.samples.len();
        if len == 0 || position < 0.0 {
            return 0.0;
        }
        let position = if self.loop_enabled {
            self.loop_position(position)
        } else {
            position
        };
        if position < 0.0 || position >= len as f64 {
            return 0.0;
        }
        let index = position.floor() as usize;
        let loop_range = self.loop_range();
        let next = if self.loop_enabled && index + 1 == loop_range.end {
            loop_range.start
        } else if index + 1 < len {
            index + 1
        } else if self.loop_enabled && loop_range.end == len {
            loop_range.start
        } else {
            index
        };
        let fraction = position.fract() as f32;
        self.samples[index] + (self.samples[next] - self.samples[index]) * fraction
    }

    fn loop_position(&self, position: f64) -> f64 {
        let loop_range = self.loop_range();
        if loop_range.start == 0 && loop_range.end == self.samples.len() {
            position.rem_euclid(self.samples.len() as f64)
        } else if position >= loop_range.end as f64 && loop_range.start < loop_range.end {
            loop_range.start as f64
                + (position - loop_range.end as f64)
                    .rem_euclid((loop_range.end - loop_range.start) as f64)
        } else {
            position
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_import_downmixes_stereo_and_embeds_the_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for value in [16_384_i16, 0, -16_384, 0] {
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();

        let sample = SampleOscillator::read_wav(&path).unwrap();
        assert_eq!(sample.name, "source");
        assert_eq!(sample.sample_rate, 48_000);
        assert_eq!(sample.samples, vec![0.25, -0.25]);
        assert!(!sample.loop_enabled);
        assert_eq!(sample.loop_range(), 0..2);
    }

    #[test]
    fn interpolation_supports_loop_boundaries_and_one_shot_ends() {
        let mut sample = SampleOscillator::new(vec![0.0, 1.0, -1.0], 48_000);
        assert_eq!(sample.at(0.5), 0.5);
        assert_eq!(sample.at(1.5), 0.0);
        assert_eq!(sample.at(2.5), -1.0);
        assert_eq!(sample.at(3.0), 0.0);
        sample.loop_enabled = true;
        assert_eq!(sample.at(2.5), -0.5);
        assert_eq!(sample.at(3.5), 0.5);

        let explicit_full_source = SampleOscillator {
            loop_end_frame: Some(3),
            ..sample.clone()
        };
        for position in [0.0, 0.5, 2.5, 3.0, 3.5, 19.25] {
            assert_eq!(sample.at(position), explicit_full_source.at(position));
            assert_eq!(
                sample.advance(position, 0.75),
                explicit_full_source.advance(position, 0.75)
            );
        }
    }

    #[test]
    fn custom_loop_points_wrap_and_interpolate_without_replaying_the_intro() {
        let mut sample = SampleOscillator {
            loop_enabled: true,
            loop_start_frame: 1,
            loop_end_frame: Some(3),
            ..SampleOscillator::new(vec![0.25, 1.0, -1.0, 0.75], 48_000)
        };
        assert_eq!(sample.at(0.5), 0.625);
        assert_eq!(sample.at(1.5), 0.0);
        assert_eq!(sample.at(2.5), 0.0);
        assert_eq!(sample.at(3.0), 1.0);
        assert_eq!(sample.at(3.5), 0.0);
        assert_eq!(sample.at(8.5), 0.0);
        assert_eq!(sample.advance(2.5, 0.75), 1.25);
        assert_eq!(sample.advance(2.5, 8.75), 1.25);

        sample.loop_start_frame = 2;
        assert_eq!(sample.at(2.25), -1.0);
        assert_eq!(sample.at(3.25), -1.0);
        assert_eq!(sample.advance(2.5, 0.5), 2.0);
    }

    #[test]
    fn disabled_looping_ignores_retained_custom_points() {
        let one_shot = SampleOscillator::new(vec![0.25, 1.0, -1.0, 0.75], 48_000);
        let retained_points = SampleOscillator {
            loop_start_frame: 1,
            loop_end_frame: Some(3),
            ..one_shot.clone()
        };
        for position in [0.0, 0.5, 1.5, 2.5, 3.5, 8.5] {
            assert_eq!(one_shot.at(position), retained_points.at(position));
            assert_eq!(
                one_shot.advance(position, 0.75),
                retained_points.advance(position, 0.75)
            );
        }
    }
}
