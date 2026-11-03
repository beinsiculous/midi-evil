//! Bounded, sample-by-sample synthesis for the performance keyboard.
use crate::render::{DcBlocker, ModulationState};
use crate::voice::{VoiceControls, VoiceState};
use crate::{Delay, DelayMode, Error, Filter, Patch, VoiceMode, Waveform};

const MAX_VOICES: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveControls {
    pub bend: f32,
    pub bend_range_semitones: f32,
    pub gain: f32,
    pub pan: f32,
    pub tempo_bpm: f32,
    pub filter: Option<Filter>,
    pub delay: Option<Delay>,
}

impl LiveControls {
    pub fn for_patch(patch: &Patch) -> Self {
        Self {
            bend: 0.0,
            bend_range_semitones: patch.pitch_bend_range_semitones,
            gain: 0.8,
            pan: 0.0,
            tempo_bpm: 120.0,
            filter: patch.filter.clone(),
            delay: patch.delay.clone(),
        }
    }
}

struct Voice {
    pitch: u8,
    glide_from_semitones: f32,
    held: bool,
    patch: Patch,
    state: VoiceState,
    modulation: ModulationState,
    frame: u64,
    gate: Option<f32>,
}

impl Voice {
    fn time(&self) -> f32 {
        self.frame as f32 / self.patch.sample_rate as f32
    }
    fn release(&mut self) {
        if self.gate.is_none() {
            self.gate = Some(self.time());
        }
    }
    fn finished(&self) -> bool {
        self.gate
            .is_some_and(|gate| self.time() >= gate + self.patch.envelope.release_seconds)
    }
}

pub struct LiveSynth {
    patch: Patch,
    controls: LiveControls,
    voices: Vec<Voice>,
    sustain: bool,
    delay_line: Vec<[f32; 2]>,
    delay_position: usize,
    dc: [DcBlocker; 2],
}

impl LiveSynth {
    pub fn new(patch: Patch) -> Result<Self, Error> {
        patch.validate()?;
        Ok(Self {
            controls: LiveControls::for_patch(&patch),
            delay_line: vec![[0.0; 2]; patch.sample_rate as usize * 2 + 1],
            patch,
            voices: Vec::with_capacity(MAX_VOICES),
            sustain: false,
            delay_position: 0,
            dc: [DcBlocker::default(), DcBlocker::default()],
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.patch.sample_rate
    }
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    pub fn set_controls(&mut self, controls: LiveControls) -> Result<(), Error> {
        if !controls.bend.is_finite()
            || !(-1.0..=1.0).contains(&controls.bend)
            || !controls.bend_range_semitones.is_finite()
            || controls.bend_range_semitones <= 0.0
            || controls.bend_range_semitones > 24.0
            || !controls.gain.is_finite()
            || !(0.0..=1.0).contains(&controls.gain)
            || !controls.pan.is_finite()
            || !(-1.0..=1.0).contains(&controls.pan)
            || !controls.tempo_bpm.is_finite()
            || controls.tempo_bpm <= 0.0
        {
            return Err(Error::InvalidPatch(
                "invalid live performance controls".to_owned(),
            ));
        }
        let mut patch = self.patch.clone();
        patch.filter.clone_from(&controls.filter);
        patch.delay.clone_from(&controls.delay);
        patch.validate()?;
        self.patch = patch;
        for voice in &mut self.voices {
            voice.patch.filter =
                pitched_filter(controls.filter.clone(), voice.pitch, self.patch.sample_rate);
        }
        if controls.delay.is_none() && self.controls.delay.is_some() {
            self.delay_line.fill([0.0; 2]);
        }
        self.controls = controls;
        Ok(())
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) -> Result<(), Error> {
        if pitch > 127 || velocity == 0 || velocity > 127 {
            return Err(Error::InvalidNote {
                index: 0,
                message: "pitch must be 0..127 and velocity 1..127".to_owned(),
            });
        }
        let glide_from_semitones = self
            .voices
            .last()
            .map_or(0.0, |voice| f32::from(voice.pitch) - f32::from(pitch));
        let mut patch = self.patch.clone();
        patch.gain *= f32::from(velocity) / 127.0;
        patch.gain = (patch.gain
            * 2.0_f32.powf((60.0 - f32::from(pitch)) / 12.0 * patch.pitch_gain_per_octave))
        .min(1.0);
        if patch.accent_velocity_threshold > 0 && velocity >= patch.accent_velocity_threshold {
            patch.gain = (patch.gain * (1.0 + patch.accent_gain)).min(1.0);
        }
        if patch.pitch_tracking {
            let ratio = 2.0_f32.powf((f32::from(pitch) - 69.0) / 12.0);
            for oscillator in &mut patch.oscillators {
                if oscillator.waveform != Waveform::Noise {
                    // Live playing supports the entire MIDI keyboard, even for bright patches.
                    let limit = patch.sample_rate as f32 * 0.49;
                    oscillator.start_frequency_hz =
                        (oscillator.start_frequency_hz * ratio).min(limit);
                    oscillator.end_frequency_hz = (oscillator.end_frequency_hz * ratio).min(limit);
                }
            }
        }
        patch.filter = pitched_filter(patch.filter, pitch, patch.sample_rate);
        if patch.accent_velocity_threshold > 0
            && velocity >= patch.accent_velocity_threshold
            && let Some(filter) = &mut patch.filter
        {
            filter.cutoff_hz = (filter.cutoff_hz * 2.0_f32.powf(patch.accent_filter_octaves))
                .min(patch.sample_rate as f32 * 0.49);
        }
        match self.patch.voice_mode {
            VoiceMode::Poly => {
                self.voices.retain(|voice| voice.pitch != pitch);
                if self.voices.len() == MAX_VOICES {
                    let oldest_release = self
                        .voices
                        .iter()
                        .position(|voice| !voice.held)
                        .unwrap_or(0);
                    self.voices.remove(oldest_release);
                }
            }
            VoiceMode::Mono => self.voices.clear(),
            VoiceMode::Legato => {
                if let Some(voice) = self.voices.last_mut().filter(|voice| !voice.finished()) {
                    voice.pitch = pitch;
                    voice.glide_from_semitones = glide_from_semitones;
                    voice.held = true;
                    voice.patch = patch;
                    voice.state.select_note(&voice.patch, pitch);
                    voice.gate = None;
                    return Ok(());
                }
                self.voices.clear();
            }
        }
        self.voices.push(Voice {
            pitch,
            glide_from_semitones,
            held: true,
            state: VoiceState::new(&patch, pitch),
            modulation: ModulationState::new(&patch),
            patch,
            frame: 0,
            gate: None,
        });
        Ok(())
    }

    pub fn note_off(&mut self, pitch: u8) {
        for voice in &mut self.voices {
            if voice.pitch == pitch {
                voice.held = false;
                if !self.sustain {
                    voice.release();
                }
            }
        }
    }

    pub fn set_sustain(&mut self, sustain: bool) {
        self.sustain = sustain;
        if !sustain {
            for voice in &mut self.voices {
                if !voice.held {
                    voice.release();
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.voices.clear();
        self.sustain = false;
        self.delay_line.fill([0.0; 2]);
        self.dc = [DcBlocker::default(), DcBlocker::default()];
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        self.voices.retain(|voice| !voice.finished());
        let mut mono = 0.0;
        for voice in &mut self.voices {
            let time = voice.time();
            let modulation = voice.modulation.next(
                &voice.patch,
                self.patch.sample_rate,
                time,
                voice.gate.unwrap_or(f32::INFINITY),
                1.0,
                self.controls.tempo_bpm / 60.0,
            );
            mono += voice.state.sample(
                &voice.patch,
                time,
                voice.gate.unwrap_or(f32::INFINITY),
                VoiceControls {
                    modulation,
                    bend_semitones: self.controls.bend * self.controls.bend_range_semitones,
                    glide_from_semitones: voice.glide_from_semitones,
                    pressure: 1.0,
                    expression: 1.0,
                },
            );
            voice.frame += 1;
        }
        let mono = mono.tanh() * self.controls.gain;
        let angle = (self.controls.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let mut stereo = [mono * angle.cos(), mono * angle.sin()];
        if let Some(delay) = &self.controls.delay {
            let frames = (delay.time_seconds * self.patch.sample_rate as f32)
                .round()
                .max(1.0) as usize;
            let read =
                (self.delay_position + self.delay_line.len() - frames) % self.delay_line.len();
            let delayed = self.delay_line[read];
            self.delay_line[self.delay_position] = if delay.mode == DelayMode::StereoCross {
                [
                    stereo[1] + delayed[1] * delay.feedback,
                    stereo[0] + delayed[0] * delay.feedback,
                ]
            } else {
                [
                    stereo[0] + delayed[0] * delay.feedback,
                    stereo[1] + delayed[1] * delay.feedback,
                ]
            };
            stereo[0] += delayed[0] * delay.mix;
            stereo[1] += delayed[1] * delay.mix;
            self.delay_position = (self.delay_position + 1) % self.delay_line.len();
        }
        for (sample, dc) in stereo.iter_mut().zip(&mut self.dc) {
            *sample = sample.tanh();
            if self.patch.uses_sync() {
                *sample = dc.process(*sample, self.patch.sample_rate);
            }
        }
        stereo
    }
}

fn pitched_filter(filter: Option<Filter>, pitch: u8, sample_rate: u32) -> Option<Filter> {
    filter.map(|mut filter| {
        if filter.key_tracking {
            filter.cutoff_hz = (filter.cutoff_hz * 2.0_f32.powf((f32::from(pitch) - 60.0) / 12.0))
                .min(sample_rate as f32 * 0.49);
        }
        filter
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_notes_sustain_release_and_panic_clears_delay() {
        let mut synth = LiveSynth::new(crate::preset("lead").unwrap()).unwrap();
        synth.note_on(60, 100).unwrap();
        for _ in 0..48_000 {
            synth.next_frame();
        }
        assert_eq!(synth.voice_count(), 1);
        synth.set_sustain(true);
        synth.note_off(60);
        for _ in 0..48_000 {
            synth.next_frame();
        }
        assert_eq!(synth.voice_count(), 1);
        synth.set_sustain(false);
        for _ in 0..48_000 {
            synth.next_frame();
        }
        assert_eq!(synth.voice_count(), 0);
        synth.note_on(64, 100).unwrap();
        for _ in 0..10_000 {
            synth.next_frame();
        }
        synth.all_notes_off();
        assert!((0..1000).all(|_| synth.next_frame() == [0.0; 2]));
    }

    #[test]
    fn live_voice_matches_the_shared_offline_synth_while_held() {
        let patch = crate::preset("lead").unwrap();
        let dry = crate::render_notes_dry(
            &patch,
            &[crate::SynthNote {
                pitch: 60,
                velocity: 100,
                start_seconds: 0.0,
                duration_seconds: f64::from(patch.duration_seconds),
            }],
        )
        .unwrap();
        let mut live = LiveSynth::new(patch).unwrap();
        live.note_on(60, 100).unwrap();
        for expected in dry.samples.iter().take(8_000) {
            let expected = (expected * 0.8 * std::f32::consts::FRAC_1_SQRT_2).tanh();
            assert!((live.next_frame()[0] - expected).abs() < 0.00001);
        }
    }

    #[test]
    fn oscillator_modes_match_offline_at_multiple_pitches() {
        for mode in [
            crate::OscillatorMode::Mix,
            crate::OscillatorMode::Sync,
            crate::OscillatorMode::Ring,
        ] {
            for pitch in [48, 60, 72] {
                let patch = Patch {
                    duration_seconds: 0.08,
                    oscillator_mode: mode,
                    sync: (mode == crate::OscillatorMode::Sync).then(Default::default),
                    oscillators: vec![
                        crate::Oscillator {
                            waveform: Waveform::Saw,
                            amplitude: 0.4,
                            ..crate::Oscillator::default()
                        },
                        crate::Oscillator {
                            waveform: Waveform::Square,
                            amplitude: 0.2,
                            detune_cents: 100.0,
                            ..crate::Oscillator::default()
                        },
                    ],
                    envelope: crate::Envelope {
                        attack_seconds: 0.0,
                        decay_seconds: 0.0,
                        sustain_level: 1.0,
                        release_seconds: 0.0,
                    },
                    ..Patch::default()
                };
                assert_offline_matches_live(patch, &[test_note(pitch, 0.0, 0.08)]);
            }
        }
    }

    #[test]
    fn independent_filter_envelope_and_slopes_match_offline_at_multiple_pitches() {
        for mode in [
            crate::FilterMode::LowPass,
            crate::FilterMode::BandPass,
            crate::FilterMode::HighPass,
        ] {
            for stages in [2, 4] {
                for resonance in [0.0, 0.4] {
                    for pitch in [48, 60, 72] {
                        let patch = Patch {
                            duration_seconds: 0.1,
                            envelope: crate::Envelope {
                                attack_seconds: 0.002,
                                decay_seconds: 0.04,
                                sustain_level: 0.6,
                                release_seconds: 0.03,
                            },
                            filter: Some(Filter {
                                mode,
                                cutoff_hz: 500.0,
                                envelope_amount: 3.0,
                                resonance,
                                stages,
                                key_tracking: true,
                            }),
                            filter_envelope: Some(crate::Envelope {
                                attack_seconds: 0.01,
                                decay_seconds: 0.02,
                                sustain_level: 0.2,
                                release_seconds: 0.03,
                            }),
                            ..Patch::default()
                        };
                        assert_offline_matches_live(patch, &[test_note(pitch, 0.0, 0.1)]);
                    }
                }
            }
        }
    }

    #[test]
    fn both_auxiliary_matrix_slots_match_offline_at_multiple_pitches() {
        for second in [false, true] {
            for route in 0..6 {
                for sign in [-1.0, 1.0] {
                    for pitch in [48, 60, 72] {
                        let mut aux = crate::AuxEnvelope {
                            envelope: crate::Envelope {
                                attack_seconds: 0.015,
                                decay_seconds: 0.035,
                                sustain_level: 0.3,
                                release_seconds: 0.03,
                            },
                            ..crate::AuxEnvelope::default()
                        };
                        match route {
                            0 => aux.amplitude_depth = sign * 0.6,
                            1 => aux.pitch_depth_cents = sign * 600.0,
                            2 => aux.filter_cutoff_depth_octaves = sign * 3.0,
                            3 => aux.pulse_width_depth = sign * 0.4,
                            4 => aux.sync_ratio_depth = sign * 0.8,
                            5 => {
                                aux.target_lfo = Some(0);
                                aux.lfo_rate_depth = sign;
                            }
                            _ => unreachable!(),
                        }
                        let mut patch = auxiliary_test_patch();
                        if matches!(route, 3 | 4) {
                            patch.oscillators[0].waveform = Waveform::Square;
                        }
                        if route == 4 {
                            patch.sync = Some(crate::OscillatorSync { ratio: 2.0 });
                        }
                        if second {
                            patch.aux_envelope_2 = Some(aux);
                        } else {
                            patch.aux_envelope = Some(aux);
                        }
                        assert_offline_matches_live(patch, &[test_note(pitch, 0.0, 0.12)]);
                    }
                }
            }
        }
    }

    #[test]
    fn combined_auxiliary_envelopes_match_offline_for_voice_modes_and_pitches() {
        let first = crate::AuxEnvelope {
            envelope: crate::Envelope {
                attack_seconds: 0.01,
                decay_seconds: 0.04,
                sustain_level: 0.2,
                release_seconds: 0.03,
            },
            amplitude_depth: -0.25,
            pitch_depth_cents: -350.0,
            pulse_width_depth: -0.2,
            sync_ratio_depth: -0.5,
            filter_cutoff_depth_octaves: -1.5,
            target_lfo: Some(0),
            lfo_rate_depth: -0.75,
        };
        let second = crate::AuxEnvelope {
            envelope: crate::Envelope {
                attack_seconds: 0.03,
                decay_seconds: 0.02,
                sustain_level: 0.7,
                release_seconds: 0.02,
            },
            amplitude_depth: 0.2,
            pitch_depth_cents: 500.0,
            pulse_width_depth: 0.35,
            sync_ratio_depth: 0.8,
            filter_cutoff_depth_octaves: 2.0,
            target_lfo: Some(1),
            lfo_rate_depth: 1.25,
        };
        for mode in [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato] {
            for pitch in [48, 60, 72] {
                let mut patch = Patch {
                    voice_mode: mode,
                    glide_seconds: 0.025,
                    sync: Some(crate::OscillatorSync { ratio: 2.0 }),
                    aux_envelope: Some(first.clone()),
                    aux_envelope_2: Some(second.clone()),
                    ..auxiliary_test_patch()
                };
                patch.oscillators[0].waveform = Waveform::Square;
                let notes = [
                    test_note(pitch, 0.0, 0.1),
                    test_note(pitch + 4, 0.05, 0.1),
                    test_note(pitch + 12, 0.1, 0.08),
                ];
                assert_offline_matches_live(patch, &notes);
            }
        }
    }

    #[test]
    fn custom_sample_loop_points_match_offline_at_multiple_pitches() {
        for loop_enabled in [false, true] {
            for pitch in [48, 60, 72] {
                let patch = Patch {
                    sample_rate: 8_000,
                    duration_seconds: 0.01,
                    oscillators: vec![crate::Oscillator {
                        waveform: Waveform::Sample,
                        sample: Some(crate::SampleOscillator {
                            samples: (0..32)
                                .map(|index| (index % 9) as f32 / 8.0 - 0.5)
                                .collect(),
                            sample_rate: 8_000,
                            loop_enabled,
                            loop_start_frame: 5,
                            loop_end_frame: Some(7),
                            ..crate::SampleOscillator::default()
                        }),
                        ..crate::Oscillator::default()
                    }],
                    envelope: crate::Envelope {
                        attack_seconds: 0.0,
                        decay_seconds: 0.0,
                        sustain_level: 1.0,
                        release_seconds: 0.002,
                    },
                    ..Patch::default()
                };
                assert_offline_matches_live(patch, &[test_note(pitch, 0.0, 0.01)]);
            }
        }
    }

    #[test]
    fn sample_key_zones_match_offline_for_fixed_and_tracked_pitch() {
        for loop_enabled in [false, true] {
            for pitch_tracking in [false, true] {
                for pitch in [48, 59, 60, 72] {
                    let patch = zoned_sample_test_patch(loop_enabled, pitch_tracking);
                    assert_offline_matches_live(patch, &[test_note(pitch, 0.0, 0.01)]);
                }
            }
        }
    }

    #[test]
    fn keyboard_sliced_samples_match_offline_at_multiple_pitches() {
        for loop_enabled in [false, true] {
            for pitch_tracking in [false, true] {
                let source = crate::SampleOscillator {
                    samples: (0..35)
                        .map(|index| (index % 11) as f32 / 10.0 - 0.5)
                        .collect(),
                    sample_rate: 8_000,
                    root_frequency_hz: 220.0,
                    loop_enabled,
                    ..crate::SampleOscillator::default()
                };
                let mut zones = source.keyboard_slices(48, 52, 3).unwrap();
                let middle = &mut zones[1].sample;
                middle.loop_start_frame += 2;
                middle.loop_end_frame = Some(middle.playback_range().end - 1);
                let patch = Patch {
                    sample_rate: 8_000,
                    duration_seconds: 0.02,
                    pitch_tracking,
                    oscillators: vec![crate::Oscillator {
                        waveform: Waveform::Sample,
                        sample_zones: zones,
                        ..crate::Oscillator::default()
                    }],
                    envelope: crate::Envelope {
                        attack_seconds: 0.0,
                        decay_seconds: 0.0,
                        sustain_level: 1.0,
                        release_seconds: 0.002,
                    },
                    ..Patch::default()
                };
                for pitch in [47, 48, 49, 50, 51, 52, 53] {
                    assert_offline_matches_live(patch.clone(), &[test_note(pitch, 0.0, 0.02)]);
                }

                let legato = Patch {
                    voice_mode: VoiceMode::Legato,
                    ..patch
                };
                assert_offline_matches_live(
                    legato,
                    &[
                        test_note(48, 0.0, 0.01),
                        test_note(52, 0.01, 0.01),
                        test_note(50, 0.02, 0.01),
                    ],
                );
            }
        }
    }

    #[test]
    fn legato_sample_zones_reselect_only_at_note_on() {
        let notes = [
            test_note(48, 0.0, 0.01),
            test_note(72, 0.01, 0.01),
            test_note(59, 0.02, 0.01),
        ];
        for pitch_tracking in [false, true] {
            let patch = Patch {
                voice_mode: VoiceMode::Legato,
                ..zoned_sample_test_patch(true, pitch_tracking)
            };
            assert_offline_matches_live(patch, &notes);
        }
    }

    #[test]
    fn live_pitch_modulation_does_not_change_the_selected_sample_zone() {
        let zoned = Patch {
            pitch_bend_range_semitones: 24.0,
            lfo: Some(crate::Lfo {
                rate_hz: 31.0,
                depth_cents: -2_400.0,
                waveform: crate::LfoWaveform::SawDown,
                ..crate::Lfo::default()
            }),
            ..zoned_sample_test_patch(true, true)
        };
        let high = zoned.oscillators[0].sample_for_key(72).unwrap().clone();
        let single = Patch {
            oscillators: vec![crate::Oscillator {
                sample: Some(high),
                sample_zones: Vec::new(),
                ..zoned.oscillators[0].clone()
            }],
            ..zoned.clone()
        };
        let mut zoned_live = LiveSynth::new(zoned).unwrap();
        let mut single_live = LiveSynth::new(single).unwrap();
        for synth in [&mut zoned_live, &mut single_live] {
            let mut controls = LiveControls::for_patch(&synth.patch);
            controls.bend = -1.0;
            controls.bend_range_semitones = 24.0;
            synth.set_controls(controls).unwrap();
            synth.note_on(72, 100).unwrap();
        }
        for _ in 0..160 {
            assert_eq!(zoned_live.next_frame(), single_live.next_frame());
        }
    }

    #[test]
    fn voice_modes_match_offline_across_overlapping_pitches() {
        let notes = [
            test_note(48, 0.0, 0.12),
            test_note(60, 0.06, 0.12),
            test_note(72, 0.12, 0.08),
        ];
        let mut outputs = Vec::new();
        for mode in [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato] {
            let patch = Patch {
                voice_mode: mode,
                envelope: crate::Envelope {
                    attack_seconds: 0.01,
                    decay_seconds: 0.03,
                    sustain_level: 0.7,
                    release_seconds: 0.04,
                },
                ..Patch::default()
            };
            assert_offline_matches_live(patch, &notes);
            outputs.push(
                crate::render_notes_dry(
                    &Patch {
                        voice_mode: mode,
                        envelope: crate::Envelope {
                            attack_seconds: 0.01,
                            decay_seconds: 0.03,
                            sustain_level: 0.7,
                            release_seconds: 0.04,
                        },
                        ..Patch::default()
                    },
                    &notes,
                )
                .unwrap()
                .samples,
            );
        }
        assert_ne!(outputs[0], outputs[1]);
        assert_ne!(outputs[0], outputs[2]);
        assert_ne!(outputs[1], outputs[2]);
    }

    #[test]
    fn mono_retriggers_and_legato_preserves_voice_time() {
        for mode in [VoiceMode::Mono, VoiceMode::Legato] {
            let mut synth = LiveSynth::new(Patch {
                voice_mode: mode,
                ..Patch::default()
            })
            .unwrap();
            synth.note_on(48, 100).unwrap();
            for _ in 0..100 {
                synth.next_frame();
            }
            synth.note_on(60, 100).unwrap();
            assert_eq!(synth.voice_count(), 1);
            let expected_frame = if mode == VoiceMode::Legato { 100 } else { 0 };
            assert_eq!(synth.voices[0].frame, expected_frame);
            synth.note_on(72, 100).unwrap();
            assert_eq!(synth.voices[0].frame, expected_frame);
        }
    }

    fn auxiliary_test_patch() -> Patch {
        Patch {
            duration_seconds: 0.12,
            envelope: crate::Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 0.9,
                release_seconds: 0.03,
            },
            filter: Some(Filter {
                cutoff_hz: 600.0,
                envelope_amount: 1.5,
                resonance: 0.25,
                stages: 2,
                key_tracking: true,
                ..Filter::default()
            }),
            filter_envelope: Some(crate::Envelope {
                attack_seconds: 0.01,
                decay_seconds: 0.03,
                sustain_level: 0.4,
                release_seconds: 0.03,
            }),
            lfo: Some(crate::Lfo {
                rate_hz: 7.0,
                destination: crate::LfoDestination::Pitch,
                waveform: crate::LfoWaveform::SawDown,
                depth_cents: 20.0,
                ..crate::Lfo::default()
            }),
            modulation: vec![
                crate::Lfo {
                    rate_hz: 5.0,
                    destination: crate::LfoDestination::FilterCutoff,
                    waveform: crate::LfoWaveform::Triangle,
                    filter_cutoff_depth: 0.7,
                    ..crate::Lfo::default()
                },
                crate::Lfo {
                    rate_hz: 3.0,
                    destination: crate::LfoDestination::Amplitude,
                    waveform: crate::LfoWaveform::SawUp,
                    amplitude_depth: 0.15,
                    ..crate::Lfo::default()
                },
            ],
            ..Patch::default()
        }
    }

    fn zoned_sample_test_patch(loop_enabled: bool, pitch_tracking: bool) -> Patch {
        let low = crate::SampleOscillator {
            name: "low".to_owned(),
            samples: (0..32).map(|index| index as f32 / 64.0 - 0.25).collect(),
            sample_rate: 8_000,
            root_frequency_hz: 220.0,
            playback_start_frame: 0,
            playback_end_frame: None,
            loop_enabled,
            loop_start_frame: 3,
            loop_end_frame: Some(13),
        };
        let high = crate::SampleOscillator {
            name: "high".to_owned(),
            samples: (0..32).map(|index| 0.5 - index as f32 / 64.0).collect(),
            sample_rate: 8_000,
            root_frequency_hz: 440.0,
            playback_start_frame: 0,
            playback_end_frame: None,
            loop_enabled,
            loop_start_frame: 5,
            loop_end_frame: Some(17),
        };
        Patch {
            sample_rate: 8_000,
            duration_seconds: 0.01,
            pitch_tracking,
            oscillators: vec![crate::Oscillator {
                waveform: Waveform::Sample,
                sample_zones: vec![
                    crate::SampleZone::new(60, 127, high),
                    crate::SampleZone::new(0, 59, low),
                ],
                ..crate::Oscillator::default()
            }],
            envelope: crate::Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.002,
            },
            ..Patch::default()
        }
    }

    fn test_note(pitch: u8, start_seconds: f64, duration_seconds: f64) -> crate::SynthNote {
        crate::SynthNote {
            pitch,
            start_seconds,
            duration_seconds,
            velocity: 100,
        }
    }

    fn assert_offline_matches_live(patch: Patch, notes: &[crate::SynthNote]) {
        let dry = crate::render_notes_dry(&patch, notes).unwrap();
        assert_eq!(
            dry.samples,
            crate::render_notes_dry(&patch, notes).unwrap().samples,
            "offline rendering was not deterministic"
        );
        let uses_sync = patch.uses_sync();
        let sample_rate = patch.sample_rate;
        let mut dc = DcBlocker::default();
        let mut live = LiveSynth::new(patch).unwrap();
        for frame in 0..dry.samples.len() {
            for note in notes {
                let end = ((note.start_seconds + note.duration_seconds)
                    * f64::from(live.sample_rate()))
                .round() as usize;
                if end == frame {
                    live.note_off(note.pitch);
                }
            }
            for note in notes {
                let start = (note.start_seconds * f64::from(live.sample_rate())).round() as usize;
                if start == frame {
                    live.note_on(note.pitch, note.velocity).unwrap();
                }
            }
            let expected = (dry.samples[frame] * 0.8 * std::f32::consts::FRAC_1_SQRT_2).tanh();
            let expected = if uses_sync {
                dc.process(expected, sample_rate)
            } else {
                expected
            };
            let actual = live.next_frame()[0];
            assert!(
                (actual - expected).abs() < 0.00001,
                "offline/live mismatch at frame {frame}: {actual} != {expected}"
            );
        }
    }
}
