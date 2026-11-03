use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, read_mono_wav};

pub const MAX_SAMPLE_SECONDS: u32 = 30;

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
}

impl Default for SampleOscillator {
    fn default() -> Self {
        Self {
            name: "Sample".to_owned(),
            samples: Vec::new(),
            sample_rate: 48_000,
            root_frequency_hz: 440.0,
            loop_enabled: false,
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
        Ok(())
    }

    pub(crate) fn at(&self, position: f64) -> f32 {
        let len = self.samples.len();
        if len == 0 {
            return 0.0;
        }
        let position = if self.loop_enabled {
            position.rem_euclid(len as f64)
        } else {
            position
        };
        if position < 0.0 || position >= len as f64 {
            return 0.0;
        }
        let index = position.floor() as usize;
        let next = if index + 1 < len {
            index + 1
        } else if self.loop_enabled {
            0
        } else {
            index
        };
        let fraction = position.fract() as f32;
        self.samples[index] + (self.samples[next] - self.samples[index]) * fraction
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
    }
}
