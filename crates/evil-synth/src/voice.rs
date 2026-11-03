//! The oscillator/filter voice shared by offline rendering and live performance.
use serde::{Deserialize, Serialize};

use crate::filter::FilterState;
use crate::oscillator::OscillatorState;
use crate::render::Modulation;
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

pub(crate) struct VoiceState {
    oscillators: Vec<Vec<OscillatorState>>,
    filter: FilterState,
    sync_phases: Vec<f32>,
    modal_phases: Vec<f32>,
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
            filter: FilterState::default(),
            sync_phases: vec![0.0; count],
            modal_phases: vec![0.0; patch.modal_partials.len()],
        }
    }

    pub fn select_note(&mut self, patch: &Patch, note_pitch: u8) {
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
        let sample_rate = patch.sample_rate;
        let sweep_progress = (time / patch.duration_seconds).min(1.0);
        let amp_envelope = patch.envelope.amplitude(time, gate_seconds);
        let (aux_amplitude, aux_pitch_cents, aux_filter_cutoff_octaves) = patch
            .aux_envelope
            .as_ref()
            .map(|aux| {
                let level = aux.envelope.amplitude(time, gate_seconds);
                (
                    aux.amplitude_depth * level,
                    aux.pitch_depth_cents * level,
                    aux.filter_cutoff_depth_octaves * level,
                )
            })
            .unwrap_or_default();
        let modulation = controls.modulation;
        let bend_semitones = controls.bend_semitones;
        let glide_semitones = if patch.glide_seconds > 0.0 && time < patch.glide_seconds {
            controls.glide_from_semitones * (1.0 - time / patch.glide_seconds)
        } else {
            0.0
        };
        let pitch_multiplier = 2.0_f32.powf(
            (modulation.pitch_cents + aux_pitch_cents + (bend_semitones + glide_semitones) * 100.0)
                / 1200.0,
        );
        let oscillator_mode = patch.effective_oscillator_mode();
        let sync_ratio = patch
            .uses_sync()
            .then(|| (patch.sync_ratio() + modulation.sync_ratio).clamp(0.01, 16.0));
        let count = self.oscillators.len();
        let mut sample = 0.0;
        for (voice_index, states) in self.oscillators.iter_mut().enumerate() {
            let detune = patch.unison.as_ref().map_or(0.0, |unison| {
                (voice_index as f32 - (count - 1) as f32 / 2.0) * unison.detune_cents
            });
            let voice_pitch = pitch_multiplier * 2.0_f32.powf(detune / 1200.0);
            let slave_pitch = sync_ratio.map_or(voice_pitch, |ratio| ratio * voice_pitch);
            let mut voice_sample = 0.0;
            if oscillator_mode == OscillatorMode::Ring {
                let mut oscillators = patch.oscillators.iter().zip(states.iter_mut());
                let (a, a_state) = oscillators.next().expect("ring mode requires Osc A");
                let (b, b_state) = oscillators.next().expect("ring mode requires Osc B");
                let a = a.sample(
                    a_state,
                    sweep_progress,
                    sample_rate,
                    voice_pitch,
                    modulation.pulse_width,
                );
                let b = b.sample(
                    b_state,
                    sweep_progress,
                    sample_rate,
                    voice_pitch,
                    modulation.pulse_width,
                );
                voice_sample = a * b
                    + oscillators
                        .map(|(oscillator, state)| {
                            oscillator.sample(
                                state,
                                sweep_progress,
                                sample_rate,
                                voice_pitch,
                                modulation.pulse_width,
                            )
                        })
                        .sum::<f32>();
            } else {
                for (index, (oscillator, state)) in
                    patch.oscillators.iter().zip(states.iter_mut()).enumerate()
                {
                    let multiplier =
                        if sync_ratio.is_some() && (index > 0 || patch.oscillators.len() == 1) {
                            slave_pitch
                        } else {
                            voice_pitch
                        };
                    voice_sample += oscillator.sample(
                        state,
                        sweep_progress,
                        sample_rate,
                        multiplier,
                        modulation.pulse_width,
                    );
                }
            }
            if sync_ratio.is_some() {
                let sync_step = patch.oscillators[0].frequency(sweep_progress, voice_pitch)
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
                        voice_sample += oscillator.sync_correction(
                            state,
                            *phase,
                            sweep_progress,
                            sample_rate,
                            slave_pitch,
                            modulation.pulse_width,
                        );
                        state.reset_phase(oscillator);
                    }
                }
            }
            sample += voice_sample / count as f32;
        }
        let fundamental = patch.oscillators[0].frequency(sweep_progress, pitch_multiplier);
        for (index, partial) in patch.modal_partials.iter().enumerate() {
            let frequency = fundamental * partial.ratio;
            let phase = self.modal_phases[index];
            sample += (phase * 2.0 * std::f32::consts::PI).sin()
                * partial.amplitude
                * (-time / partial.decay_seconds).exp();
            self.modal_phases[index] = (phase + frequency / sample_rate as f32).fract();
        }
        if let Some(filter) = &patch.filter {
            let filter_envelope = patch
                .filter_envelope
                .as_ref()
                .unwrap_or(&patch.envelope)
                .amplitude(time, gate_seconds);
            sample = filter.process(
                sample,
                &mut self.filter,
                sample_rate,
                filter_envelope,
                modulation.filter_cutoff_octaves
                    + aux_filter_cutoff_octaves
                    + if filter.key_tracking {
                        bend_semitones / 12.0
                    } else {
                        0.0
                    },
            );
        }
        let amplitude = (1.0 + modulation.amplitude + aux_amplitude).clamp(0.0, 1.0);
        sample * (amp_envelope * patch.gain * amplitude * controls.pressure * controls.expression)
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
            sync: Some(OscillatorSync { ratio: 2.0 }),
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
