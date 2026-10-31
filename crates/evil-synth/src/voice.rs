//! The oscillator/filter voice shared by offline rendering and live performance.
use crate::Patch;
use crate::filter::FilterState;
use crate::oscillator::OscillatorState;
use crate::render::Modulation;

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
    sync_phase: f32,
    modal_phases: Vec<f32>,
}

impl VoiceState {
    pub fn new(patch: &Patch) -> Self {
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
                            )
                        })
                        .collect()
                })
                .collect(),
            filter: FilterState::default(),
            sync_phase: 0.0,
            modal_phases: vec![0.0; patch.modal_partials.len()],
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
        let envelope = patch.envelope.amplitude(time, gate_seconds);
        let modulation = controls.modulation;
        let bend_semitones = controls.bend_semitones;
        let glide_semitones = if patch.glide_seconds > 0.0 && time < patch.glide_seconds {
            controls.glide_from_semitones * (1.0 - time / patch.glide_seconds)
        } else {
            0.0
        };
        let pitch_multiplier = 2.0_f32
            .powf((modulation.pitch_cents + (bend_semitones + glide_semitones) * 100.0) / 1200.0);
        let sync_ratio = patch
            .sync
            .as_ref()
            .map(|sync| (sync.ratio + modulation.sync_ratio).clamp(0.01, 16.0));
        let multiplier = sync_ratio.map_or(pitch_multiplier, |ratio| ratio * pitch_multiplier);
        let sync_step = patch.sync.as_ref().map(|_| {
            patch.oscillators[0].frequency(sweep_progress, pitch_multiplier) / sample_rate as f32
        });
        let count = self.oscillators.len();
        let mut sample = 0.0;
        for (voice_index, states) in self.oscillators.iter_mut().enumerate() {
            let detune = patch.unison.as_ref().map_or(0.0, |unison| {
                (voice_index as f32 - (count - 1) as f32 / 2.0) * unison.detune_cents
            });
            let voice_multiplier = multiplier * 2.0_f32.powf(detune / 1200.0);
            sample += patch
                .oscillators
                .iter()
                .zip(states.iter_mut())
                .map(|(oscillator, state)| {
                    oscillator.sample(
                        state,
                        sweep_progress,
                        sample_rate,
                        voice_multiplier,
                        modulation.pulse_width,
                    )
                })
                .sum::<f32>();
        }
        sample /= count as f32;
        if let Some(step) = sync_step {
            self.sync_phase = (self.sync_phase + step).fract();
            if self.sync_phase < step {
                for (voice_index, states) in self.oscillators.iter_mut().enumerate() {
                    let detune = patch.unison.as_ref().map_or(0.0, |unison| {
                        (voice_index as f32 - (count - 1) as f32 / 2.0) * unison.detune_cents
                    });
                    let voice_multiplier = multiplier * 2.0_f32.powf(detune / 1200.0);
                    for (oscillator, state) in patch.oscillators.iter().zip(states.iter_mut()) {
                        sample += oscillator.sync_correction(
                            state,
                            self.sync_phase,
                            sweep_progress,
                            sample_rate,
                            voice_multiplier,
                            modulation.pulse_width,
                        ) / count as f32;
                        state.reset_phase();
                    }
                }
            }
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
            sample = filter.process(
                sample,
                &mut self.filter,
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
        sample * (envelope * patch.gain * amplitude * controls.pressure * controls.expression)
    }
}
