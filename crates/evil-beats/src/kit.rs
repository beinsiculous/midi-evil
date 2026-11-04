use std::{io::Read, path::Path};

use serde::{Deserialize, Serialize};

use crate::{DrumPad, Error};

pub const PAD_COUNT: usize = 16;
const MAX_BANK_BYTES: u64 = 128 * 1024 * 1024;

/// A portable bank: sample audio is embedded, so moving the file keeps its sounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumKit {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub gain: f32,
    pub pads: [Option<KitPad>; PAD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitPad {
    pub voice: DrumPad,
    pub pan: f32,
    pub muted: bool,
    #[serde(default)]
    pub gate_seconds: Option<f64>,
    /// Zero is independent; pads sharing group 1 or 2 silence each other.
    #[serde(default)]
    pub choke_group: u8,
}

impl KitPad {
    pub fn new(voice: DrumPad) -> Self {
        Self {
            voice,
            pan: 0.0,
            muted: false,
            gate_seconds: None,
            choke_group: 0,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.voice.validate()?;
        if self
            .gate_seconds
            .is_some_and(|gate| !gate.is_finite() || gate <= 0.0 || gate > 30.0)
        {
            return Err(Error::InvalidKit(
                "pad gate must be between 0 and 30 seconds".into(),
            ));
        }
        if !self.pan.is_finite() || !(-1.0..=1.0).contains(&self.pan) {
            return Err(Error::InvalidKit("pad pan must be between -1 and 1".into()));
        }
        if self.choke_group > 2 {
            return Err(Error::InvalidKit(
                "pad choke group must be 0 (off), 1, or 2".into(),
            ));
        }
        Ok(())
    }
}

impl DrumKit {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            format: "midi-evil-drumkit".into(),
            version: 1,
            name: name.into(),
            gain: 0.8,
            pads: std::array::from_fn(|_| None),
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.format != "midi-evil-drumkit" || self.version != 1 {
            return Err(Error::InvalidKit("unsupported format or version".into()));
        }
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err(Error::InvalidKit(
                "bank gain must be between 0 and 1".into(),
            ));
        }
        for pad in self.pads.iter().flatten() {
            pad.validate()?;
        }
        Ok(())
    }

    pub fn from_json(json: &str) -> Result<Self, Error> {
        let kit: Self = serde_json::from_str(json)?;
        kit.validate()?;
        Ok(kit)
    }

    pub fn to_json(&self) -> Result<String, Error> {
        self.validate()?;
        let json = serde_json::to_string(self)?;
        if json.len() as u64 > MAX_BANK_BYTES {
            return Err(Error::InvalidKit(
                "bank exceeds 128 MiB; use shorter pad samples".into(),
            ));
        }
        Ok(json)
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file = std::fs::File::open(path).map_err(|e| Error::InvalidKit(e.to_string()))?;
        let mut json = String::new();
        file.take(MAX_BANK_BYTES + 1)
            .read_to_string(&mut json)
            .map_err(|e| Error::InvalidKit(e.to_string()))?;
        if json.len() as u64 > MAX_BANK_BYTES {
            return Err(Error::InvalidKit("bank exceeds 128 MiB".into()));
        }
        Self::from_json(&json)
    }
}

impl DrumPad {
    /// Import PCM/float mono or stereo WAV, downmixing stereo to a mono pad.
    pub fn read_wav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let sound = evil_synth::read_mono_wav(path)
            .map_err(|error| Error::InvalidPad(error.to_string()))?;
        let mut pad = Self::new(sound.samples, sound.sample_rate);
        pad.name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        pad.validate()?;
        Ok(pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_round_trip_keeps_empty_slots_audio_and_mixer_settings() {
        let mut kit = DrumKit::empty("Test");
        let mut pad = KitPad::new(DrumPad::new(vec![0.0, 0.5, -0.5], 48_000));
        pad.pan = -0.6;
        pad.voice.tuning_semitones = 7.0;
        pad.voice.playback_start_frame = 1;
        pad.voice.playback_end_frame = Some(3);
        pad.voice.direction = crate::SampleDirection::Reverse;
        pad.voice.loop_enabled = true;
        pad.gate_seconds = Some(0.25);
        pad.choke_group = 2;
        kit.pads[15] = Some(pad);
        assert_eq!(DrumKit::from_json(&kit.to_json().unwrap()).unwrap(), kit);

        let mut legacy = serde_json::to_value(&kit).unwrap();
        let legacy_pad = legacy["pads"][15].as_object_mut().unwrap();
        legacy_pad.remove("choke_group");
        legacy_pad.remove("gate_seconds");
        let voice = legacy_pad["voice"].as_object_mut().unwrap();
        for field in [
            "playback_start_frame",
            "playback_end_frame",
            "direction",
            "loop_enabled",
        ] {
            voice.remove(field);
        }
        let migrated = DrumKit::from_json(&legacy.to_string()).unwrap();
        let voice = &migrated.pads[15].as_ref().unwrap().voice;
        assert_eq!(voice.playback_range(), 0..3);
        assert_eq!(voice.direction, crate::SampleDirection::Forward);
        assert!(!voice.loop_enabled);
        assert_eq!(migrated.pads[15].as_ref().unwrap().choke_group, 0);

        kit.pads[15].as_mut().unwrap().choke_group = 3;
        assert!(kit.validate().is_err());
        kit.pads[15].as_mut().unwrap().choke_group = 2;

        kit.version = 99;
        assert!(kit.to_json().is_err());
    }

    #[test]
    fn imports_pcm_stereo_and_rejects_nonfinite_float_audio() {
        let path = std::env::temp_dir().join(format!("evil-beats-wav-{}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in [16384_i16, 0, -16384, 0] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        assert_eq!(DrumPad::read_wav(&path).unwrap().samples, vec![0.25, -0.25]);
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
                ..spec
            },
        )
        .unwrap();
        writer.write_sample(f32::NAN).unwrap();
        writer.finalize().unwrap();
        assert!(DrumPad::read_wav(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
