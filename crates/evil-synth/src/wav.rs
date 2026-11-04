use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::{Error, RenderedSound, StereoSound, sample::MAX_SAMPLE_SECONDS};

/// Read finite mono/stereo PCM or float WAV audio and downmix it to mono.
/// Instrument sources share the 8–192 kHz and 30-second bounds.
pub fn read_mono_wav(path: impl AsRef<Path>) -> Result<RenderedSound, Error> {
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
    let samples = downmix_interleaved(samples, spec.channels);
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::InvalidWav(
            "WAV samples must be nonempty and finite".to_owned(),
        ));
    }
    Ok(RenderedSound {
        samples,
        sample_rate: spec.sample_rate,
    })
}

pub(crate) fn downmix_interleaved(samples: Vec<f32>, channels: u16) -> Vec<f32> {
    samples
        .chunks(usize::from(channels))
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

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
