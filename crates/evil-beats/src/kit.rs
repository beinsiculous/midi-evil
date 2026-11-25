use std::{io::Read, path::Path};

use serde::{Deserialize, Serialize};

use crate::{BUS_COUNT, DrumBus, DrumPad, Error};

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
    #[serde(default)]
    pub buses: [DrumBus; BUS_COUNT],
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
    /// Post-pad-gain, pre-pan sends into the three output buses.
    #[serde(default)]
    pub bus_sends: [f32; BUS_COUNT],
}

impl KitPad {
    pub fn new(voice: DrumPad) -> Self {
        Self {
            voice,
            pan: 0.0,
            muted: false,
            gate_seconds: None,
            choke_group: 0,
            bus_sends: [0.0; BUS_COUNT],
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
        if self
            .bus_sends
            .iter()
            .any(|send| !send.is_finite() || !(0.0..=1.0).contains(send))
        {
            return Err(Error::InvalidKit("pad bus sends must be 0..=1".into()));
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
            buses: [DrumBus::default(); BUS_COUNT],
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
        for bus in self.buses {
            bus.validate()?;
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
    /// Import WAV or uncompressed AIFF/AIFC audio based on file extension, preserving stereo.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let audio =
            evil_synth::read_audio(path).map_err(|error| Error::InvalidPad(error.to_string()))?;
        Self::from_decoded(path, audio)
    }

    /// Import PCM/float mono or stereo WAV, preserving stereo channels.
    pub fn read_wav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let audio =
            evil_synth::read_wav(path).map_err(|error| Error::InvalidPad(error.to_string()))?;
        Self::from_decoded(path, audio)
    }

    /// Import mono or stereo uncompressed AIFF/AIFC, preserving stereo channels.
    pub fn read_aiff(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let audio =
            evil_synth::read_aiff(path).map_err(|error| Error::InvalidPad(error.to_string()))?;
        Self::from_decoded(path, audio)
    }

    fn from_decoded(path: &Path, audio: evil_synth::DecodedAudio) -> Result<Self, Error> {
        let mut pad = match audio.right_samples {
            Some(right) => Self::new_stereo(audio.samples, right, audio.sample_rate),
            None => Self::new(audio.samples, audio.sample_rate),
        };
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
        pad.bus_sends = [0.25, 0.5, 0.75];
        pad.voice.filter = Some(crate::DrumFilter {
            mode: evil_synth::FilterMode::HighPass,
            cutoff_hz: 2_000.0,
            resonance: 0.3,
            stages: 4,
        });
        kit.buses[0] = DrumBus {
            filter: Some(crate::DrumFilter::default()),
            gain: 0.7,
            pan: 0.4,
            muted: false,
        };
        kit.pads[15] = Some(pad);
        assert_eq!(DrumKit::from_json(&kit.to_json().unwrap()).unwrap(), kit);

        let mut legacy = serde_json::to_value(&kit).unwrap();
        legacy.as_object_mut().unwrap().remove("buses");
        let legacy_pad = legacy["pads"][15].as_object_mut().unwrap();
        legacy_pad.remove("choke_group");
        legacy_pad.remove("gate_seconds");
        legacy_pad.remove("bus_sends");
        let voice = legacy_pad["voice"].as_object_mut().unwrap();
        for field in [
            "playback_start_frame",
            "playback_end_frame",
            "direction",
            "loop_enabled",
            "filter",
        ] {
            voice.remove(field);
        }
        let migrated = DrumKit::from_json(&legacy.to_string()).unwrap();
        let voice = &migrated.pads[15].as_ref().unwrap().voice;
        assert_eq!(voice.playback_range(), 0..3);
        assert_eq!(voice.direction, crate::SampleDirection::Forward);
        assert!(!voice.loop_enabled);
        assert_eq!(voice.filter, None);
        let legacy_pad = migrated.pads[15].as_ref().unwrap();
        assert_eq!(legacy_pad.choke_group, 0);
        assert_eq!(legacy_pad.bus_sends, [0.0; BUS_COUNT]);
        assert_eq!(migrated.buses, [DrumBus::default(); BUS_COUNT]);

        kit.pads[15].as_mut().unwrap().bus_sends[0] = 1.1;
        assert!(kit.validate().is_err());
        kit.pads[15].as_mut().unwrap().bus_sends[0] = 0.25;
        kit.buses[0].gain = -0.1;
        assert!(kit.validate().is_err());
        kit.buses[0].gain = 0.7;
        kit.pads[15]
            .as_mut()
            .unwrap()
            .voice
            .filter
            .as_mut()
            .unwrap()
            .cutoff_hz = 0.0;
        assert!(kit.validate().is_err());
        kit.pads[15]
            .as_mut()
            .unwrap()
            .voice
            .filter
            .as_mut()
            .unwrap()
            .cutoff_hz = 2_000.0;

        kit.pads[15].as_mut().unwrap().choke_group = 3;
        assert!(kit.validate().is_err());
        kit.pads[15].as_mut().unwrap().choke_group = 2;

        kit.version = 99;
        assert!(kit.to_json().is_err());
    }

    #[test]
    fn imports_pcm_stereo_and_aiff_and_rejects_nonfinite_float_audio() {
        let path = std::env::temp_dir().join(format!("evil-beats-wav-{}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in [16384_i16, 0, -16384, 8192] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        let stereo_pad = DrumPad::read_wav(&path).unwrap();
        assert_eq!(stereo_pad.samples, vec![0.5, -0.5]);
        assert_eq!(stereo_pad.right_samples, Some(vec![0.0, 0.25]));
        assert!(stereo_pad.is_stereo());

        let mut kit = DrumKit::empty("Stereo");
        kit.pads[0] = Some(KitPad::new(stereo_pad.clone()));
        let kit_json = kit.to_json().unwrap();
        assert_eq!(DrumKit::from_json(&kit_json).unwrap(), kit);
        let mono_kit = {
            let mut mono = DrumKit::empty("Mono");
            mono.pads[0] = Some(KitPad::new(DrumPad::new(vec![0.25], 48_000)));
            mono
        };
        assert!(!mono_kit.to_json().unwrap().contains("right_samples"));

        let aiff_path =
            std::env::temp_dir().join(format!("evil-beats-aiff-{}.aiff", std::process::id()));
        let mut pcm = Vec::new();
        for value in [16_384_i16, -8_192, -16_384, 8_192] {
            pcm.extend(value.to_be_bytes());
        }
        let mut comm = Vec::new();
        comm.extend(2_u16.to_be_bytes());
        comm.extend(2_u32.to_be_bytes());
        comm.extend(16_u16.to_be_bytes());
        let mut rate = [0_u8; 10];
        let exp = 31 - 48_000_u32.leading_zeros();
        let sig = u64::from(48_000_u32) << (63 - exp);
        rate[0..2].copy_from_slice(&(16_383 + exp as u16).to_be_bytes());
        rate[2..10].copy_from_slice(&sig.to_be_bytes());
        comm.extend(rate);
        let mut ssnd = vec![0_u8; 8];
        ssnd.extend(pcm);
        let mut chunks = Vec::new();
        for (id, data) in [(b"COMM", &comm), (b"SSND", &ssnd)] {
            chunks.extend(id);
            chunks.extend((data.len() as u32).to_be_bytes());
            chunks.extend(data);
        }
        let mut aiff = Vec::new();
        aiff.extend(b"FORM");
        aiff.extend((chunks.len() as u32 + 4).to_be_bytes());
        aiff.extend(b"AIFF");
        aiff.extend(chunks);
        std::fs::write(&aiff_path, aiff).unwrap();
        let aiff_pad = DrumPad::read(&aiff_path).unwrap();
        assert_eq!(aiff_pad.samples, vec![0.5, -0.5]);
        assert_eq!(aiff_pad.right_samples, Some(vec![-0.25, 0.25]));
        assert_eq!(DrumPad::read_aiff(&aiff_path).unwrap(), aiff_pad);
        std::fs::remove_file(aiff_path).unwrap();

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
