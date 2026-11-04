use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp_sample::ToSample;
use evil_synth::SampleOscillator;

const MAX_SAMPLE_SECONDS: usize = 30;

struct RecordingBuffer {
    interleaved: Vec<f32>,
    channels: u16,
    sample_rate: u32,
}

impl RecordingBuffer {
    fn new(channels: u16, sample_rate: u32) -> Result<Self, String> {
        if !(1..=2).contains(&channels) || !(8_000..=192_000).contains(&sample_rate) {
            return Err("microphone input must be mono/stereo at 8–192 kHz".to_owned());
        }
        Ok(Self {
            interleaved: Vec::new(),
            channels,
            sample_rate,
        })
    }

    fn push_interleaved(&mut self, samples: &[f32]) {
        let remaining = self.sample_rate as usize * MAX_SAMPLE_SECONDS * self.channels as usize
            - self.interleaved.len();
        self.interleaved
            .extend_from_slice(&samples[..samples.len().min(remaining)]);
        let frame_length = self.interleaved.len() / self.channels as usize * self.channels as usize;
        self.interleaved.truncate(frame_length);
    }

    fn finish(self) -> Result<SampleOscillator, String> {
        if self.interleaved.is_empty() {
            return Err("no microphone audio was captured".to_owned());
        }
        let samples = self
            .interleaved
            .chunks(self.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect();
        let mut sample = SampleOscillator::new(samples, self.sample_rate);
        sample.name = "Microphone".to_owned();
        sample.validate().map_err(|error| error.to_string())?;
        Ok(sample)
    }
}

pub struct SampleRecorder {
    stream: cpal::Stream,
    buffer: Arc<Mutex<RecordingBuffer>>,
    error: Arc<Mutex<Option<String>>>,
}

impl SampleRecorder {
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "no microphone input device is available".to_owned())?;
        let config = device
            .default_input_config()
            .map_err(|error| format!("could not query microphone input: {error}"))?;
        let buffer = Arc::new(Mutex::new(RecordingBuffer::new(
            config.channels(),
            config.sample_rate(),
        )?));
        let error = Arc::new(Mutex::new(None));
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, &buffer, &error),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, &buffer, &error),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, &buffer, &error),
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, &buffer, &error),
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, &buffer, &error),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, &buffer, &error),
            cpal::SampleFormat::I64 => build_stream::<i64>(&device, &config, &buffer, &error),
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config, &buffer, &error),
            cpal::SampleFormat::U64 => build_stream::<u64>(&device, &config, &buffer, &error),
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config, &buffer, &error),
            format => return Err(format!("unsupported microphone sample format {format}")),
        }?;
        stream
            .play()
            .map_err(|error| format!("could not start microphone input: {error}"))?;
        Ok(Self {
            stream,
            buffer,
            error,
        })
    }

    pub fn finish(self) -> Result<SampleOscillator, String> {
        let Self {
            stream,
            buffer,
            error,
        } = self;
        drop(stream);
        if let Some(error) = error.lock().ok().and_then(|mut error| error.take()) {
            return Err(error);
        }
        Arc::try_unwrap(buffer)
            .map_err(|_| "microphone buffer is still in use".to_owned())?
            .into_inner()
            .map_err(|_| "microphone buffer was poisoned".to_owned())?
            .finish()
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buffer: &Arc<Mutex<RecordingBuffer>>,
    error: &Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + ToSample<f32>,
{
    let buffer = Arc::clone(buffer);
    let stream_error = Arc::clone(error);
    let stream_config: cpal::StreamConfig = (*config).into();
    device
        .build_input_stream(
            stream_config,
            move |data: &[T], _| {
                let samples = data
                    .iter()
                    .map(|sample| sample.to_sample())
                    .collect::<Vec<_>>();
                if let Ok(mut buffer) = buffer.lock() {
                    buffer.push_interleaved(&samples);
                }
            },
            move |stream_error_value| {
                if let Ok(mut error) = stream_error.lock() {
                    *error = Some(format!("microphone stream failed: {stream_error_value}"));
                }
            },
            None,
        )
        .map_err(|error| format!("could not open microphone input: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_buffer_downmixes_stereo_and_enforces_the_limit() {
        let mut buffer = RecordingBuffer::new(2, 8_000).unwrap();
        buffer.push_interleaved(&[0.5, 0.25, -0.5, 0.25]);
        let sample = buffer.finish().unwrap();
        assert_eq!(sample.name, "Microphone");
        assert_eq!(sample.samples, vec![0.375, -0.125]);
        assert_eq!(sample.sample_rate, 8_000);

        let mut buffer = RecordingBuffer::new(1, 8_000).unwrap();
        buffer.push_interleaved(&vec![0.25; 8_000 * 30 + 100]);
        assert_eq!(buffer.interleaved.len(), 8_000 * 30);
        assert!(buffer.finish().is_ok());
    }

    #[test]
    fn recording_buffer_rejects_empty_or_invalid_audio() {
        assert!(RecordingBuffer::new(3, 48_000).is_err());
        assert!(RecordingBuffer::new(1, 4_000).is_err());
        assert!(RecordingBuffer::new(1, 48_000).unwrap().finish().is_err());
    }
}
