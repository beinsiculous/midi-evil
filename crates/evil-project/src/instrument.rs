use serde::{Deserialize, Serialize};

use crate::{Error, invalid};

pub const DEFAULT_DRUM_BASE_NOTE: u8 = 36;
pub const DRUM_PAD_COUNT: u8 = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Instrument {
    pub name: String,
    pub source: InstrumentSource,
    /// General MIDI program to preserve on export, overriding the patch default.
    pub midi_program: Option<u8>,
    pub gain: f32,
    pub pan: f32,
    pub pitch_bend_range_semitones: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstrumentSource {
    Synth {
        patch: String,
    },
    DrumKit {
        kit: String,
        #[serde(default = "default_drum_base_note")]
        base_note: u8,
    },
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            name: "Instrument".to_owned(),
            source: InstrumentSource::Synth {
                patch: "lead".to_owned(),
            },
            midi_program: None,
            gain: 0.8,
            pan: 0.0,
            pitch_bend_range_semitones: 2.0,
        }
    }
}

impl Instrument {
    pub fn synth(name: impl Into<String>, patch: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: InstrumentSource::Synth {
                patch: patch.into(),
            },
            ..Self::default()
        }
    }

    pub fn drum_kit(name: impl Into<String>, kit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: InstrumentSource::DrumKit {
                kit: kit.into(),
                base_note: DEFAULT_DRUM_BASE_NOTE,
            },
            ..Self::default()
        }
    }

    pub fn patch(&self) -> Option<&str> {
        match &self.source {
            InstrumentSource::Synth { patch } => Some(patch),
            InstrumentSource::DrumKit { .. } => None,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty() {
            return Err(invalid("instrument name cannot be empty"));
        }
        match &self.source {
            InstrumentSource::Synth { patch } if patch.trim().is_empty() => {
                return Err(invalid("instrument patch cannot be empty"));
            }
            InstrumentSource::DrumKit { kit, .. } if kit.trim().is_empty() => {
                return Err(invalid("drum-kit path cannot be empty"));
            }
            InstrumentSource::DrumKit { base_note, .. }
                if *base_note > 127 - DRUM_PAD_COUNT + 1 =>
            {
                return Err(invalid("drum base note leaves fewer than 16 MIDI pads"));
            }
            _ => {}
        }
        if self.midi_program.is_some_and(|program| program > 127) {
            return Err(invalid("instrument midi_program must be between 0 and 127"));
        }
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err(invalid("instrument gain must be between 0 and 1"));
        }
        if !self.pan.is_finite() || !(-1.0..=1.0).contains(&self.pan) {
            return Err(invalid("instrument pan must be between -1 and 1"));
        }
        if !self.pitch_bend_range_semitones.is_finite()
            || self.pitch_bend_range_semitones <= 0.0
            || self.pitch_bend_range_semitones > 24.0
        {
            return Err(invalid(
                "pitch bend range must be between 0 and 24 semitones",
            ));
        }
        Ok(())
    }
}

const fn default_drum_base_note() -> u8 {
    DEFAULT_DRUM_BASE_NOTE
}
