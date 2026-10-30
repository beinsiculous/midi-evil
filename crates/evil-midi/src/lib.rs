mod import;
mod native;

pub use evil_performance::{
    Aftertouch, ControllerEvent, Performance, PerformanceNote, PerformanceTrack, PitchBend,
    TempoChange,
};
pub use import::{import_midi, import_midi_file, rename_midi_track};
pub use native::{NativeMidiReport, export_native_project, export_native_project_to_path};

use midly::num::{u4, u28};
use midly::{MetaMessage, MidiMessage, TrackEvent, TrackEventKind};
use thiserror::Error;

const MAX_MIDI_DELTA: u64 = 0x0fff_ffff;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tempo {0} BPM cannot be represented in a standard MIDI tempo event")]
    UnsupportedTempo(f32),
    #[error("beats per bar {0} cannot be represented in a MIDI time signature")]
    UnsupportedMeter(u32),
    #[error("project has {0} instruments with parts; MIDI supports at most 16 channels")]
    TooManyTracks(usize),
    #[error("MIDI track {0} does not exist")]
    MissingTrack(usize),
    #[error("could not encode standard MIDI: {0}")]
    Encode(&'static str),
    #[error("could not decode standard MIDI: {0}")]
    Decode(#[from] midly::Error),
    #[error("format 2 MIDI files with independent sequences are not supported")]
    UnsupportedFormat,
    #[error("invalid MIDI time signature {numerator}/2^{denominator}")]
    InvalidTimeSignature { numerator: u8, denominator: u8 },
    #[error("SMPTE or zero-division MIDI timing is not supported")]
    UnsupportedTiming,
    #[error("MIDI export is too large for the engine: {bytes} bytes, {events} events")]
    ExportTooLarge { bytes: usize, events: usize },
    #[error("timeline contains a gap larger than a MIDI delta-time can represent")]
    TimelineTooLong,
    #[error(transparent)]
    NativeProject(#[from] evil_project::Error),
    #[error(transparent)]
    Patch(#[from] evil_render::Error),
    #[error("could not access {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

struct TimedEvent {
    tick: u64,
    priority: u8,
    message: MidiMessage,
}

fn meter_exponent(beat_unit: u32) -> Result<u8, Error> {
    if beat_unit == 0 || !beat_unit.is_power_of_two() || beat_unit > (1 << 31) {
        return Err(Error::UnsupportedMeter(beat_unit));
    }
    Ok(beat_unit.ilog2() as u8)
}

fn render_track(
    events: Vec<TimedEvent>,
    channel: u4,
    name: &str,
) -> Result<Vec<TrackEvent<'_>>, Error> {
    let mut track = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::TrackName(name.as_bytes())),
    }];
    let mut previous_tick = 0_u64;
    for event in events {
        let delta = event.tick - previous_tick;
        if delta > MAX_MIDI_DELTA {
            return Err(Error::TimelineTooLong);
        }
        track.push(TrackEvent {
            delta: u28::new(delta as u32),
            kind: TrackEventKind::Midi {
                channel,
                message: event.message,
            },
        });
        previous_tick = event.tick;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(track)
}
