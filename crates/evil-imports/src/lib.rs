//! Standard MIDI loading and assisted reconstruction from user-owned WAV files.

mod midi;
mod wav;

pub use evil_performance::{
    Aftertouch, ControllerEvent, Performance, PerformanceNote, PerformanceTrack, PitchBend,
    TempoChange,
};
pub use midi::{import_midi, import_midi_file};
pub use wav::{
    AudioAnalysis, AudioBuffer, DraftReport, TempoCandidate, analyze, analyze_file, create_draft,
    read_wav,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tempo {0} BPM cannot be represented in a standard MIDI tempo event")]
    UnsupportedTempo(f32),
    #[error("format 2 MIDI files with independent sequences are not supported")]
    UnsupportedFormat,
    #[error("invalid MIDI time signature {numerator}/2^{denominator}")]
    InvalidTimeSignature { numerator: u8, denominator: u8 },
    #[error("SMPTE or zero-division MIDI timing is not supported")]
    UnsupportedTiming,
    #[error("could not decode standard MIDI: {0}")]
    Decode(#[from] midly::Error),
    #[error(transparent)]
    Wav(#[from] hound::Error),
    #[error("invalid WAV: {0}")]
    InvalidWav(String),
    #[error("no onsets were detected")]
    NoOnsets,
    #[error("no usable tempo was detected; provide --bpm")]
    NoTempo,
    #[error("grid must be 1, 2, 4, 8, 16, or 32, got {0}")]
    InvalidGrid(u8),
    #[error(transparent)]
    Project(#[from] evil_project::Error),
    #[error("could not access {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}
