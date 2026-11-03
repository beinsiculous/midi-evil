mod aux_envelope;
mod delay;
mod envelope;
mod filter;
mod lfo;
mod live;
mod mix;
mod modal;
mod oscillator;
mod patch;
mod presets;
mod render;
mod sample;
mod sync;
mod unison;
mod voice;
mod wav;

pub use live::{LiveControls, LiveSynth};

pub use aux_envelope::AuxEnvelope;
pub use delay::{Delay, DelayMode};
pub use envelope::Envelope;
pub use filter::{Filter, FilterMode};
pub use lfo::{Lfo, LfoDestination, LfoWaveform};
pub use mix::{MonoTrack, StereoSound, StereoTrack, mix_stereo, mix_stereo_with_tracks};
pub use modal::ModalPartial;
pub use oscillator::{Oscillator, OscillatorMode, Waveform};
pub use patch::Patch;
pub use presets::{PRESET_NAMES, preset};
pub use render::{
    RenderedSound, SynthAftertouch, SynthController, SynthNote, SynthPitchBend, SynthTempoChange,
    render, render_notes, render_notes_dry, render_notes_dry_with_automation,
    render_notes_dry_with_full_automation, render_notes_dry_with_pitch_bend,
    render_notes_dry_with_tempo_and_full_automation, render_notes_with_automation,
    render_notes_with_full_automation, render_notes_with_pitch_bend, render_notes_with_tempo,
};
pub use sample::{SampleOscillator, SampleZone};
pub use sync::OscillatorSync;
use thiserror::Error;
pub use unison::Unison;
pub use voice::VoiceMode;
pub use wav::read_mono_wav;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid synth patch: {0}")]
    InvalidPatch(String),
    #[error("invalid note at index {index}: {message}")]
    InvalidNote { index: usize, message: String },
    #[error("invalid pitch-bend automation: {0}")]
    InvalidAutomation(String),
    #[error("performance exceeds the maximum render length of one hour")]
    PerformanceTooLong,
    #[error("invalid stereo mix: {0}")]
    InvalidMix(String),
    #[error("unknown synth preset: {0}")]
    UnknownPreset(String),
    #[error("could not read synth patch JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid WAV audio: {0}")]
    InvalidWav(String),
    #[error("WAV I/O error: {0}")]
    Wav(#[from] hound::Error),
}
