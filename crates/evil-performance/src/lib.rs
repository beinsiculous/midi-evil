use serde::{Deserialize, Serialize};

pub const DEFAULT_TICKS_PER_QUARTER: u16 = 480;

/// Continuous controllers currently mapped to synthesis parameters.
/// Other controllers remain editable and are preserved through MIDI export.
pub fn controller_is_rendered(controller: u8) -> bool {
    matches!(controller, 1 | 11)
}

#[derive(Debug)]
pub struct Performance {
    pub tempo_bpm: f32,
    pub beats_per_bar: u32,
    pub beat_unit: u32,
    pub ticks_per_quarter: u16,
    pub tempo_changes: Vec<TempoChange>,
    pub tracks: Vec<PerformanceTrack>,
}

#[derive(Debug, Clone, Copy)]
pub struct TempoChange {
    pub tick: u64,
    pub microseconds_per_quarter: u32,
}

#[derive(Debug)]
pub struct PerformanceTrack {
    pub track_index: usize,
    pub name: String,
    pub notes: Vec<PerformanceNote>,
    pub pitch_bends: Vec<PitchBend>,
    pub aftertouch: Vec<Aftertouch>,
    pub controllers: Vec<ControllerEvent>,
    pub pitch_bend_range_semitones: f32,
    pub program: Option<u8>,
    pub volume: Option<u8>,
    pub pan: Option<u8>,
    pub skipped_controller_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerformanceNote {
    pub note: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchBend {
    pub tick: u64,
    /// Signed 14-bit MIDI pitch bend, from -8192 through 8191.
    pub value: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aftertouch {
    pub tick: u64,
    /// MIDI pitch for polyphonic pressure, or `None` for channel pressure.
    pub note: Option<u8>,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ControllerEvent {
    pub tick: u64,
    pub controller: u8,
    pub value: u8,
}

impl Performance {
    pub fn note_count(&self) -> usize {
        self.tracks.iter().map(|track| track.notes.len()).sum()
    }

    pub fn pitch_bend_count(&self) -> usize {
        self.tracks
            .iter()
            .map(|track| track.pitch_bends.len())
            .sum()
    }

    pub fn aftertouch_count(&self) -> usize {
        self.tracks.iter().map(|track| track.aftertouch.len()).sum()
    }

    pub fn controller_count(&self) -> usize {
        self.tracks
            .iter()
            .map(|track| track.controllers.len())
            .sum()
    }

    pub fn skipped_controller_events(&self) -> usize {
        self.tracks
            .iter()
            .map(|track| track.skipped_controller_events)
            .sum()
    }

    pub fn note_timing(&self, note: &PerformanceNote) -> (f64, f64) {
        let start = self.seconds_at(note.start_tick);
        let end = self.seconds_at(note.start_tick + note.duration_ticks);
        (start, end - start)
    }

    pub fn duration_seconds(&self) -> f64 {
        self.tracks
            .iter()
            .flat_map(|track| &track.notes)
            .map(|note| note.start_tick + note.duration_ticks)
            .max()
            .map_or(0.0, |tick| self.seconds_at(tick))
    }

    pub fn seconds_at(&self, tick: u64) -> f64 {
        let mut elapsed = 0.0;
        let mut previous_tick = 0;
        let mut microseconds = 500_000;
        for change in &self.tempo_changes {
            if change.tick > tick {
                break;
            }
            if change.tick == 0 {
                microseconds = change.microseconds_per_quarter;
                continue;
            }
            elapsed += self.seconds_between(previous_tick, change.tick, microseconds);
            previous_tick = change.tick;
            microseconds = change.microseconds_per_quarter;
        }
        elapsed + self.seconds_between(previous_tick, tick, microseconds)
    }

    fn seconds_between(&self, start: u64, end: u64, microseconds_per_quarter: u32) -> f64 {
        (end - start) as f64 * f64::from(microseconds_per_quarter)
            / (f64::from(self.ticks_per_quarter) * 1_000_000.0)
    }
}
