//! The oscillator/filter voice shared by offline rendering and live performance.
use serde::{Deserialize, Serialize};

use crate::filter::FilterState;
use crate::oscillator::OscillatorState;
use crate::render::{DcBlocker, Modulation};
use crate::{OscillatorMode, Patch};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMode {
    /// Start an independent voice for every note.
    #[default]
    Poly,
    /// Retrigger one voice when a new note starts.
    Mono,
    /// Reuse one voice without retriggering its envelope or oscillator phases.
    Legato,
}

pub(crate) struct VoiceControls {
    pub modulation: Modulation,
    pub bend_semitones: f32,
    pub glide_from_semitones: f32,
    pub pressure: f32,
    pub expression: f32,
}

#[derive(Default)]
struct AuxModulation {
    amplitude: f32,
    pitch_cents: f32,
    pulse_width: f32,
    sync_ratio: f32,
    filter_cutoff_octaves: f32,
    filter_resonance: f32,
    oscillator_mix: f32,
}

pub(crate) struct VoiceState {
    oscillators: Vec<Vec<OscillatorState>>,
    filter: [FilterState; 2],
    dc_blocker: [DcBlocker; 2],
    sync_phases: Vec<f32>,
    modal_phases: Vec<f32>,
    note_pitch: u8,
}

struct VoiceFrameParams {
    sweep_progress: f32,
    pitch_multiplier: f32,
    oscillator_mode: OscillatorMode,
    oscillator_mix: f32,
    pulse_width: f32,
    sync_ratio: Option<f32>,
    filter_envelope: Option<f32>,
    filter_cutoff_octaves: f32,
    filter_resonance: f32,
    gain_scale: f32,
}

impl VoiceFrameParams {
    fn compute(patch: &Patch, time: f32, gate_seconds: f32, controls: &VoiceControls) -> Self {
        let sweep_progress = (time / patch.duration_seconds).min(1.0);
        let amp_envelope = if patch.modal_one_shot {
            1.0
        } else {
            patch
                .envelope
                .amplitude_with_inversion(time, gate_seconds, patch.invert_amp_envelope)
        };
        let aux = [patch.aux_envelope.as_ref(), patch.aux_envelope_2.as_ref()]
            .into_iter()
            .flatten()
            .fold(AuxModulation::default(), |mut total, aux| {
                let level = aux.envelope.amplitude(time, gate_seconds);
                total.amplitude += aux.amplitude_depth * level;
                total.pitch_cents += aux.pitch_depth_cents * level;
                total.pulse_width += aux.pulse_width_depth * level;
                total.sync_ratio += aux.sync_ratio_depth * level;
                total.filter_cutoff_octaves += aux.filter_cutoff_depth_octaves * level;
                total.filter_resonance += aux.filter_resonance_depth * level;
                total.oscillator_mix += aux.oscillator_mix_depth * level;
                total
            });
        let modulation = &controls.modulation;
        let bend_semitones = controls.bend_semitones;
        let glide_semitones = if patch.glide_seconds > 0.0 && time < patch.glide_seconds {
            controls.glide_from_semitones * (1.0 - time / patch.glide_seconds)
        } else {
            0.0
        };
        let pitch_multiplier = 2.0_f32.powf(
            (modulation.pitch_cents + aux.pitch_cents + (bend_semitones + glide_semitones) * 100.0)
                / 1200.0,
        );
        let oscillator_mode = patch.effective_oscillator_mode();
        let oscillator_mix = (modulation.oscillator_mix + aux.oscillator_mix).clamp(-1.0, 1.0);
        let pulse_width = modulation.pulse_width + aux.pulse_width;
        let sync_ratio = patch.uses_sync().then(|| {
            let offset = modulation.sync_ratio + aux.sync_ratio;
            let ratio = if patch
                .sync
                .as_ref()
                .is_some_and(|sync| sync.modulation_octaves)
            {
                patch.sync_ratio() * 2.0_f32.powf(offset)
            } else {
                patch.sync_ratio() + offset
            };
            ratio.clamp(0.01, 16.0)
        });
        let filter_envelope = patch.filter.as_ref().map(|_| {
            patch
                .filter_envelope
                .as_ref()
                .unwrap_or(&patch.envelope)
                .amplitude(time, gate_seconds)
        });
        let filter_cutoff_octaves = modulation.filter_cutoff_octaves
            + aux.filter_cutoff_octaves
            + patch.filter.as_ref().map_or(0.0, |filter| {
                if filter.key_tracking {
                    bend_semitones / 12.0 * filter.key_tracking_ratio
                } else {
                    0.0
                }
            });
        let filter_resonance = modulation.filter_resonance + aux.filter_resonance;
        let amplitude = (1.0 + modulation.amplitude + aux.amplitude).clamp(0.0, 1.0);
        let gain_scale =
            amp_envelope * patch.gain * amplitude * controls.pressure * controls.expression;
        Self {
            sweep_progress,
            pitch_multiplier,
            oscillator_mode,
            oscillator_mix,
            pulse_width,
            sync_ratio,
            filter_envelope,
            filter_cutoff_octaves,
            filter_resonance,
            gain_scale,
        }
    }
}

impl VoiceState {
    pub fn new(patch: &Patch, note_pitch: u8) -> Self {
        let count = patch
            .unison
            .as_ref()
            .map_or(1, |unison| usize::from(unison.voices));
        Self {
            oscillators: (0..count)
                .map(|voice| {
                    patch
                        .oscillators
                        .iter()
                        .enumerate()
                        .map(|(index, oscillator)| {
                            oscillator.state(
                                patch
                                    .seed
                                    .wrapping_add((voice as u32).wrapping_mul(0x9e37_79b9))
                                    .wrapping_add(index as u32),
                                note_pitch,
                            )
                        })
                        .collect()
                })
                .collect(),
            filter: [FilterState::default(), FilterState::default()],
            dc_blocker: [DcBlocker::default(), DcBlocker::default()],
            sync_phases: vec![0.0; count],
            modal_phases: vec![0.0; patch.modal_partials.len()],
            note_pitch,
        }
    }

    pub fn select_note(&mut self, patch: &Patch, note_pitch: u8) {
        self.note_pitch = note_pitch;
        for states in &mut self.oscillators {
            for (oscillator, state) in patch.oscillators.iter().zip(states.iter_mut()) {
                state.select_sample_zone(oscillator, note_pitch);
            }
        }
    }

    pub fn sample(
        &mut self,
        patch: &Patch,
        time: f32,
        gate_seconds: f32,
        controls: VoiceControls,
    ) -> f32 {
        self.sample_channels::<false>(patch, time, gate_seconds, controls)[0]
    }

    pub fn sample_stereo(
        &mut self,
        patch: &Patch,
        time: f32,
        gate_seconds: f32,
        controls: VoiceControls,
    ) -> [f32; 2] {
        self.sample_channels::<true>(patch, time, gate_seconds, controls)
    }

    fn sample_channels<const STEREO: bool>(
        &mut self,
        patch: &Patch,
        time: f32,
        gate_seconds: f32,
        controls: VoiceControls,
    ) -> [f32; 2] {
        let sample_rate = patch.sample_rate;
        let params = VoiceFrameParams::compute(patch, time, gate_seconds, &controls);
        let count = self.oscillators.len();
        let mut sample = [0.0_f32; 2];
        for (voice_index, states) in self.oscillators.iter_mut().enumerate() {
            let detune = patch.unison.as_ref().map_or(0.0, |unison| {
                (voice_index as f32 - (count - 1) as f32 / 2.0) * unison.detune_cents
            });
            let voice_pitch = params.pitch_multiplier * 2.0_f32.powf(detune / 1200.0);
            let slave_pitch = params
                .sync_ratio
                .map_or(voice_pitch, |ratio| ratio * voice_pitch);
            let mut voice_sample = [0.0_f32; 2];
            if matches!(
                params.oscillator_mode,
                OscillatorMode::Ring | OscillatorMode::Ring2
            ) {
                let mut oscillators = patch.oscillators.iter().zip(states.iter_mut());
                let (a, a_state) = oscillators.next().expect("ring mode requires Osc A");
                let (b, b_state) = oscillators.next().expect("ring mode requires Osc B");
                let b_pitch =
                    if params.oscillator_mode == OscillatorMode::Ring2 && patch.pitch_tracking {
                        voice_pitch * 2.0_f32.powf((69.0 - f32::from(self.note_pitch)) / 12.0)
                    } else {
                        voice_pitch
                    };
                let a = Self::eval_oscillator::<STEREO>(
                    a,
                    a_state,
                    params.sweep_progress,
                    sample_rate,
                    voice_pitch,
                    params.pulse_width,
                );
                let b = Self::eval_oscillator::<STEREO>(
                    b,
                    b_state,
                    params.sweep_progress,
                    sample_rate,
                    b_pitch,
                    params.pulse_width,
                );
                voice_sample = [a[0] * b[0], a[1] * b[1]];
                for (oscillator, state) in oscillators {
                    let aux_sample = Self::eval_oscillator::<STEREO>(
                        oscillator,
                        state,
                        params.sweep_progress,
                        sample_rate,
                        voice_pitch,
                        params.pulse_width,
                    );
                    voice_sample[0] += aux_sample[0];
                    if STEREO {
                        voice_sample[1] += aux_sample[1];
                    }
                }
            } else {
                for (index, (oscillator, state)) in
                    patch.oscillators.iter().zip(states.iter_mut()).enumerate()
                {
                    let multiplier = if params.sync_ratio.is_some()
                        && (index > 0 || patch.oscillators.len() == 1)
                    {
                        slave_pitch
                    } else {
                        voice_pitch
                    };
                    let mix_gain = if params.oscillator_mode != OscillatorMode::Mix {
                        1.0
                    } else {
                        match index {
                            0 => 1.0 - params.oscillator_mix.max(0.0),
                            1 => 1.0 + params.oscillator_mix.min(0.0),
                            _ => 1.0,
                        }
                    };
                    let osc_sample = Self::eval_oscillator::<STEREO>(
                        oscillator,
                        state,
                        params.sweep_progress,
                        sample_rate,
                        multiplier,
                        params.pulse_width,
                    );
                    voice_sample[0] += osc_sample[0] * mix_gain;
                    if STEREO {
                        voice_sample[1] += osc_sample[1] * mix_gain;
                    }
                }
            }
            if params.sync_ratio.is_some() {
                let sync_step = patch.oscillators[0].frequency(params.sweep_progress, voice_pitch)
                    / sample_rate as f32;
                let phase = &mut self.sync_phases[voice_index];
                *phase = (*phase + sync_step).fract();
                if *phase < sync_step {
                    let first_slave = usize::from(patch.oscillators.len() > 1);
                    for (oscillator, state) in patch
                        .oscillators
                        .iter()
                        .zip(states.iter_mut())
                        .skip(first_slave)
                    {
                        let correction = oscillator.sync_correction(
                            state,
                            *phase,
                            params.sweep_progress,
                            sample_rate,
                            slave_pitch,
                            params.pulse_width,
                        );
                        voice_sample[0] += correction;
                        if STEREO {
                            voice_sample[1] += correction;
                        }
                        state.reset_phase(oscillator);
                    }
                }
            }
            sample[0] += voice_sample[0] / count as f32;
            if STEREO {
                sample[1] += voice_sample[1] / count as f32;
            }
        }
        let fundamental =
            patch.oscillators[0].frequency(params.sweep_progress, params.pitch_multiplier);
        for (index, partial) in patch.modal_partials.iter().enumerate() {
            let frequency = fundamental * partial.ratio;
            let phase = self.modal_phases[index];
            let pitch_gain = 2.0_f32
                .powf((60.0 - f32::from(self.note_pitch)) / 12.0 * partial.pitch_gain_per_octave);
            let modal = (phase * 2.0 * std::f32::consts::PI).sin()
                * partial.amplitude
                * pitch_gain
                * (-time / partial.decay_seconds).exp();
            sample[0] += modal;
            if STEREO {
                sample[1] += modal;
            }
            self.modal_phases[index] = (phase + frequency / sample_rate as f32).fract();
        }
        let out0 = self.post_process_channel(patch, sample[0], 0, &params);
        if STEREO {
            [
                out0,
                self.post_process_channel(patch, sample[1], 1, &params),
            ]
        } else {
            [out0, out0]
        }
    }

    #[inline]
    fn eval_oscillator<const STEREO: bool>(
        oscillator: &crate::Oscillator,
        state: &mut OscillatorState,
        sweep_progress: f32,
        sample_rate: u32,
        pitch_multiplier: f32,
        pulse_width: f32,
    ) -> [f32; 2] {
        if STEREO {
            oscillator.sample_stereo(
                state,
                sweep_progress,
                sample_rate,
                pitch_multiplier,
                pulse_width,
            )
        } else {
            let value = oscillator.sample(
                state,
                sweep_progress,
                sample_rate,
                pitch_multiplier,
                pulse_width,
            );
            [value, value]
        }
    }

    fn post_process_channel(
        &mut self,
        patch: &Patch,
        mut sample: f32,
        channel: usize,
        params: &VoiceFrameParams,
    ) -> f32 {
        let sample_rate = patch.sample_rate;
        if let Some(filter) = &patch.filter {
            sample = filter.process(
                sample,
                &mut self.filter[channel],
                sample_rate,
                params.filter_envelope.expect("filter envelope computed"),
                params.filter_cutoff_octaves,
                params.filter_resonance,
            );
        }
        if let Some(dc_block_hz) = patch
            .sync
            .as_ref()
            .filter(|_| patch.uses_sync())
            .map(|sync| sync.dc_block_hz)
            .filter(|cutoff_hz| *cutoff_hz > 0.0)
        {
            sample = self.dc_blocker[channel].process_with_cutoff(sample, sample_rate, dc_block_hz);
        }
        sample * params.gain_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Envelope, Oscillator, OscillatorSync, Waveform};

    #[test]
    fn mix_adds_and_ring_multiplies_osc_a_and_b() {
        let patch = Patch {
            gain: 1.0,
            duration_seconds: 1.0,
            oscillators: vec![
                Oscillator {
                    amplitude: 0.6,
                    ..Oscillator::default()
                },
                Oscillator {
                    start_frequency_hz: 660.0,
                    end_frequency_hz: 660.0,
                    amplitude: 0.4,
                    ..Oscillator::default()
                },
            ],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.0,
            },
            ..Patch::default()
        };
        let mut mix = VoiceState::new(&patch, 69);
        let mut ring = VoiceState::new(&patch, 69);
        let mut a_state = patch.oscillators[0].state(1, 69);
        let mut b_state = patch.oscillators[1].state(2, 69);
        let ring_patch = Patch {
            oscillator_mode: OscillatorMode::Ring,
            ..patch.clone()
        };
        for frame in 0..64 {
            let time = frame as f32 / patch.sample_rate as f32;
            let a = patch.oscillators[0].sample(&mut a_state, time, patch.sample_rate, 1.0, 0.0);
            let b = patch.oscillators[1].sample(&mut b_state, time, patch.sample_rate, 1.0, 0.0);
            assert_eq!(mix.sample(&patch, time, 1.0, controls()), a + b);
            assert_eq!(ring.sample(&ring_patch, time, 1.0, controls()), a * b);
        }
    }

    #[test]
    fn ring2_removes_only_the_keyboard_ratio_from_osc_b() {
        let base = Patch {
            gain: 1.0,
            duration_seconds: 1.0,
            oscillator_mode: OscillatorMode::Ring2,
            oscillators: vec![
                Oscillator {
                    amplitude: 0.6,
                    ..Oscillator::default()
                },
                Oscillator {
                    start_frequency_hz: 660.0,
                    end_frequency_hz: 660.0,
                    amplitude: 0.4,
                    ..Oscillator::default()
                },
            ],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.0,
            },
            ..Patch::default()
        };
        for pitch in [48, 60, 72] {
            let key_ratio = 2.0_f32.powf((f32::from(pitch) - 69.0) / 12.0);
            let mut patch = base.clone();
            for oscillator in &mut patch.oscillators {
                oscillator.start_frequency_hz *= key_ratio;
                oscillator.end_frequency_hz *= key_ratio;
            }
            let mut voice = VoiceState::new(&patch, pitch);
            let mut a_state = patch.oscillators[0].state(1, pitch);
            let mut b_state = base.oscillators[1].state(2, 69);
            for frame in 0..64 {
                let time = frame as f32 / patch.sample_rate as f32;
                let a =
                    patch.oscillators[0].sample(&mut a_state, time, patch.sample_rate, 1.0, 0.0);
                let b = base.oscillators[1].sample(&mut b_state, time, patch.sample_rate, 1.0, 0.0);
                let actual = voice.sample(&patch, time, 1.0, controls());
                assert!((actual - a * b).abs() < 0.000_001);
            }
        }
    }

    #[test]
    fn sync_does_not_reset_or_scale_the_osc_a_master() {
        let patch = Patch {
            gain: 1.0,
            duration_seconds: 1.0,
            oscillators: vec![
                Oscillator {
                    waveform: Waveform::Square,
                    ..Oscillator::default()
                },
                Oscillator {
                    waveform: Waveform::Square,
                    amplitude: 0.0,
                    ..Oscillator::default()
                },
            ],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.0,
            },
            ..Patch::default()
        };
        let synced = Patch {
            oscillator_mode: OscillatorMode::Sync,
            sync: Some(OscillatorSync {
                ratio: 2.0,
                ..OscillatorSync::default()
            }),
            ..patch.clone()
        };
        let mut plain_state = VoiceState::new(&patch, 69);
        let mut sync_state = VoiceState::new(&synced, 69);
        for frame in 0..256 {
            let time = frame as f32 / patch.sample_rate as f32;
            assert_eq!(
                sync_state.sample(&synced, time, 1.0, controls()),
                plain_state.sample(&patch, time, 1.0, controls())
            );
        }
    }

    #[test]
    fn octave_sync_modulation_scales_exponentially_and_dc_block_settles_collapsed_sync() {
        let base = Patch {
            sample_rate: 8_000,
            gain: 1.0,
            duration_seconds: 1.0,
            oscillator_mode: OscillatorMode::Sync,
            oscillators: vec![Oscillator {
                waveform: Waveform::Square,
                ..Oscillator::default()
            }],
            envelope: Envelope {
                attack_seconds: 0.0,
                decay_seconds: 0.0,
                sustain_level: 1.0,
                release_seconds: 0.0,
            },
            ..Patch::default()
        };
        for (octave_offset, linear_ratio) in [(1.0, 2.0), (-1.0, 0.5)] {
            let octave_patch = Patch {
                sync: Some(OscillatorSync {
                    ratio: 1.0,
                    modulation_octaves: true,
                    dc_block_hz: 0.0,
                }),
                ..base.clone()
            };
            let linear_patch = Patch {
                sync: Some(OscillatorSync {
                    ratio: linear_ratio,
                    modulation_octaves: false,
                    dc_block_hz: 0.0,
                }),
                ..base.clone()
            };
            let mut octave_state = VoiceState::new(&octave_patch, 69);
            let mut linear_state = VoiceState::new(&linear_patch, 69);
            for frame in 0..128 {
                let time = frame as f32 / 8_000.0;
                assert_eq!(
                    octave_state.sample(
                        &octave_patch,
                        time,
                        1.0,
                        VoiceControls {
                            modulation: Modulation {
                                sync_ratio: octave_offset,
                                ..Modulation::default()
                            },
                            ..controls()
                        },
                    ),
                    linear_state.sample(&linear_patch, time, 1.0, controls())
                );
            }
        }

        let unblocked_patch = Patch {
            sync: Some(OscillatorSync {
                ratio: 1.0,
                modulation_octaves: true,
                dc_block_hz: 0.0,
            }),
            ..base.clone()
        };
        let blocked_patch = Patch {
            sync: Some(OscillatorSync {
                ratio: 1.0,
                modulation_octaves: true,
                dc_block_hz: 30.0,
            }),
            ..base
        };
        let mut unblocked = VoiceState::new(&unblocked_patch, 69);
        let mut blocked = VoiceState::new(&blocked_patch, 69);
        let mut unblocked_mean = 0.0_f32;
        let mut blocked_mean = 0.0_f32;
        for frame in 0..600 {
            let time = frame as f32 / 8_000.0;
            let collapsed_controls = VoiceControls {
                modulation: Modulation {
                    sync_ratio: -1.5,
                    ..Modulation::default()
                },
                ..controls()
            };
            let raw = unblocked.sample(&unblocked_patch, time, 1.0, collapsed_controls);
            let clean = blocked.sample(
                &blocked_patch,
                time,
                1.0,
                VoiceControls {
                    modulation: Modulation {
                        sync_ratio: -1.5,
                        ..Modulation::default()
                    },
                    ..controls()
                },
            );
            if frame >= 400 {
                unblocked_mean += raw;
                blocked_mean += clean;
            }
        }
        assert!((unblocked_mean / 200.0).abs() > 0.5);
        assert!((blocked_mean / 200.0).abs() < 0.05);
    }

    fn controls() -> VoiceControls {
        VoiceControls {
            modulation: Modulation::default(),
            bend_semitones: 0.0,
            glide_from_semitones: 0.0,
            pressure: 1.0,
            expression: 1.0,
        }
    }
}
