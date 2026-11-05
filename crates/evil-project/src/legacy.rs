use serde::{Deserialize, Serialize};

use crate::{
    Aftertouch, ControllerEvent, FORMAT_NAME, FORMAT_VERSION, Instrument, InstrumentSource, Note,
    Part, PitchBend, Project, TempoChange, Track,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProjectV1 {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub tempo_bpm: f32,
    pub tempo_changes: Vec<TempoChange>,
    pub beats_per_bar: u32,
    pub beat_unit: u32,
    pub ticks_per_quarter: u16,
    pub sample_rate: u32,
    pub master_gain: f32,
    pub source_midi: Option<String>,
    pub tracks: Vec<TrackV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TrackV1 {
    pub name: String,
    pub patch: String,
    pub midi_program: Option<u8>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub notes: Vec<Note>,
    pub pitch_bends: Vec<PitchBend>,
    pub aftertouch: Vec<Aftertouch>,
    pub controllers: Vec<ControllerEvent>,
    pub pitch_bend_range_semitones: f32,
}

impl Default for ProjectV1 {
    fn default() -> Self {
        let current = Project::default();
        Self {
            format: FORMAT_NAME.to_owned(),
            version: 1,
            name: current.name,
            tempo_bpm: current.tempo_bpm,
            tempo_changes: Vec::new(),
            beats_per_bar: current.beats_per_bar,
            beat_unit: current.beat_unit,
            ticks_per_quarter: current.ticks_per_quarter,
            sample_rate: current.sample_rate,
            master_gain: current.master_gain,
            source_midi: None,
            tracks: Vec::new(),
        }
    }
}

impl Default for TrackV1 {
    fn default() -> Self {
        Self {
            name: "Track".to_owned(),
            patch: "lead".to_owned(),
            midi_program: None,
            gain: 0.8,
            pan: 0.0,
            muted: false,
            notes: Vec::new(),
            pitch_bends: Vec::new(),
            aftertouch: Vec::new(),
            controllers: Vec::new(),
            pitch_bend_range_semitones: 2.0,
        }
    }
}

impl ProjectV1 {
    pub(crate) fn into_current(self) -> Project {
        let mut instruments = Vec::with_capacity(self.tracks.len());
        let tracks = self
            .tracks
            .into_iter()
            .map(|track| {
                let instrument = instruments.len();
                instruments.push(Instrument {
                    name: track.name.clone(),
                    source: InstrumentSource::Synth { patch: track.patch },
                    midi_program: track.midi_program,
                    gain: track.gain,
                    pan: track.pan,
                    pitch_bend_range_semitones: track.pitch_bend_range_semitones,
                    ..Instrument::default()
                });
                Track {
                    name: track.name,
                    instrument,
                    muted: track.muted,
                    notes: track.notes,
                    pitch_bends: track.pitch_bends,
                    aftertouch: track.aftertouch,
                    controllers: track.controllers,
                    mixer_automation: Vec::new(),
                    parts: Vec::<Part>::new(),
                }
            })
            .collect();
        Project {
            format: FORMAT_NAME.to_owned(),
            version: FORMAT_VERSION,
            name: self.name,
            tempo_bpm: self.tempo_bpm,
            tempo_changes: self.tempo_changes,
            beats_per_bar: self.beats_per_bar,
            beat_unit: self.beat_unit,
            ticks_per_quarter: self.ticks_per_quarter,
            sample_rate: self.sample_rate,
            master_gain: self.master_gain,
            send_buses: std::array::from_fn(|_| evil_synth::SendBus::default()),
            source_midi: self.source_midi,
            instruments,
            patterns: Vec::new(),
            tracks,
        }
    }
}
