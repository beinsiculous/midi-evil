use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub use evil_performance::{Aftertouch, ControllerEvent, PitchBend, controller_is_rendered};
use evil_performance::{
    Performance, PerformanceNote, PerformanceTrack, TempoChange as PerformanceTempoChange,
};
use evil_synth::{SEND_COUNT, SendBus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod instrument;
mod legacy;
mod pattern;
mod sequence;

pub use instrument::{DEFAULT_DRUM_BASE_NOTE, DRUM_PAD_COUNT, Instrument, InstrumentSource};
pub use pattern::{Part, Pattern};
pub use sequence::{EventSequence, clean_overlaps, quantize_notes, transpose_notes};

pub const FORMAT_NAME: &str = "midi-evil-project";
pub const LEGACY_FORMAT_NAME: &str = "rusty-music-project";
pub const FORMAT_VERSION: u32 = 2;
pub const FILE_EXTENSION: &str = "evil";
pub const LEGACY_FILE_EXTENSION: &str = "rmp";
pub const NEW_TRACK_INSTRUMENT: usize = usize::MAX;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid MIDI Evil project: {0}")]
    InvalidProject(String),
    #[error("not a MIDI Evil project JSON document")]
    InvalidFormat,
    #[error("unsupported MIDI Evil project version {0}")]
    UnsupportedVersion(u32),
    #[error("could not access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse project JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub tempo_bpm: f32,
    /// Tempo changes after the initial tempo, ordered by tick.
    pub tempo_changes: Vec<TempoChange>,
    pub beats_per_bar: u32,
    pub beat_unit: u32,
    pub ticks_per_quarter: u16,
    pub sample_rate: u32,
    pub master_gain: f32,
    /// Two project-wide post-channel effect sends.
    pub send_buses: [SendBus; SEND_COUNT],
    /// Original MIDI file to update on save, if this project came from one.
    pub source_midi: Option<String>,
    pub instruments: Vec<Instrument>,
    pub patterns: Vec<Pattern>,
    pub tracks: Vec<Track>,
}

/// A sequencer track owns a unique timeline performance and may also repeat
/// shared patterns through arrangement parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub name: String,
    pub instrument: usize,
    pub muted: bool,
    pub notes: Vec<Note>,
    pub pitch_bends: Vec<PitchBend>,
    pub aftertouch: Vec<Aftertouch>,
    pub controllers: Vec<ControllerEvent>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TempoChange {
    pub tick: u64,
    pub bpm: f32,
}

#[derive(Debug, Default)]
pub struct NoteEdit {
    pub pitch: Option<u8>,
    pub velocity: Option<u8>,
    pub start_tick: Option<u64>,
    pub duration_ticks: Option<u64>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            format: FORMAT_NAME.to_owned(),
            version: FORMAT_VERSION,
            name: "Untitled".to_owned(),
            tempo_bpm: 120.0,
            tempo_changes: Vec::new(),
            beats_per_bar: 4,
            beat_unit: 4,
            ticks_per_quarter: evil_performance::DEFAULT_TICKS_PER_QUARTER,
            sample_rate: 48_000,
            master_gain: 0.8,
            send_buses: std::array::from_fn(|_| SendBus::default()),
            source_midi: None,
            instruments: Vec::new(),
            patterns: Vec::new(),
            tracks: Vec::new(),
        }
    }
}

impl Default for Track {
    fn default() -> Self {
        Self {
            name: "Track".to_owned(),
            instrument: NEW_TRACK_INSTRUMENT,
            muted: false,
            notes: Vec::new(),
            pitch_bends: Vec::new(),
            aftertouch: Vec::new(),
            controllers: Vec::new(),
            parts: Vec::new(),
        }
    }
}

impl Default for Note {
    fn default() -> Self {
        Self {
            pitch: 60,
            velocity: 100,
            start_tick: 0,
            duration_ticks: 128,
        }
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn from_performance(
        name: impl Into<String>,
        performance: &Performance,
    ) -> Result<Self, Error> {
        let mut instruments = Vec::with_capacity(performance.tracks.len());
        let tracks = performance
            .tracks
            .iter()
            .map(|track| {
                let instrument = instruments.len();
                instruments.push(Instrument {
                    name: track.name.clone(),
                    midi_program: track.program,
                    gain: track.volume.map_or(0.8, |volume| f32::from(volume) / 127.0),
                    pan: track.pan.map_or(0.0, |pan| match pan {
                        64 => 0.0,
                        0..=63 => f32::from(pan) / 64.0 - 1.0,
                        _ => f32::from(pan - 64) / 63.0,
                    }),
                    pitch_bend_range_semitones: track.pitch_bend_range_semitones,
                    ..Instrument::default()
                });
                Track {
                    name: track.name.clone(),
                    instrument,
                    notes: track
                        .notes
                        .iter()
                        .map(|note| Note {
                            pitch: note.note,
                            velocity: note.velocity,
                            start_tick: note.start_tick,
                            duration_ticks: note.duration_ticks,
                        })
                        .collect(),
                    pitch_bends: track.pitch_bends.clone(),
                    aftertouch: track.aftertouch.clone(),
                    controllers: track.controllers.clone(),
                    ..Track::default()
                }
            })
            .collect();
        let project = Self {
            name: name.into(),
            tempo_bpm: performance.tempo_bpm,
            tempo_changes: performance
                .tempo_changes
                .iter()
                .filter(|change| change.tick > 0)
                .map(|change| TempoChange {
                    tick: change.tick,
                    bpm: 60_000_000.0 / change.microseconds_per_quarter as f32,
                })
                .collect(),
            beats_per_bar: performance.beats_per_bar,
            beat_unit: performance.beat_unit,
            ticks_per_quarter: performance.ticks_per_quarter,
            instruments,
            tracks,
            ..Self::default()
        };
        project.validate()?;
        Ok(project)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let json = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json(&json)
    }

    pub fn from_json(json: &str) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        let mut project = if version == 1 {
            let legacy: legacy::ProjectV1 = serde_json::from_value(value)?;
            if legacy.format != FORMAT_NAME && legacy.format != LEGACY_FORMAT_NAME {
                return Err(Error::InvalidFormat);
            }
            legacy.into_current()
        } else {
            serde_json::from_value::<Self>(value)?
        };
        if project.format == LEGACY_FORMAT_NAME {
            project.format = FORMAT_NAME.to_owned();
        }
        project.validate()?;
        Ok(project)
    }

    pub fn to_json_pretty(&self) -> Result<String, Error> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let json = self.to_json_pretty()?;
        atomic_write(path, json.as_bytes())
    }

    pub fn add_instrument(&mut self, instrument: Instrument) -> Result<usize, Error> {
        instrument.validate()?;
        let index = self.instruments.len();
        self.instruments.push(instrument);
        Ok(index)
    }

    pub fn instrument_for_track(&self, track_index: usize) -> Result<&Instrument, Error> {
        let track = self
            .tracks
            .get(track_index)
            .ok_or_else(|| invalid(&format!("track {track_index} does not exist")))?;
        self.instruments
            .get(track.instrument)
            .ok_or_else(|| invalid(&format!("track {track_index} has no instrument")))
    }

    pub fn instrument_for_track_mut(
        &mut self,
        track_index: usize,
    ) -> Result<&mut Instrument, Error> {
        let index = self
            .tracks
            .get(track_index)
            .ok_or_else(|| invalid(&format!("track {track_index} does not exist")))?
            .instrument;
        self.instruments
            .get_mut(index)
            .ok_or_else(|| invalid(&format!("track {track_index} has no instrument")))
    }

    pub fn add_pattern(&mut self, pattern: Pattern) -> Result<usize, Error> {
        pattern.validate()?;
        let index = self.patterns.len();
        self.patterns.push(pattern);
        Ok(index)
    }

    pub fn add_part(&mut self, track_index: usize, part: Part) -> Result<usize, Error> {
        self.validate_part(part)?;
        let track = self.track_mut(track_index)?;
        let index = track.parts.len();
        track.parts.push(part);
        Ok(index)
    }

    pub fn add_track(&mut self, mut track: Track) -> Result<usize, Error> {
        validate_track(&track)?;
        if track.instrument == NEW_TRACK_INSTRUMENT {
            let instrument = Instrument {
                name: track.name.clone(),
                ..Instrument::default()
            };
            track.instrument = self.add_instrument(instrument)?;
        } else if track.instrument >= self.instruments.len() {
            return Err(invalid("track instrument does not exist"));
        }
        let index = self.tracks.len();
        self.tracks.push(track);
        Ok(index)
    }

    pub fn remove_track(&mut self, track_index: usize) -> Result<Track, Error> {
        if track_index >= self.tracks.len() {
            return Err(invalid(&format!("track {track_index} does not exist")));
        }
        Ok(self.tracks.remove(track_index))
    }

    pub fn duplicate_track(&mut self, track_index: usize) -> Result<usize, Error> {
        let mut track = self
            .tracks
            .get(track_index)
            .cloned()
            .ok_or_else(|| invalid(&format!("track {track_index} does not exist")))?;
        let mut instrument = self.instrument_for_track(track_index)?.clone();
        instrument.name = format!("{} Copy", instrument.name);
        track.instrument = self.add_instrument(instrument)?;
        track.name = format!("{} Copy", track.name);
        let new_index = track_index + 1;
        self.tracks.insert(new_index, track);
        Ok(new_index)
    }

    pub fn move_track(&mut self, track_index: usize, new_index: usize) -> Result<(), Error> {
        if track_index >= self.tracks.len() || new_index >= self.tracks.len() {
            return Err(invalid("track move is outside the track list"));
        }
        let track = self.tracks.remove(track_index);
        self.tracks.insert(new_index, track);
        Ok(())
    }

    pub fn add_note(&mut self, track_index: usize, note: Note) -> Result<usize, Error> {
        validate_note(&note)?;
        let track = self.track_mut(track_index)?;
        let index = track.notes.len();
        track.notes.push(note);
        Ok(index)
    }

    pub fn edit_note(
        &mut self,
        track_index: usize,
        note_index: usize,
        edit: NoteEdit,
    ) -> Result<(), Error> {
        if edit.pitch.is_none()
            && edit.velocity.is_none()
            && edit.start_tick.is_none()
            && edit.duration_ticks.is_none()
        {
            return Err(invalid("no note changes requested"));
        }
        let track = self.track_mut(track_index)?;
        let Some(existing) = track.notes.get(note_index).copied() else {
            return Err(invalid(&format!(
                "note {note_index} does not exist on track {track_index}"
            )));
        };
        let mut updated = existing;
        if let Some(pitch) = edit.pitch {
            updated.pitch = pitch;
        }
        if let Some(velocity) = edit.velocity {
            updated.velocity = velocity;
        }
        if let Some(start_tick) = edit.start_tick {
            updated.start_tick = start_tick;
        }
        if let Some(duration_ticks) = edit.duration_ticks {
            updated.duration_ticks = duration_ticks;
        }
        validate_note(&updated)?;
        track.notes[note_index] = updated;
        Ok(())
    }

    pub fn remove_note(&mut self, track_index: usize, note_index: usize) -> Result<Note, Error> {
        let track = self.track_mut(track_index)?;
        if note_index >= track.notes.len() {
            return Err(invalid(&format!(
                "note {note_index} does not exist on track {track_index}"
            )));
        }
        Ok(track.notes.remove(note_index))
    }

    pub fn set_track_muted(&mut self, track_index: usize, muted: bool) -> Result<(), Error> {
        self.track_mut(track_index)?.muted = muted;
        Ok(())
    }

    fn track_mut(&mut self, track_index: usize) -> Result<&mut Track, Error> {
        self.tracks
            .get_mut(track_index)
            .ok_or_else(|| invalid(&format!("track {track_index} does not exist")))
    }

    pub fn to_performance(&self) -> Result<Performance, Error> {
        self.validate()?;
        let tracks = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(track_index, track)| {
                if track.muted {
                    return None;
                }
                let instrument = &self.instruments[track.instrument];
                let mut notes = track.notes.clone();
                let mut pitch_bends = track.pitch_bends.clone();
                let mut aftertouch = track.aftertouch.clone();
                let mut controllers = track.controllers.clone();
                for part in &track.parts {
                    pattern::expand_part(
                        &self.patterns[part.pattern],
                        *part,
                        &mut notes,
                        &mut pitch_bends,
                        &mut aftertouch,
                        &mut controllers,
                    );
                }
                if notes.is_empty()
                    && pitch_bends.is_empty()
                    && aftertouch.is_empty()
                    && controllers.is_empty()
                {
                    return None;
                }
                notes.sort_by_key(|note| note.start_tick);
                pitch_bends.sort_by_key(|event| event.tick);
                aftertouch.sort_by_key(|event| event.tick);
                controllers.sort_by_key(|event| event.tick);
                Some(PerformanceTrack {
                    track_index,
                    name: track.name.clone(),
                    notes: notes
                        .iter()
                        .map(|note| PerformanceNote {
                            note: note.pitch,
                            velocity: note.velocity,
                            start_tick: note.start_tick,
                            duration_ticks: note.duration_ticks,
                        })
                        .collect(),
                    pitch_bends,
                    aftertouch,
                    controllers,
                    pitch_bend_range_semitones: instrument.pitch_bend_range_semitones,
                    skipped_controller_events: 0,
                    program: instrument.midi_program,
                    volume: Some((instrument.gain * 127.0).round() as u8),
                    pan: Some(((instrument.pan + 1.0) * 63.5).round() as u8),
                })
            })
            .collect();
        let mut tempo_changes = Vec::with_capacity(self.tempo_changes.len() + 1);
        tempo_changes.push(PerformanceTempoChange {
            tick: 0,
            microseconds_per_quarter: tempo_microseconds(self.tempo_bpm)?,
        });
        for change in &self.tempo_changes {
            tempo_changes.push(PerformanceTempoChange {
                tick: change.tick,
                microseconds_per_quarter: tempo_microseconds(change.bpm)?,
            });
        }
        Ok(Performance {
            tempo_bpm: self.tempo_bpm,
            beats_per_bar: self.beats_per_bar,
            beat_unit: self.beat_unit,
            ticks_per_quarter: self.ticks_per_quarter,
            tempo_changes,
            tracks,
        })
    }

    /// Limit arrangement playback without modifying shared pattern contents.
    pub fn truncate_timeline(&mut self, end_tick: u64) {
        for track in &mut self.tracks {
            track.notes.retain(|note| note.start_tick < end_tick);
            track.pitch_bends.retain(|event| event.tick < end_tick);
            track.aftertouch.retain(|event| event.tick < end_tick);
            track.controllers.retain(|event| event.tick < end_tick);
            for part in &mut track.parts {
                part.length_ticks = part
                    .length_ticks
                    .min(end_tick.saturating_sub(part.start_tick));
            }
            track.parts.retain(|part| part.length_ticks > 0);
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.format != FORMAT_NAME {
            return Err(Error::InvalidFormat);
        }
        if self.version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(self.version));
        }
        if self.name.trim().is_empty() {
            return Err(invalid("project name cannot be empty"));
        }
        tempo_microseconds(self.tempo_bpm)?;
        let mut previous_tick = 0;
        for change in &self.tempo_changes {
            if change.tick == 0 || change.tick <= previous_tick {
                return Err(invalid(
                    "tempo changes must have strictly increasing ticks greater than zero",
                ));
            }
            tempo_microseconds(change.bpm)?;
            previous_tick = change.tick;
        }
        if self.beats_per_bar == 0 {
            return Err(invalid("beats_per_bar must be greater than zero"));
        }
        if self.beat_unit == 0 || !self.beat_unit.is_power_of_two() {
            return Err(invalid("beat_unit must be a power of two"));
        }
        if self.ticks_per_quarter == 0 {
            return Err(invalid("ticks_per_quarter must be greater than zero"));
        }
        if !(8_000..=192_000).contains(&self.sample_rate) {
            return Err(invalid("sample_rate must be between 8000 and 192000"));
        }
        if !self.master_gain.is_finite() || !(0.0..=1.0).contains(&self.master_gain) {
            return Err(invalid("master_gain must be between 0 and 1"));
        }
        for bus in &self.send_buses {
            bus.validate()
                .map_err(|error| invalid(&error.to_string()))?;
        }
        if self.source_midi.as_deref().is_some_and(str::is_empty) {
            return Err(invalid("source_midi cannot be empty when present"));
        }
        for instrument in &self.instruments {
            instrument.validate()?;
        }
        for pattern in &self.patterns {
            pattern.validate()?;
        }
        for track in &self.tracks {
            validate_track(track)?;
            if track.instrument >= self.instruments.len() {
                return Err(invalid("track instrument does not exist"));
            }
            for part in &track.parts {
                self.validate_part(*part)?;
            }
        }
        Ok(())
    }

    fn validate_part(&self, part: Part) -> Result<(), Error> {
        if part.pattern >= self.patterns.len() {
            return Err(invalid("part pattern does not exist"));
        }
        if part.length_ticks == 0 {
            return Err(invalid("part length must be greater than zero"));
        }
        let part_end = part
            .start_tick
            .checked_add(part.length_ticks)
            .ok_or_else(|| invalid("part end tick exceeds the supported range"))?;
        let pattern = &self.patterns[part.pattern];
        let last_occurrence = (part.length_ticks - 1) / pattern.length_ticks;
        let last_start = part
            .start_tick
            .checked_add(last_occurrence * pattern.length_ticks)
            .ok_or_else(|| invalid("part occurrence exceeds the supported range"))?;
        for note in &pattern.notes {
            let start = last_start
                .checked_add(note.start_tick)
                .ok_or_else(|| invalid("expanded note start exceeds the supported range"))?;
            if start < part_end {
                start
                    .checked_add(note.duration_ticks)
                    .ok_or_else(|| invalid("expanded note end exceeds the supported range"))?;
            }
        }
        Ok(())
    }
}

pub fn atomic_write(path: impl AsRef<Path>, data: &[u8]) -> Result<(), Error> {
    let path = path.as_ref();
    let permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("output path must have a UTF-8 file name"))?;
    let temporary_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|source| Error::Io {
                path: temporary_path.clone(),
                source,
            })?;
        file.write_all(data)
            .and_then(|()| file.sync_all())
            .map_err(|source| Error::Io {
                path: temporary_path.clone(),
                source,
            })?;
        if let Some(permissions) = permissions {
            fs::set_permissions(&temporary_path, permissions).map_err(|source| Error::Io {
                path: temporary_path.clone(),
                source,
            })?;
        }
        drop(file);
        fs::rename(&temporary_path, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

fn validate_track(track: &Track) -> Result<(), Error> {
    if track.name.trim().is_empty() {
        return Err(invalid("track name cannot be empty"));
    }
    pattern::validate_events(
        1,
        &track.notes,
        &track.pitch_bends,
        &track.aftertouch,
        &track.controllers,
    )
}

pub(crate) fn validate_note(note: &Note) -> Result<(), Error> {
    if note.pitch > 127 || note.velocity == 0 || note.velocity > 127 {
        return Err(invalid(
            "note pitch must be 0..=127 and velocity must be 1..=127",
        ));
    }
    if note.duration_ticks == 0 {
        return Err(invalid("note duration must be greater than zero"));
    }
    note.start_tick
        .checked_add(note.duration_ticks)
        .ok_or_else(|| invalid("note end tick exceeds the supported range"))?;
    Ok(())
}

fn tempo_microseconds(tempo_bpm: f32) -> Result<u32, Error> {
    if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
        return Err(invalid("tempo_bpm must be finite and positive"));
    }
    let microseconds = (60_000_000.0 / f64::from(tempo_bpm)).round();
    if !(1.0..=0x00ff_ffff as f64).contains(&microseconds) {
        return Err(invalid("tempo_bpm is outside the MIDI-compatible range"));
    }
    Ok(microseconds as u32)
}

pub(crate) fn invalid(message: &str) -> Error {
    Error::InvalidProject(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_project_round_trips_and_converts_to_performance() {
        let mut project = Project::new("Test Song");
        project.ticks_per_quarter = 128;
        project.source_midi = Some("song.mid".to_owned());
        let track = project
            .add_track(Track {
                name: "Lead".to_owned(),
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].pan = -0.25;
        project.instruments[0].inserts =
            vec![evil_synth::InsertEffect::Bitcrush(evil_synth::Bitcrush {
                bits: 6,
                downsample: 2,
                mix: 0.75,
            })];
        project.instruments[0].send_gains = [0.25, 0.5];
        project.send_buses[0].effect = Some(evil_synth::InsertEffect::Waveshaper(
            evil_synth::Waveshaper::default(),
        ));
        project.send_buses[0].gain = 0.5;
        project
            .add_note(
                track,
                Note {
                    pitch: 69,
                    velocity: 110,
                    start_tick: 128,
                    duration_ticks: 64,
                },
            )
            .unwrap();
        let json = project.to_json_pretty().unwrap();
        let decoded = Project::from_json(&json).unwrap();
        let performance = decoded.to_performance().unwrap();
        assert_eq!(performance.note_count(), 1);
        assert_eq!(performance.tracks[0].name, "Lead");
        let (start, duration) = performance.note_timing(&performance.tracks[0].notes[0]);
        assert!((start - 0.5).abs() < 0.000_001);
        assert!((duration - 0.25).abs() < 0.000_001);
        assert_eq!(decoded.instruments[0].pan, -0.25);
        assert_eq!(
            decoded.instruments[0].inserts,
            project.instruments[0].inserts
        );
        assert_eq!(decoded.instruments[0].send_gains, [0.25, 0.5]);
        assert_eq!(decoded.send_buses, project.send_buses);
        assert_eq!(decoded.source_midi.as_deref(), Some("song.mid"));
    }

    #[test]
    fn version_one_tracks_migrate_to_instruments_without_losing_performance() {
        let project = Project::from_json(
            r#"{
                "format":"midi-evil-project","version":1,"name":"Old Song",
                "tracks":[{"name":"Lead","patch":"nasty","gain":0.5,"pan":-0.25,
                    "pitch_bend_range_semitones":7.0,
                    "notes":[{"pitch":69,"velocity":110,"start_tick":128,"duration_ticks":64}]}]
            }"#,
        )
        .unwrap();
        assert_eq!(project.version, FORMAT_VERSION);
        assert_eq!(project.instruments[0].patch(), Some("nasty"));
        assert_eq!(project.instruments[0].pitch_bend_range_semitones, 7.0);
        assert_eq!(project.to_performance().unwrap().note_count(), 1);
        assert_eq!(project.tracks[0].instrument, 0);
        assert!(project.instruments[0].inserts.is_empty());
        assert_eq!(project.instruments[0].send_gains, [0.0; SEND_COUNT]);
        assert!(project.send_buses.iter().all(|bus| bus.gain == 0.0));
    }

    #[test]
    fn shared_patterns_expand_into_repeated_and_cropped_parts() {
        let mut project = Project::new("Patterns");
        project.add_track(Track::default()).unwrap();
        let pattern = project
            .add_pattern(Pattern {
                name: "Beat".to_owned(),
                length_ticks: 480,
                notes: vec![Note {
                    start_tick: 120,
                    duration_ticks: 600,
                    ..Note::default()
                }],
                controllers: vec![ControllerEvent {
                    tick: 120,
                    controller: 1,
                    value: 90,
                }],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 480,
                    length_ticks: 900,
                },
            )
            .unwrap();
        let performance = project.to_performance().unwrap();
        let starts: Vec<_> = performance.tracks[0]
            .notes
            .iter()
            .map(|note| note.start_tick)
            .collect();
        assert_eq!(starts, [600, 1080]);
        assert_eq!(performance.tracks[0].notes[0].duration_ticks, 600);
        assert_eq!(performance.tracks[0].controllers.len(), 2);
        project.truncate_timeline(1_000);
        assert_eq!(project.to_performance().unwrap().note_count(), 1);
    }

    #[test]
    fn part_expansion_rejects_note_end_overflow_without_adding_the_part() {
        let mut project = Project::new("Overflow");
        project.add_track(Track::default()).unwrap();
        let pattern = project
            .add_pattern(Pattern {
                notes: vec![Note {
                    duration_ticks: 100,
                    ..Note::default()
                }],
                ..Pattern::default()
            })
            .unwrap();
        let result = project.add_part(
            0,
            Part {
                pattern,
                start_tick: u64::MAX - 5,
                length_ticks: 5,
            },
        );
        assert!(result.is_err());
        assert!(project.tracks[0].parts.is_empty());
    }

    #[test]
    fn tracks_can_share_an_instrument_and_scale_beyond_midi_channels() {
        let mut project = Project::new("Shared");
        let instrument = project.add_instrument(Instrument::default()).unwrap();
        for index in 0..64 {
            project
                .add_track(Track {
                    name: format!("Track {index}"),
                    instrument,
                    notes: vec![Note::default()],
                    ..Track::default()
                })
                .unwrap();
        }
        assert_eq!(project.instruments.len(), 1);
        assert_eq!(project.to_performance().unwrap().note_count(), 64);
    }

    #[test]
    fn native_capacity_covers_instruments_patterns_events_and_parts() {
        let mut project = Project::new("Capacity");
        let instruments: Vec<_> = (0..16)
            .map(|index| {
                project
                    .add_instrument(Instrument::synth(format!("Instrument {index}"), "lead"))
                    .unwrap()
            })
            .collect();
        for index in 0..64 {
            project
                .add_track(Track {
                    name: format!("Track {index}"),
                    instrument: instruments[index % 16],
                    ..Track::default()
                })
                .unwrap();
        }
        for index in 0..100 {
            let event_count = if index == 99 { 1_000 } else { 1 };
            project
                .add_pattern(Pattern {
                    name: format!("Pattern {index}"),
                    notes: (0..event_count)
                        .map(|_| Note {
                            duration_ticks: 1,
                            ..Note::default()
                        })
                        .collect(),
                    ..Pattern::default()
                })
                .unwrap();
        }
        for index in 0..1_000 {
            project
                .add_part(
                    index % 64,
                    Part {
                        pattern: index % 99,
                        start_tick: 0,
                        length_ticks: 1_920,
                    },
                )
                .unwrap();
        }
        project
            .add_part(
                0,
                Part {
                    pattern: 99,
                    start_tick: 0,
                    length_ticks: 1_920,
                },
            )
            .unwrap();
        let performance = project.to_performance().unwrap();
        assert_eq!(project.instruments.len(), 16);
        assert_eq!(project.tracks.len(), 64);
        assert_eq!(project.patterns.len(), 100);
        assert_eq!(project.patterns[99].notes.len(), 1_000);
        assert_eq!(
            project
                .tracks
                .iter()
                .map(|track| track.parts.len())
                .sum::<usize>(),
            1_001
        );
        assert_eq!(performance.tracks.len(), 64);
        assert_eq!(performance.note_count(), 2_000);
    }

    #[test]
    fn muted_tracks_are_excluded_from_performance() {
        let mut project = Project::new("Muted");
        project
            .add_track(Track {
                muted: true,
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        assert_eq!(project.to_performance().unwrap().note_count(), 0);
    }

    #[test]
    fn note_editing_and_mute_changes_are_validated() {
        let mut project = Project::new("Editing");
        project.add_track(Track::default()).unwrap();
        project.add_note(0, Note::default()).unwrap();
        project
            .edit_note(
                0,
                0,
                NoteEdit {
                    pitch: Some(62),
                    velocity: Some(90),
                    start_tick: Some(256),
                    duration_ticks: Some(64),
                },
            )
            .unwrap();
        assert_eq!(project.tracks[0].notes[0].pitch, 62);
        assert_eq!(project.tracks[0].notes[0].start_tick, 256);
        assert_eq!(project.tracks[0].notes[0].duration_ticks, 64);
        assert!(
            project
                .edit_note(
                    0,
                    0,
                    NoteEdit {
                        duration_ticks: Some(0),
                        ..NoteEdit::default()
                    }
                )
                .is_err()
        );
        assert_eq!(project.tracks[0].notes[0].duration_ticks, 64);
        assert!(project.remove_note(0, 99).is_err());
        assert_eq!(project.remove_note(0, 0).unwrap().pitch, 62);
        project.set_track_muted(0, true).unwrap();
        assert!(project.tracks[0].muted);
        project.set_track_muted(0, false).unwrap();
        assert!(!project.tracks[0].muted);
    }

    #[test]
    fn track_management_duplicates_reorders_and_removes_tracks() {
        let mut project = Project::new("Tracks");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project
            .add_track(Track {
                name: "Bass".to_owned(),
                ..Track::default()
            })
            .unwrap();
        assert_eq!(project.duplicate_track(0).unwrap(), 1);
        assert_eq!(project.tracks[1].name, "Lead Copy");
        assert_eq!(project.tracks[1].notes.len(), 1);
        assert_ne!(project.tracks[0].instrument, project.tracks[1].instrument);
        project.move_track(2, 0).unwrap();
        assert_eq!(project.tracks[0].name, "Bass");
        assert_eq!(project.remove_track(1).unwrap().name, "Lead");
        assert_eq!(project.tracks.len(), 2);
        assert!(project.move_track(99, 0).is_err());
    }

    #[test]
    fn track_automation_round_trips_through_native_json() {
        let mut project = Project::new("Bend");
        project.add_track(Track::default()).unwrap();
        project.instruments[0].pitch_bend_range_semitones = 7.0;
        project.tracks[0].pitch_bends = vec![PitchBend {
            tick: 240,
            value: -4096,
        }];
        project.tracks[0].aftertouch = vec![Aftertouch {
            tick: 240,
            note: Some(60),
            value: 96,
        }];
        project.tracks[0].controllers = vec![ControllerEvent {
            tick: 240,
            controller: 1,
            value: 96,
        }];
        let decoded = Project::from_json(&project.to_json_pretty().unwrap()).unwrap();
        assert_eq!(decoded.tracks[0].pitch_bends, project.tracks[0].pitch_bends);
        assert_eq!(decoded.tracks[0].aftertouch, project.tracks[0].aftertouch);
        assert_eq!(decoded.tracks[0].controllers, project.tracks[0].controllers);
        assert_eq!(decoded.instruments[0].pitch_bend_range_semitones, 7.0);
    }

    #[test]
    fn performance_import_preserves_tempo_changes() {
        let performance = Performance {
            tempo_bpm: 120.0,
            beats_per_bar: 4,
            beat_unit: 4,
            ticks_per_quarter: 128,
            tempo_changes: vec![
                PerformanceTempoChange {
                    tick: 0,
                    microseconds_per_quarter: 500_000,
                },
                PerformanceTempoChange {
                    tick: 128,
                    microseconds_per_quarter: 250_000,
                },
            ],
            tracks: vec![PerformanceTrack {
                track_index: 5,
                name: "Imported".to_owned(),
                notes: vec![PerformanceNote {
                    note: 60,
                    velocity: 100,
                    start_tick: 128,
                    duration_ticks: 128,
                }],
                pitch_bends: Vec::new(),
                aftertouch: Vec::new(),
                controllers: Vec::new(),
                pitch_bend_range_semitones: 2.0,
                skipped_controller_events: 0,
                program: Some(80),
                volume: Some(100),
                pan: Some(64),
            }],
        };
        let project = Project::from_performance("MIDI", &performance).unwrap();
        assert_eq!(project.tracks[0].name, "Imported");
        assert_eq!(project.instruments[0].midi_program, Some(80));
        assert_eq!(project.instruments[0].pan, 0.0);
        assert_eq!(project.tempo_changes.len(), 1);
        assert_eq!(project.tempo_changes[0].tick, 128);
        assert_eq!(project.tempo_changes[0].bpm, 240.0);
        let performance = project.to_performance().unwrap();
        let (start, duration) = performance.note_timing(&performance.tracks[0].notes[0]);
        assert!((start - 0.5).abs() < 0.000_001);
        assert!((duration - 0.25).abs() < 0.000_001);
    }

    #[test]
    fn legacy_project_json_is_migrated_to_the_evil_format() {
        let project =
            Project::from_json(r#"{"format":"rusty-music-project","name":"Legacy Song"}"#).unwrap();
        assert_eq!(project.format, FORMAT_NAME);
        assert_eq!(project.name, "Legacy Song");
        assert!(
            project
                .to_json_pretty()
                .unwrap()
                .contains("midi-evil-project")
        );
    }

    #[test]
    fn save_and_load_round_trip_through_the_filesystem() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.evil");
        let project = Project::new("Saved Song");
        project.save(&path).unwrap();
        assert_eq!(Project::load(&path).unwrap().name, "Saved Song");
        project.save(&path).unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn channel_and_send_graph_settings_are_validated() {
        let mut project = Project::new("Invalid graph");
        project.add_track(Track::default()).unwrap();
        project.instruments[0].inserts = (0..5)
            .map(|_| evil_synth::InsertEffect::Bitcrush(evil_synth::Bitcrush::default()))
            .collect();
        assert!(project.validate().is_err());
        project.instruments[0].inserts.truncate(4);
        project.instruments[0].send_gains[0] = f32::NAN;
        assert!(project.validate().is_err());
        project.instruments[0].send_gains[0] = 0.0;
        project.send_buses[1].gain = -0.1;
        assert!(project.validate().is_err());
    }

    #[test]
    fn invalid_notes_and_versions_are_rejected() {
        let mut project = Project::new("Invalid");
        assert!(
            project
                .add_note(
                    0,
                    Note {
                        duration_ticks: 0,
                        ..Note::default()
                    }
                )
                .is_err()
        );
        project.version = 99;
        assert!(matches!(
            project.validate(),
            Err(Error::UnsupportedVersion(99))
        ));
    }
}
