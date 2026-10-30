use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::{Error, RenderedSound, StereoSound};

impl RenderedSound {
    pub fn write_wav(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_planar_wav(path, self.sample_rate, &[&self.samples])
    }
}

impl StereoSound {
    pub fn write_wav(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_planar_wav(path, self.sample_rate, &[&self.left, &self.right])
    }
}

fn write_planar_wav(
    path: impl AsRef<Path>,
    sample_rate: u32,
    channels: &[&[f32]],
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
    let mut writer = WavWriter::create(path, spec)?;
    for frame in 0..frame_count {
        for channel in channels {
            let sample = (channel[frame].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(sample)?;
        }
    }
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
}
