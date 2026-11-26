use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::{Error, RenderCancellation, RenderedSound, StereoSound, sample::MAX_SAMPLE_SECONDS};

/// Decoded mono or stereo audio preserving discrete left/right channels when present.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub right_samples: Option<Vec<f32>>,
    pub sample_rate: u32,
}

impl DecodedAudio {
    pub fn is_stereo(&self) -> bool {
        self.right_samples.is_some()
    }

    pub fn into_mono(self) -> RenderedSound {
        let samples = match self.right_samples {
            Some(right) => self
                .samples
                .into_iter()
                .zip(right)
                .map(|(left, right)| (left + right) * 0.5)
                .collect(),
            None => self.samples,
        };
        RenderedSound {
            samples,
            sample_rate: self.sample_rate,
        }
    }
}

/// Read finite mono/stereo PCM or float WAV audio, preserving stereo channels.
/// Instrument sources share the 8–192 kHz and 30-second bounds.
pub fn read_wav(path: impl AsRef<Path>) -> Result<DecodedAudio, Error> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if !(1..=2).contains(&spec.channels) || !(8_000..=192_000).contains(&spec.sample_rate) {
        return Err(Error::InvalidWav(
            "use a mono/stereo WAV at 8–192 kHz".to_owned(),
        ));
    }
    if reader.duration() > spec.sample_rate * MAX_SAMPLE_SECONDS {
        return Err(Error::InvalidWav(
            "WAV instrument sources can be at most 30 seconds long".to_owned(),
        ));
    }
    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        SampleFormat::Int => {
            let scale = 2.0_f32.powi(i32::from(spec.bits_per_sample) - 1);
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|sample| sample as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    deinterleave_audio(samples, spec.channels, spec.sample_rate)
        .map_err(|message| Error::InvalidWav(format!("WAV {message}")))
}

/// Read finite mono/stereo PCM or float WAV audio and downmix it to mono.
/// Instrument sources share the 8–192 kHz and 30-second bounds.
pub fn read_mono_wav(path: impl AsRef<Path>) -> Result<RenderedSound, Error> {
    read_wav(path).map(DecodedAudio::into_mono)
}

pub(crate) fn deinterleave_audio(
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
) -> Result<DecodedAudio, &'static str> {
    if samples.is_empty() {
        return Err("samples must be nonempty and finite");
    }
    match channels {
        1 => {
            if samples.iter().any(|sample| !sample.is_finite()) {
                return Err("samples must be nonempty and finite");
            }
            Ok(DecodedAudio {
                samples,
                right_samples: None,
                sample_rate,
            })
        }
        2 => {
            if !samples.len().is_multiple_of(2) {
                return Err("stereo audio must contain complete left/right frames");
            }
            let frame_count = samples.len() / 2;
            let mut left = Vec::with_capacity(frame_count);
            let mut right = Vec::with_capacity(frame_count);
            for frame in samples.chunks_exact(2) {
                if !frame[0].is_finite() || !frame[1].is_finite() {
                    return Err("samples must be nonempty and finite");
                }
                left.push(frame[0]);
                right.push(frame[1]);
            }
            Ok(DecodedAudio {
                samples: left,
                right_samples: Some(right),
                sample_rate,
            })
        }
        _ => Err("audio must be mono or stereo"),
    }
}

impl RenderedSound {
    pub fn write_wav(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_planar_wav(path, self.sample_rate, &[&self.samples])
    }

    pub fn write_wav_with_cancellation(
        &self,
        path: impl AsRef<Path>,
        cancellation: &RenderCancellation,
    ) -> Result<(), Error> {
        write_planar_wav_with_cancellation(path, self.sample_rate, &[&self.samples], cancellation)
    }
}

impl StereoSound {
    pub fn write_wav(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_planar_wav(path, self.sample_rate, &[&self.left, &self.right])
    }

    pub fn write_wav_with_cancellation(
        &self,
        path: impl AsRef<Path>,
        cancellation: &RenderCancellation,
    ) -> Result<(), Error> {
        write_planar_wav_with_cancellation(
            path,
            self.sample_rate,
            &[&self.left, &self.right],
            cancellation,
        )
    }
}

fn write_planar_wav(
    path: impl AsRef<Path>,
    sample_rate: u32,
    channels: &[&[f32]],
) -> Result<(), Error> {
    write_planar_wav_with_cancellation(path, sample_rate, channels, &RenderCancellation::default())
}

fn write_planar_wav_with_cancellation(
    path: impl AsRef<Path>,
    sample_rate: u32,
    channels: &[&[f32]],
    cancellation: &RenderCancellation,
) -> Result<(), Error> {
    if channels.is_empty() || channels.len() > 2 {
        return Err(Error::InvalidMix(
            "WAV output supports one or two channels".to_owned(),
        ));
    }
    let frame_count = channels[0].len();
    if channels.iter().any(|channel| channel.len() != frame_count) {
        return Err(Error::InvalidMix(
            "audio channels must have the same frame count".to_owned(),
        ));
    }
    let spec = WavSpec {
        channels: channels.len() as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    cancellation.check()?;
    let mut writer = WavWriter::create(path, spec)?;
    for frame in 0..frame_count {
        cancellation.check_frame(frame)?;
        for channel in channels {
            let sample = (channel[frame].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(sample)?;
        }
    }
    cancellation.check()?;
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Patch, StereoSound, render};

    #[test]
    fn wav_output_is_readable_16_bit_mono_audio() {
        let sound = render(&Patch::default()).unwrap();
        let path = tempfile::tempdir().unwrap().keep().join("tone.wav");
        sound.write_wav(&path).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.samples::<i16>().count(), sound.samples.len());
    }

    #[test]
    fn stereo_wav_interleaves_left_and_right_samples() {
        let sound = StereoSound {
            left: vec![0.5, -0.5],
            right: vec![-0.25, 0.25],
            sample_rate: 48_000,
        };
        let path = tempfile::tempdir().unwrap().keep().join("stereo.wav");
        sound.write_wav(&path).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples.len(), 4);
        assert!(samples[0] > 0 && samples[1] < 0);
        assert!(samples[2] < 0 && samples[3] > 0);
    }

    #[test]
    fn read_wav_preserves_stereo_and_keeps_mono_single_channel() {
        let directory = tempfile::tempdir().unwrap();
        let stereo_path = directory.path().join("stereo.wav");
        let mut writer = hound::WavWriter::create(
            &stereo_path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for sample in [16_384_i16, -8_192, -16_384, 8_192] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        let decoded = super::read_wav(&stereo_path).unwrap();
        assert_eq!(decoded.samples, vec![0.5, -0.5]);
        assert_eq!(decoded.right_samples, Some(vec![-0.25, 0.25]));
        assert!(decoded.is_stereo());
        assert_eq!(
            super::read_mono_wav(&stereo_path).unwrap().samples,
            vec![0.125, -0.125]
        );

        let mono_path = directory.path().join("mono.wav");
        let mut writer = hound::WavWriter::create(
            &mono_path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        writer.write_sample(16_384_i16).unwrap();
        writer.finalize().unwrap();
        let mono = super::read_wav(&mono_path).unwrap();
        assert_eq!(mono.samples, vec![0.5]);
        assert_eq!(mono.right_samples, None);
        assert!(!mono.is_stereo());
    }
}
