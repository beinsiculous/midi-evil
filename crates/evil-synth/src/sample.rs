use std::{ops::Range, path::Path};

use serde::{Deserialize, Serialize};

use crate::{Error, RenderedSound, StereoSound, read_mono_aiff, read_mono_wav};

pub const MAX_SAMPLE_SECONDS: u32 = 30;

fn is_zero(value: &usize) -> bool {
    *value == 0
}

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
    /// First source frame played by this sample or keyboard slice.
    #[serde(skip_serializing_if = "is_zero")]
    pub playback_start_frame: usize,
    /// Exclusive playback end; `None` preserves the default full-source range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_end_frame: Option<usize>,
    pub loop_enabled: bool,
    /// First frame in the loop region, within the playback range.
    pub loop_start_frame: usize,
    /// Exclusive loop end; `None` preserves the default playback-range loop.
    pub loop_end_frame: Option<usize>,
}

impl Default for SampleOscillator {
    fn default() -> Self {
        Self {
            name: "Sample".to_owned(),
            samples: Vec::new(),
            sample_rate: 48_000,
            root_frequency_hz: 440.0,
            playback_start_frame: 0,
            playback_end_frame: None,
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

    /// Downmix a rendered stereo section into a portable embedded sample.
    pub fn from_stereo(name: impl Into<String>, sound: &StereoSound) -> Result<Self, Error> {
        if sound.left.len() != sound.right.len() {
            return Err(Error::InvalidMix(
                "resampled audio channels must have the same frame count".to_owned(),
            ));
        }
        let samples = sound
            .left
            .iter()
            .zip(&sound.right)
            .map(|(left, right)| (left + right) * 0.5)
            .collect();
        let mut sample = Self::new(samples, sound.sample_rate);
        sample.name = name.into();
        sample.validate()?;
        Ok(sample)
    }

    /// Import a WAV or AIFF/AIFC source based on its file extension.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if path
            .extension()
            .is_some_and(|extension| matches!(extension.to_str(), Some("aif" | "aiff" | "aifc")))
        {
            Self::read_aiff(path)
        } else {
            Self::read_wav(path)
        }
    }

    /// Import mono/stereo PCM or float WAV, downmixing stereo to mono.
    pub fn read_wav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        Self::from_rendered(path, read_mono_wav(path)?)
    }

    /// Import mono/stereo uncompressed AIFF/AIFC, downmixing stereo to mono.
    pub fn read_aiff(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        Self::from_rendered(path, read_mono_aiff(path)?)
    }

    fn from_rendered(path: &Path, sound: RenderedSound) -> Result<Self, Error> {
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
        let playback_range = self.playback_range();
        if playback_range.start >= playback_range.end || playback_range.end > self.samples.len() {
            return Err(Error::InvalidPatch(
                "sample playback points must satisfy 0 <= start < end <= source frame count"
                    .to_owned(),
            ));
        }
        let loop_range = self.loop_range();
        if loop_range.start >= loop_range.end
            || loop_range.start < playback_range.start
            || loop_range.end > playback_range.end
        {
            return Err(Error::InvalidPatch(
                "sample loop points must be nonempty and lie within the playback range".to_owned(),
            ));
        }
        Ok(())
    }

    /// The validated playback region, with an exclusive end frame.
    pub fn playback_range(&self) -> Range<usize> {
        self.playback_start_frame..self.playback_end_frame.unwrap_or(self.samples.len())
    }

    /// The validated loop region, with an exclusive end frame.
    pub fn loop_range(&self) -> Range<usize> {
        self.loop_start_frame..self.loop_end_frame.unwrap_or(self.playback_range().end)
    }

    /// Divide the current playback range into contiguous, non-destructive keyboard slices.
    ///
    /// Keys and frames are distributed evenly; earlier slices receive one extra item when
    /// either total has a remainder. Loop-enabled slices loop their own complete range.
    /// Every zone retains the original source frames, root frequency, and playback settings.
    pub fn keyboard_slices(
        &self,
        low_key: u8,
        high_key: u8,
        slice_count: usize,
    ) -> Result<Vec<SampleZone>, Error> {
        self.validate()?;
        if low_key > high_key || high_key > 127 {
            return Err(Error::InvalidPatch(
                "keyboard slicing needs an inclusive MIDI range with 0 <= low <= high <= 127"
                    .to_owned(),
            ));
        }
        let key_count = usize::from(high_key - low_key) + 1;
        if slice_count == 0 || slice_count > key_count {
            return Err(Error::InvalidPatch(
                "slice count must be between 1 and the number of selected keys".to_owned(),
            ));
        }
        if slice_count == 1 {
            return Ok(vec![SampleZone::new(low_key, high_key, self.clone())]);
        }
        let playback = self.playback_range();
        let frame_count = playback.end - playback.start;
        if slice_count > frame_count {
            return Err(Error::InvalidPatch(
                "slice count cannot exceed the playback frame count".to_owned(),
            ));
        }

        Ok((0..slice_count)
            .map(|index| {
                let first_key = usize::from(low_key) + split_offset(index, key_count, slice_count);
                let next_key =
                    usize::from(low_key) + split_offset(index + 1, key_count, slice_count);
                let start = playback.start + split_offset(index, frame_count, slice_count);
                let end = playback.start + split_offset(index + 1, frame_count, slice_count);
                let mut sample = self.clone();
                sample.playback_start_frame = start;
                sample.playback_end_frame = Some(end);
                sample.loop_start_frame = start;
                sample.loop_end_frame = Some(end);
                SampleZone::new(first_key as u8, (next_key - 1) as u8, sample)
            })
            .collect())
    }

    pub(crate) fn advance(&self, position: f64, frames: f64) -> f64 {
        let position = position + frames;
        if self.loop_enabled {
            self.loop_position(position)
        } else {
            position.min(self.playback_range().end as f64)
        }
    }

    pub(crate) fn at(&self, position: f64) -> f32 {
        let len = self.samples.len();
        if len == 0 || position < 0.0 {
            return 0.0;
        }
        let playback_range = self.playback_range();
        let position = if self.loop_enabled {
            self.loop_position(position)
        } else {
            position
        };
        if position < playback_range.start as f64 || position >= playback_range.end as f64 {
            return 0.0;
        }
        let index = position.floor() as usize;
        let loop_range = self.loop_range();
        let next = if self.loop_enabled && index + 1 == loop_range.end {
            loop_range.start
        } else if index + 1 < playback_range.end {
            index + 1
        } else if self.loop_enabled && loop_range.end == playback_range.end {
            loop_range.start
        } else {
            index
        };
        let fraction = position.fract() as f32;
        self.samples[index] + (self.samples[next] - self.samples[index]) * fraction
    }

    fn loop_position(&self, position: f64) -> f64 {
        let loop_range = self.loop_range();
        if position >= loop_range.end as f64 && loop_range.start < loop_range.end {
            loop_range.start as f64
                + (position - loop_range.end as f64)
                    .rem_euclid((loop_range.end - loop_range.start) as f64)
        } else {
            position
        }
    }
}

fn split_offset(index: usize, total: usize, parts: usize) -> usize {
    index * (total / parts) + index.min(total % parts)
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
    fn stereo_resampling_downmixes_and_validates_embedded_audio() {
        let sound = crate::StereoSound {
            left: vec![0.5, -0.25],
            right: vec![0.25, 0.75],
            sample_rate: 48_000,
        };
        let sample = SampleOscillator::from_stereo("Section", &sound).unwrap();
        assert_eq!(sample.name, "Section");
        assert_eq!(sample.samples, vec![0.375, 0.25]);
        assert_eq!(sample.sample_rate, 48_000);
        let mismatched = crate::StereoSound {
            right: Vec::new(),
            ..sound
        };
        assert!(SampleOscillator::from_stereo("Invalid", &mismatched).is_err());
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

    #[test]
    fn playback_ranges_interpolate_loop_and_end_without_cropping_source_audio() {
        let source = (0..8).map(|value| value as f32 / 10.0).collect::<Vec<_>>();
        let mut sample = SampleOscillator {
            playback_start_frame: 2,
            playback_end_frame: Some(6),
            loop_start_frame: 2,
            ..SampleOscillator::new(source.clone(), 8_000)
        };
        assert_eq!(sample.at(1.75), 0.0);
        assert_eq!(sample.at(2.5), 0.25);
        assert_eq!(sample.at(5.5), 0.5);
        assert_eq!(sample.at(6.0), 0.0);
        assert_eq!(sample.advance(5.5, 0.75), 6.0);

        sample.loop_enabled = true;
        assert!((sample.at(5.5) - 0.35).abs() < 0.000_001);
        assert_eq!(sample.at(6.5), 0.25);
        assert_eq!(sample.advance(5.5, 0.75), 2.25);

        sample.loop_start_frame = 3;
        sample.loop_end_frame = Some(5);
        assert_eq!(sample.at(2.5), 0.25);
        assert!((sample.at(4.5) - 0.35).abs() < 0.000_001);
        assert!((sample.at(5.5) - 0.35).abs() < 0.000_001);
        assert_eq!(sample.advance(4.5, 1.25), 3.75);
        assert_eq!(sample.samples, source);
    }

    #[test]
    fn keyboard_slicing_distributes_remainders_and_preserves_the_source() {
        let original = SampleOscillator {
            root_frequency_hz: 220.0,
            loop_enabled: true,
            ..SampleOscillator::new((0..10).map(|value| value as f32 / 20.0).collect(), 8_000)
        };
        let unchanged = original.clone();
        let zones = original.keyboard_slices(20, 24, 3).unwrap();
        assert_eq!(original, unchanged);
        assert_eq!(
            zones
                .iter()
                .map(|zone| (zone.low_key, zone.high_key))
                .collect::<Vec<_>>(),
            vec![(20, 21), (22, 23), (24, 24)]
        );
        assert_eq!(
            zones
                .iter()
                .map(|zone| zone.sample.playback_range())
                .collect::<Vec<_>>(),
            vec![0..4, 4..7, 7..10]
        );
        for zone in &zones {
            assert_eq!(zone.sample.samples, original.samples);
            assert_eq!(zone.sample.root_frequency_hz, 220.0);
            assert!(zone.sample.loop_enabled);
            assert_eq!(zone.sample.loop_range(), zone.sample.playback_range());
            zone.sample.validate().unwrap();
        }
        assert_eq!(
            original.keyboard_slices(20, 24, 1).unwrap(),
            vec![SampleZone::new(20, 24, original.clone())]
        );
    }

    #[test]
    fn keyboard_slicing_rejects_invalid_ranges_counts_and_empty_slices() {
        let sample = SampleOscillator::new(vec![0.0; 10], 8_000);
        for (low_key, high_key, count) in
            [(5, 4, 1), (0, 128, 1), (0, 1, 0), (0, 1, 3), (0, 127, 11)]
        {
            assert!(
                sample.keyboard_slices(low_key, high_key, count).is_err(),
                "accepted invalid slicing request {low_key}..={high_key}, {count} slices"
            );
        }
        assert!(
            SampleOscillator::default()
                .keyboard_slices(0, 127, 1)
                .is_err()
        );
    }
}
