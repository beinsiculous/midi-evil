mod aiff;
mod aux_envelope;
mod delay;
mod dynamics;
mod effects;
mod envelope;
mod equalizer;
mod filter;
mod insert_automation;
mod lfo;
mod live;
mod mix;
mod modal;
mod modulation;
mod oscillator;
mod patch;
mod presets;
mod render;
mod reverb;
mod sample;
mod sync;
mod unison;
mod voice;
mod wav;
mod waveshaper;
mod wavetable;

pub use live::{LiveControls, LiveSynth};

pub use aiff::read_mono_aiff;
pub use aux_envelope::AuxEnvelope;
pub use delay::{Delay, DelayMode};
pub use dynamics::Compressor;
pub use effects::{
    Bitcrush, INSERT_LIMIT, InsertEffect, SEND_COUNT, SendBus, apply_mono_insert, effect_parameter,
    set_effect_parameter,
};
pub use envelope::Envelope;
pub use equalizer::Equalizer;
pub use filter::{Filter, FilterMode, FilterProcessor};
pub use insert_automation::{
    DELAY_FEEDBACK_CC, DELAY_MIX_CC, DELAY_TIME_CC, WAVESHAPER_CONTOUR_CC, WAVESHAPER_DRIVE_CC,
    WAVESHAPER_LEVEL_CC, controller_is_insert_automation,
};
pub use lfo::{Lfo, LfoDestination, LfoWaveform};
pub use mix::{
    AnalyzedMix, ChannelMeter, METER_BLOCK_FRAMES, MixAnalysis, MonoTrack, StereoSound,
    StereoTrack, SynthMixerAutomation, mix_stereo, mix_stereo_analyzed, mix_stereo_with_sends,
    mix_stereo_with_sends_and_tempo, mix_stereo_with_sends_and_tempo_analyzed,
    mix_stereo_with_sends_and_tempo_analyzed_range, mix_stereo_with_tracks,
};
pub use modal::ModalPartial;
pub use modulation::{Chorus, Flanger};
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
pub use reverb::Reverb;
pub use sample::{SampleOscillator, SampleZone};
pub use sync::OscillatorSync;
use thiserror::Error;
pub use unison::Unison;
pub use voice::VoiceMode;
pub use wav::read_mono_wav;
pub use waveshaper::Waveshaper;

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
    #[error("invalid AIFF audio: {0}")]
    InvalidAiff(String),
    #[error("WAV I/O error: {0}")]
    Wav(#[from] hound::Error),
}
