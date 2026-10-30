use std::collections::HashMap;

use crate::delay::apply_delay;
use crate::filter::FilterState;
use crate::{Error, Lfo, LfoDestination, LfoWaveform, Patch, Waveform};

const MAX_PERFORMANCE_SECONDS: f64 = 3_600.0;

pub struct RenderedSound {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SynthNote {
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct SynthPitchBend {
    pub time_seconds: f64,
    pub value: i16,
}

#[derive(Debug, Clone, Copy)]
pub struct SynthAftertouch {
    pub time_seconds: f64,
    pub note: Option<u8>,
    pub value: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct SynthController {
    pub time_seconds: f64,
    pub controller: u8,
    pub value: u8,
}

#[derive(Clone, Copy)]
struct VoiceAutomation<'a> {
    note_start_seconds: f64,
    note_pitch: u8,
    pitch_bends: &'a [SynthPitchBend],
    aftertouch: &'a [SynthAftertouch],
    controllers: &'a [SynthController],
}

pub fn render(patch: &Patch) -> Result<RenderedSound, Error> {
    patch.validate()?;
    let mut samples = vec![0.0; voice_frame_count(patch)];
    render_voice_into(
        patch,
        0,
        &mut samples,
        VoiceAutomation {
            note_start_seconds: 0.0,
            note_pitch: 69,
            pitch_bends: &[],
            aftertouch: &[],
            controllers: &[],
        },
    );
    apply_delay(patch, &mut samples);
    soft_limit(&mut samples);
    if patch.sync.is_some() {
        block_dc(&mut samples, patch.sample_rate);
    }
    Ok(RenderedSound {
        samples,
        sample_rate: patch.sample_rate,
    })
}

/// Render overlapping notes as independent synth voices and mix them.
/// Oscillator frequencies are designed around A4 (MIDI note 69) when
/// `pitch_tracking` is enabled.
pub fn render_notes(patch: &Patch, notes: &[SynthNote]) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, &[], &[], &[], true)
}

/// Render notes without insert effects so a stereo mixer can apply them per track.
pub fn render_notes_dry(patch: &Patch, notes: &[SynthNote]) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, &[], &[], &[], false)
}

pub fn render_notes_with_pitch_bend(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, &[], &[], true)
}

pub fn render_notes_dry_with_pitch_bend(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, &[], &[], false)
}

pub fn render_notes_with_automation(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
    aftertouch: &[SynthAftertouch],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, aftertouch, &[], true)
}

pub fn render_notes_dry_with_automation(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
    aftertouch: &[SynthAftertouch],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, aftertouch, &[], false)
}

pub fn render_notes_with_full_automation(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
    aftertouch: &[SynthAftertouch],
    controllers: &[SynthController],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, aftertouch, controllers, true)
}

pub fn render_notes_dry_with_full_automation(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
    aftertouch: &[SynthAftertouch],
    controllers: &[SynthController],
) -> Result<RenderedSound, Error> {
    render_notes_inner(patch, notes, pitch_bends, aftertouch, controllers, false)
}

fn render_notes_inner(
    patch: &Patch,
    notes: &[SynthNote],
    pitch_bends: &[SynthPitchBend],
    aftertouch: &[SynthAftertouch],
    controllers: &[SynthController],
    apply_insert_effects: bool,
) -> Result<RenderedSound, Error> {
    patch.validate()?;
    validate_pitch_bends(pitch_bends)?;
    validate_aftertouch(aftertouch)?;
    validate_controllers(controllers)?;
    let sample_rate = patch.sample_rate;
    let cacheable = pitch_bends.is_empty()
        && aftertouch.is_empty()
        && controllers.is_empty()
        && patch
            .oscillators
            .iter()
            .all(|oscillator| !matches!(oscillator.waveform, Waveform::Noise));
    let mut cache = HashMap::new();
    let mut samples = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        validate_note(note, index)?;

        let mut voice = patch.clone();
        voice.duration_seconds = note.duration_seconds as f32;
        voice.gain *= f32::from(note.velocity) / 127.0;
        voice.seed ^= (index as u32).wrapping_mul(0x9e37_79b9);
        let pitch_ratio = 2.0_f32.powf((f32::from(note.pitch) - 69.0) / 12.0);
        let filter_pitch_ratio = 2.0_f32.powf((f32::from(note.pitch) - 60.0) / 12.0);
        if patch.pitch_tracking {
            for oscillator in &mut voice.oscillators {
                if !matches!(oscillator.waveform, Waveform::Noise) {
                    oscillator.start_frequency_hz *= pitch_ratio;
                    oscillator.end_frequency_hz *= pitch_ratio;
                }
            }
        }
        if let Some(filter) = &mut voice.filter
            && filter.key_tracking
        {
            filter.cutoff_hz =
                (filter.cutoff_hz * filter_pitch_ratio).min(sample_rate as f32 * 0.49);
        }
        voice.validate().map_err(|error| Error::InvalidNote {
            index,
            message: error.to_string(),
        })?;
        let end_seconds = note.start_seconds + note.duration_seconds;
        let required_frames = ((end_seconds + f64::from(patch.envelope.release_seconds))
            * f64::from(sample_rate))
        .round() as usize;
        if samples.len() < required_frames {
            samples.resize(required_frames, 0.0);
        }
        let start_frame = (note.start_seconds * f64::from(sample_rate)).round() as usize;
        let end_frame = start_frame + voice_frame_count(&voice);
        if samples.len() < end_frame {
            samples.resize(end_frame, 0.0);
        }
        if cacheable && cache.len() < 128 {
            let key = (note.pitch, voice_frame_count(&voice) as u32, note.velocity);
            let rendered = cache.entry(key).or_insert_with(|| {
                let mut rendered = vec![0.0; voice_frame_count(&voice)];
                render_voice_into(
                    &voice,
                    0,
                    &mut rendered,
                    VoiceAutomation {
                        note_start_seconds: 0.0,
                        note_pitch: note.pitch,
                        pitch_bends: &[],
                        aftertouch: &[],
                        controllers: &[],
                    },
                );
                rendered
            });
            for (offset, sample) in rendered.iter().enumerate() {
                samples[start_frame + offset] += sample;
            }
        } else {
            render_voice_into(
                &voice,
                start_frame,
                &mut samples,
                VoiceAutomation {
                    note_start_seconds: note.start_seconds,
                    note_pitch: note.pitch,
                    pitch_bends,
                    aftertouch,
                    controllers,
                },
            );
        }
    }
    if apply_insert_effects {
        apply_delay(patch, &mut samples);
    }
    soft_limit(&mut samples);
    if patch.sync.is_some() {
        block_dc(&mut samples, sample_rate);
    }
    Ok(RenderedSound {
        samples,
        sample_rate,
    })
}

fn validate_pitch_bends(pitch_bends: &[SynthPitchBend]) -> Result<(), Error> {
    let mut previous_time = 0.0;
    for bend in pitch_bends {
        if !bend.time_seconds.is_finite()
            || bend.time_seconds < previous_time
            || !(-8192..=8191).contains(&bend.value)
        {
            return Err(Error::InvalidAutomation(
                "events need ordered nonnegative times and signed 14-bit values".to_owned(),
            ));
        }
        previous_time = bend.time_seconds;
    }
    Ok(())
}

fn pitch_bend_at(pitch_bends: &[SynthPitchBend], time_seconds: f64) -> i16 {
    let index = pitch_bends.partition_point(|bend| bend.time_seconds <= time_seconds);
    index
        .checked_sub(1)
        .map_or(0, |index| pitch_bends[index].value)
}

fn validate_aftertouch(aftertouch: &[SynthAftertouch]) -> Result<(), Error> {
    let mut previous_time = 0.0;
    for pressure in aftertouch {
        if !pressure.time_seconds.is_finite()
            || pressure.time_seconds < previous_time
            || pressure.value > 127
            || pressure.note.is_some_and(|note| note > 127)
        {
            return Err(Error::InvalidAutomation(
                "aftertouch needs ordered times and 7-bit notes/values".to_owned(),
            ));
        }
        previous_time = pressure.time_seconds;
    }
    Ok(())
}

fn aftertouch_gain(aftertouch: &[SynthAftertouch], pitch: u8, time_seconds: f64) -> f32 {
    let index = aftertouch.partition_point(|pressure| pressure.time_seconds <= time_seconds);
    aftertouch[..index]
        .iter()
        .rev()
        .find(|pressure| pressure.note.is_none_or(|note| note == pitch))
        .map_or(1.0, |pressure| f32::from(pressure.value) / 127.0)
}

fn validate_controllers(controllers: &[SynthController]) -> Result<(), Error> {
    let mut previous_time = 0.0;
    for event in controllers {
        if !event.time_seconds.is_finite()
            || event.time_seconds < previous_time
            || event.controller > 127
            || event.value > 127
        {
            return Err(Error::InvalidAutomation(
                "controllers need ordered times and 7-bit numbers/values".to_owned(),
            ));
        }
        previous_time = event.time_seconds;
    }
    Ok(())
}

fn modulation_depth(controllers: &[SynthController], time_seconds: f64) -> f32 {
    if controllers.iter().any(|event| event.controller == 1) {
        controller_gain(controllers, 1, time_seconds, 0.0)
    } else {
        1.0
    }
}

fn expression_gain(controllers: &[SynthController], time_seconds: f64) -> f32 {
    if controllers.iter().any(|event| event.controller == 11) {
        controller_gain(controllers, 11, time_seconds, 1.0)
    } else {
        1.0
    }
}

fn controller_gain(
    controllers: &[SynthController],
    controller: u8,
    time_seconds: f64,
    default: f32,
) -> f32 {
    let index = controllers.partition_point(|event| event.time_seconds <= time_seconds);
    controllers[..index]
        .iter()
        .rev()
        .find(|event| event.controller == controller)
        .map_or(default, |event| f32::from(event.value) / 127.0)
}

fn validate_note(note: &SynthNote, index: usize) -> Result<(), Error> {
    if !note.start_seconds.is_finite() || note.start_seconds < 0.0 {
        return Err(invalid_note(
            index,
            "start time must be finite and nonnegative",
        ));
    }
    if !note.duration_seconds.is_finite() || note.duration_seconds <= 0.0 {
        return Err(invalid_note(index, "duration must be finite and positive"));
    }
    if note.pitch > 127 || note.velocity == 0 || note.velocity > 127 {
        return Err(invalid_note(
            index,
            "pitch must be 0..=127 and velocity must be 1..=127",
        ));
    }
    if note.start_seconds + note.duration_seconds > MAX_PERFORMANCE_SECONDS {
        return Err(Error::PerformanceTooLong);
    }
    Ok(())
}

fn invalid_note(index: usize, message: &str) -> Error {
    Error::InvalidNote {
        index,
        message: message.to_owned(),
    }
}

fn voice_frame_count(patch: &Patch) -> usize {
    ((patch.duration_seconds + patch.envelope.release_seconds) * patch.sample_rate as f32).round()
        as usize
}

#[derive(Default)]
struct Modulation {
    pitch_cents: f32,
    pulse_width: f32,
    sync_ratio: f32,
    filter_cutoff_octaves: f32,
    amplitude: f32,
}

struct ModulationState {
    lfo_phases: Vec<f32>,
}

impl ModulationState {
    fn new(patch: &Patch) -> Self {
        Self {
            lfo_phases: vec![0.0; patch.lfo_count()],
        }
    }

    fn next(&mut self, patch: &Patch, sample_rate: u32, depth: f32) -> Modulation {
        let mut modulation = Modulation::default();
        let mut rate_multipliers = vec![1.0; patch.lfo_count()];
        for (index, lfo) in patch.lfos().enumerate() {
            if lfo.destination == LfoDestination::LfoRate
                && let Some(target) = lfo.target_lfo
            {
                let value = lfo_value(lfo, self.lfo_phases[index]);
                rate_multipliers[target] += depth * lfo.lfo_rate_depth * value;
            }
        }
        for (index, lfo) in patch.lfos().enumerate() {
            let value = lfo_value(lfo, self.lfo_phases[index]);
            match lfo.destination {
                LfoDestination::Pitch => modulation.pitch_cents += lfo.depth_cents * value,
                LfoDestination::PulseWidth => {
                    modulation.pulse_width += lfo.pulse_width_depth * value;
                }
                LfoDestination::SyncRatio => {
                    modulation.sync_ratio += lfo.sync_ratio_depth * value;
                }
                LfoDestination::FilterCutoff => {
                    modulation.filter_cutoff_octaves += lfo.filter_cutoff_depth * value;
                }
                LfoDestination::Amplitude => {
                    modulation.amplitude += lfo.amplitude_depth * value;
                }
                LfoDestination::LfoRate => {}
            }
            let rate = (lfo.rate_hz * rate_multipliers[index]).max(0.0);
            self.lfo_phases[index] = (self.lfo_phases[index] + rate / sample_rate as f32).fract();
        }
        modulation.pitch_cents *= depth;
        modulation.pulse_width *= depth;
        modulation.sync_ratio *= depth;
        modulation.filter_cutoff_octaves *= depth;
        modulation.amplitude *= depth;
        modulation
    }
}

fn lfo_value(lfo: &Lfo, phase: f32) -> f32 {
    match lfo.waveform {
        LfoWaveform::Sine => (2.0 * std::f32::consts::PI * phase).sin(),
        LfoWaveform::SawDown => 1.0 - 2.0 * phase,
        LfoWaveform::SawUp => 2.0 * phase - 1.0,
        LfoWaveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        LfoWaveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

#[derive(Default)]
struct DcBlocker {
    previous_input: f32,
    previous_output: f32,
}

impl DcBlocker {
    fn process(&mut self, input: f32, sample_rate: u32) -> f32 {
        let coefficient = 1.0 - 2.0 * std::f32::consts::PI * 10.0 / sample_rate as f32;
        let output = input - self.previous_input + coefficient * self.previous_output;
        self.previous_input = input;
        self.previous_output = output;
        output
    }
}

pub(crate) fn block_dc(samples: &mut [f32], sample_rate: u32) {
    let mut blocker = DcBlocker::default();
    for sample in samples {
        *sample = blocker.process(*sample, sample_rate);
    }
}

fn render_voice_into(
    patch: &Patch,
    start_frame: usize,
    samples: &mut [f32],
    automation: VoiceAutomation<'_>,
) {
    let sample_rate = patch.sample_rate;
    let frame_count = voice_frame_count(patch);
    let unison_count = patch
        .unison
        .as_ref()
        .map_or(1, |unison| unison.voices as usize);
    let mut unison_voices: Vec<Vec<_>> = (0..unison_count)
        .map(|voice_index| {
            patch
                .oscillators
                .iter()
                .enumerate()
                .map(|(index, oscillator)| {
                    let seed = patch
                        .seed
                        .wrapping_add((voice_index as u32).wrapping_mul(0x9e37_79b9))
                        .wrapping_add(index as u32);
                    (oscillator, oscillator.state(seed))
                })
                .collect()
        })
        .collect();
    let mut filter_state = FilterState::default();
    let mut modulation_state = ModulationState::new(patch);
    let mut sync_phase = 0.0_f32;

    for frame in 0..frame_count {
        let time = frame as f32 / sample_rate as f32;
        let sweep_progress = (time / patch.duration_seconds).min(1.0);
        let envelope = patch.envelope.amplitude(time, patch.duration_seconds);
        let absolute_time = automation.note_start_seconds + f64::from(time);
        let modulation = modulation_state.next(
            patch,
            sample_rate,
            modulation_depth(automation.controllers, absolute_time),
        );
        let bend_semitones = f32::from(pitch_bend_at(automation.pitch_bends, absolute_time))
            * patch.pitch_bend_range_semitones
            / 8192.0;
        let pitch_multiplier =
            2.0_f32.powf((modulation.pitch_cents + bend_semitones * 100.0) / 1200.0);
        let pulse_width_offset = modulation.pulse_width;
        let sync_ratio = patch
            .sync
            .as_ref()
            .map(|sync| (sync.ratio + modulation.sync_ratio).clamp(0.01, 16.0));
        let oscillator_multiplier =
            sync_ratio.map_or(pitch_multiplier, |ratio| ratio * pitch_multiplier);
        let sync_step = patch.sync.as_ref().map(|_| {
            unison_voices[0][0]
                .0
                .frequency(sweep_progress, pitch_multiplier)
                / sample_rate as f32
        });
        let mut sample = 0.0;
        for (voice_index, oscillators) in unison_voices.iter_mut().enumerate() {
            let detune_cents = patch.unison.as_ref().map_or(0.0, |unison| {
                (voice_index as f32 - (unison_count - 1) as f32 / 2.0) * unison.detune_cents
            });
            let voice_multiplier = oscillator_multiplier * 2.0_f32.powf(detune_cents / 1200.0);
            sample += oscillators
                .iter_mut()
                .map(|(oscillator, state)| {
                    oscillator.sample(
                        state,
                        sweep_progress,
                        sample_rate,
                        voice_multiplier,
                        pulse_width_offset,
                    )
                })
                .sum::<f32>();
        }
        sample /= unison_count as f32;
        if let Some(step) = sync_step {
            sync_phase = (sync_phase + step).fract();
            if sync_phase < step {
                for oscillators in &mut unison_voices {
                    for (_, state) in oscillators {
                        state.reset_phase();
                    }
                }
            }
        }
        if let Some(filter) = &patch.filter {
            sample = filter.process(
                sample,
                &mut filter_state,
                sample_rate,
                envelope,
                modulation.filter_cutoff_octaves
                    + if filter.key_tracking {
                        bend_semitones / 12.0
                    } else {
                        0.0
                    },
            );
        }
        let amplitude = (1.0 + modulation.amplitude).clamp(0.0, 1.0);
        let pressure = aftertouch_gain(automation.aftertouch, automation.note_pitch, absolute_time);
        let expression = expression_gain(automation.controllers, absolute_time);
        sample *= envelope * patch.gain * amplitude * pressure * expression;
        samples[start_frame + frame] += sample;
    }
}

fn soft_limit(samples: &mut [f32]) {
    for sample in samples {
        *sample = sample.tanh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Filter, Lfo, LfoDestination, LfoWaveform, Oscillator, OscillatorSync, PRESET_NAMES, Unison,
        Waveform, preset,
    };

    #[test]
    fn every_preset_renders_finite_bounded_audio() {
        for name in PRESET_NAMES {
            let patch = preset(name).unwrap();
            let sound = render(&patch).unwrap();
            assert!(sound.samples.len() >= voice_frame_count(&patch));
            assert_eq!(sound.sample_rate, 48_000);
            assert!(sound.samples.iter().all(|sample| sample.is_finite()));
            assert!(sound.samples.iter().all(|sample| sample.abs() <= 1.0));
            assert!(sound.samples.iter().any(|sample| sample.abs() > 0.001));
        }
    }

    #[test]
    fn seeded_noise_is_reproducible() {
        let patch = preset("explosion").unwrap();
        assert_eq!(
            render(&patch).unwrap().samples,
            render(&patch).unwrap().samples
        );
    }

    #[test]
    fn same_pitch_notes_are_independent_voices() {
        let patch = Patch::default();
        let sound = render_notes(
            &patch,
            &[
                SynthNote {
                    start_seconds: 0.0,
                    duration_seconds: 0.25,
                    pitch: 69,
                    velocity: 127,
                },
                SynthNote {
                    start_seconds: 0.1,
                    duration_seconds: 0.25,
                    pitch: 69,
                    velocity: 64,
                },
            ],
        )
        .unwrap();
        assert_eq!(sound.samples.len(), 21_600);
        assert!(sound.samples.iter().all(|sample| sample.is_finite()));
        assert!(sound.samples.iter().all(|sample| sample.abs() <= 1.0));
        assert!(sound.samples.iter().any(|sample| sample.abs() > 0.001));
    }

    #[test]
    fn fixed_pitch_effects_ignore_midi_pitch() {
        let patch = Patch {
            pitch_tracking: false,
            ..Patch::default()
        };
        let low = render_notes(&patch, &[note(40)]).unwrap();
        let high = render_notes(&patch, &[note(100)]).unwrap();
        assert_eq!(low.samples, high.samples);
    }

    #[test]
    fn tracked_instruments_follow_midi_pitch() {
        let patch = Patch::default();
        let low = render_notes(&patch, &[note(40)]).unwrap();
        let high = render_notes(&patch, &[note(100)]).unwrap();
        assert_ne!(low.samples, high.samples);
    }

    #[test]
    fn eden_parameters_change_the_audio() {
        let plain = Patch::default();
        let detuned = Patch {
            oscillators: vec![Oscillator {
                detune_cents: 100.0,
                ..Oscillator::default()
            }],
            ..Patch::default()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&detuned).unwrap().samples
        );

        let narrow = Patch {
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                pulse_width: 0.25,
                ..Oscillator::default()
            }],
            ..Patch::default()
        };
        let wide = Patch {
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                pulse_width: 0.75,
                ..Oscillator::default()
            }],
            ..Patch::default()
        };
        assert_ne!(
            render(&narrow).unwrap().samples,
            render(&wide).unwrap().samples
        );

        let filtered = Patch {
            filter: Some(Filter {
                envelope_amount: 2.0,
                ..Filter::default()
            }),
            ..Patch::default()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&filtered).unwrap().samples
        );

        let modulated = Patch {
            lfo: Some(Lfo {
                rate_hz: 10.0,
                depth_cents: 100.0,
                ..Lfo::default()
            }),
            ..Patch::default()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&modulated).unwrap().samples
        );
    }

    #[test]
    fn pulse_width_lfo_changes_square_wave_audio() {
        let plain = Patch {
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                ..Oscillator::default()
            }],
            ..Patch::default()
        };
        let modulated = Patch {
            lfo: Some(Lfo {
                destination: LfoDestination::PulseWidth,
                waveform: LfoWaveform::SawDown,
                pulse_width_depth: 0.4,
                ..Lfo::default()
            }),
            ..plain.clone()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&modulated).unwrap().samples
        );
    }

    #[test]
    fn oscillator_sync_and_sync_lfo_change_the_audio() {
        let plain = Patch {
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                ..Oscillator::default()
            }],
            ..Patch::default()
        };
        let synced = Patch {
            sync: Some(OscillatorSync::default()),
            ..plain.clone()
        };
        let modulated = Patch {
            lfo: Some(Lfo {
                destination: LfoDestination::SyncRatio,
                waveform: LfoWaveform::SawDown,
                ..Lfo::default()
            }),
            ..synced.clone()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&synced).unwrap().samples
        );
        assert_ne!(
            render(&synced).unwrap().samples,
            render(&modulated).unwrap().samples
        );
    }

    #[test]
    fn synced_oscillators_do_not_accumulate_dc_offset() {
        let patch = Patch {
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                ..Oscillator::default()
            }],
            sync: Some(OscillatorSync::default()),
            ..Patch::default()
        };
        let sound = render(&patch).unwrap();
        let steady = &sound.samples[4_800..12_000];
        let mean = steady.iter().sum::<f32>() / steady.len() as f32;
        assert!(mean.abs() < 0.001, "unexpected DC offset: {mean}");
    }

    #[test]
    fn key_tracked_cutoff_clamps_for_high_notes() {
        let patch = Patch {
            filter: Some(Filter {
                cutoff_hz: 20_000.0,
                key_tracking: true,
                ..Filter::default()
            }),
            ..Patch::default()
        };
        let sound = render_notes(
            &patch,
            &[SynthNote {
                start_seconds: 0.0,
                duration_seconds: 0.1,
                pitch: 96,
                velocity: 100,
            }],
        )
        .unwrap();
        assert!(sound.samples.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn resonance_changes_the_filter_audio_and_stays_stable() {
        let clean = Patch {
            filter: Some(Filter {
                cutoff_hz: 1_000.0,
                stages: 2,
                ..Filter::default()
            }),
            ..Patch::default()
        };
        let resonant = Patch {
            filter: Some(Filter {
                resonance: 0.9,
                ..clean.filter.clone().unwrap()
            }),
            ..clean.clone()
        };
        let clean_audio = render(&clean).unwrap();
        let resonant_audio = render(&resonant).unwrap();
        assert_ne!(clean_audio.samples, resonant_audio.samples);
        assert!(
            resonant_audio
                .samples
                .iter()
                .all(|sample| sample.is_finite())
        );
        assert!(
            resonant_audio
                .samples
                .iter()
                .all(|sample| sample.abs() <= 1.0)
        );
    }

    #[test]
    fn unison_detunes_and_normalizes_multiple_voices() {
        let plain = Patch::default();
        let single = Patch {
            unison: Some(Unison {
                voices: 1,
                detune_cents: 30.0,
            }),
            ..plain.clone()
        };
        let wide = Patch {
            unison: Some(Unison {
                voices: 4,
                detune_cents: 30.0,
            }),
            ..plain.clone()
        };
        assert_eq!(
            render(&plain).unwrap().samples,
            render(&single).unwrap().samples
        );
        let audio = render(&wide).unwrap();
        assert_ne!(render(&plain).unwrap().samples, audio.samples);
        assert!(audio.samples.iter().all(|sample| sample.is_finite()));
        assert!(audio.samples.iter().all(|sample| sample.abs() <= 1.0));
    }

    #[test]
    fn multiple_lfo_lanes_drive_cutoff_and_amplitude() {
        let plain = Patch {
            filter: Some(Filter::default()),
            ..Patch::default()
        };
        let cutoff = Patch {
            lfo: Some(Lfo {
                destination: LfoDestination::FilterCutoff,
                waveform: LfoWaveform::SawUp,
                ..Lfo::default()
            }),
            ..plain.clone()
        };
        assert_ne!(
            render(&plain).unwrap().samples,
            render(&cutoff).unwrap().samples
        );

        let one_lane = Patch {
            lfo: Some(Lfo {
                destination: LfoDestination::Amplitude,
                amplitude_depth: 0.2,
                ..Lfo::default()
            }),
            ..Patch::default()
        };
        let two_lanes = Patch {
            modulation: vec![Lfo {
                destination: LfoDestination::Amplitude,
                amplitude_depth: 0.2,
                ..Lfo::default()
            }],
            ..one_lane.clone()
        };
        assert_ne!(
            render(&Patch::default()).unwrap().samples,
            render(&one_lane).unwrap().samples
        );
        assert_ne!(
            render(&one_lane).unwrap().samples,
            render(&two_lanes).unwrap().samples
        );
    }

    #[test]
    fn lfo_rate_modulation_changes_the_target_lane() {
        let base = Patch {
            sync: Some(OscillatorSync::default()),
            modulation: vec![Lfo {
                destination: LfoDestination::SyncRatio,
                waveform: LfoWaveform::SawDown,
                ..Lfo::default()
            }],
            ..Patch::default()
        };
        let with_meta_lfo = Patch {
            modulation: vec![
                base.modulation[0].clone(),
                Lfo {
                    rate_hz: 20.0,
                    destination: LfoDestination::LfoRate,
                    waveform: LfoWaveform::Square,
                    target_lfo: Some(0),
                    lfo_rate_depth: 1.0,
                    ..Lfo::default()
                },
            ],
            ..base.clone()
        };
        assert_ne!(
            render(&base).unwrap().samples,
            render(&with_meta_lfo).unwrap().samples
        );
    }

    #[test]
    fn filter_key_tracking_is_independent_of_oscillator_pitch_tracking() {
        let patch = Patch {
            pitch_tracking: false,
            filter: Some(Filter {
                cutoff_hz: 500.0,
                key_tracking: true,
                ..Filter::default()
            }),
            ..Patch::default()
        };
        assert_ne!(
            render_notes(&patch, &[note(40)]).unwrap().samples,
            render_notes(&patch, &[note(100)]).unwrap().samples
        );

        let mut middle_c_untracked = patch.clone();
        middle_c_untracked.filter.as_mut().unwrap().key_tracking = false;
        assert_eq!(
            render_notes(&patch, &[note(60)]).unwrap().samples,
            render_notes(&middle_c_untracked, &[note(60)])
                .unwrap()
                .samples
        );
    }

    #[test]
    fn pitch_bend_changes_audio_only_from_its_event_time() {
        let patch = Patch {
            duration_seconds: 0.5,
            pitch_bend_range_semitones: 12.0,
            ..Patch::default()
        };
        let notes = [SynthNote {
            start_seconds: 0.0,
            duration_seconds: 0.5,
            pitch: 69,
            velocity: 100,
        }];
        let bends = [SynthPitchBend {
            time_seconds: 0.25,
            value: 8191,
        }];
        let plain = render_notes(&patch, &notes).unwrap();
        let bent = render_notes_with_pitch_bend(&patch, &notes, &bends).unwrap();
        assert_eq!(plain.samples[..12_000], bent.samples[..12_000]);
        assert_ne!(plain.samples[12_000..], bent.samples[12_000..]);
    }

    #[test]
    fn aftertouch_is_timed_and_scoped_to_its_note_or_channel() {
        let patch = Patch {
            duration_seconds: 0.5,
            ..Patch::default()
        };
        let notes = [SynthNote {
            start_seconds: 0.0,
            duration_seconds: 0.5,
            pitch: 60,
            velocity: 100,
        }];
        let pressure = [SynthAftertouch {
            time_seconds: 0.25,
            note: Some(60),
            value: 0,
        }];
        let plain = render_notes(&patch, &notes).unwrap();
        let pressed = render_notes_with_automation(&patch, &notes, &[], &pressure).unwrap();
        assert_eq!(plain.samples[..12_000], pressed.samples[..12_000]);
        assert_ne!(plain.samples[12_000..], pressed.samples[12_000..]);

        let other_notes = [SynthNote {
            pitch: 62,
            ..notes[0]
        }];
        let other_plain = render_notes(&patch, &other_notes).unwrap();
        let other_pressed =
            render_notes_with_automation(&patch, &other_notes, &[], &pressure).unwrap();
        assert_eq!(other_plain.samples, other_pressed.samples);

        let channel_pressure = [SynthAftertouch {
            note: None,
            ..pressure[0]
        }];
        let channel_pressed =
            render_notes_with_automation(&patch, &other_notes, &[], &channel_pressure).unwrap();
        assert_ne!(other_plain.samples, channel_pressed.samples);
    }

    #[test]
    fn modulation_wheel_and_expression_follow_controller_events() {
        let patch = Patch {
            duration_seconds: 0.5,
            lfo: Some(Lfo {
                destination: LfoDestination::Amplitude,
                amplitude_depth: 0.5,
                ..Lfo::default()
            }),
            ..Patch::default()
        };
        let notes = [SynthNote {
            start_seconds: 0.0,
            duration_seconds: 0.5,
            pitch: 69,
            velocity: 100,
        }];
        let wheel = [
            SynthController {
                time_seconds: 0.0,
                controller: 1,
                value: 127,
            },
            SynthController {
                time_seconds: 0.25,
                controller: 1,
                value: 0,
            },
        ];
        let modulated = render_notes(&patch, &notes).unwrap();
        let unmodulated = render_notes(
            &Patch {
                lfo: None,
                ..patch.clone()
            },
            &notes,
        )
        .unwrap();
        let controlled =
            render_notes_with_full_automation(&patch, &notes, &[], &[], &wheel).unwrap();
        assert_eq!(controlled.samples[..12_000], modulated.samples[..12_000]);
        assert_eq!(controlled.samples[12_000..], unmodulated.samples[12_000..]);

        let expression = [SynthController {
            time_seconds: 0.25,
            controller: 11,
            value: 0,
        }];
        let plain = render_notes(&Patch::default(), &notes).unwrap();
        let expressed =
            render_notes_with_full_automation(&Patch::default(), &notes, &[], &[], &expression)
                .unwrap();
        assert_eq!(plain.samples[..12_000], expressed.samples[..12_000]);
        assert_ne!(plain.samples[12_000..], expressed.samples[12_000..]);
    }

    #[test]
    fn invalid_notes_are_rejected() {
        let mut invalid = note(69);
        invalid.velocity = 0;
        assert!(render_notes(&Patch::default(), &[invalid]).is_err());
    }

    fn note(pitch: u8) -> SynthNote {
        SynthNote {
            start_seconds: 0.0,
            duration_seconds: 0.25,
            pitch,
            velocity: 100,
        }
    }
}
