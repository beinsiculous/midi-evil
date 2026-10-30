//! Bounded, sample-by-sample synthesis for the performance keyboard.
use crate::render::{DcBlocker, ModulationState};
use crate::voice::{VoiceControls, VoiceState};
use crate::{Delay, DelayMode, Error, Filter, Patch, Waveform};

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
        let mut patch = self.patch.clone();
        patch.gain *= f32::from(velocity) / 127.0;
        patch.gain = (patch.gain
            * 2.0_f32.powf((60.0 - f32::from(pitch)) / 12.0 * patch.pitch_gain_per_octave))
        .min(1.0);
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
        self.voices.retain(|voice| voice.pitch != pitch);
        if self.voices.len() == MAX_VOICES {
            let oldest_release = self
                .voices
                .iter()
                .position(|voice| !voice.held)
                .unwrap_or(0);
            self.voices.remove(oldest_release);
        }
        self.voices.push(Voice {
            pitch,
            held: true,
            state: VoiceState::new(&patch),
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
            if self.patch.sync.is_some() {
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
    fn pitch_and_xy_controls_change_held_audio_and_voice_count_is_bounded() {
        let patch = crate::preset("lead").unwrap();
        let mut plain = LiveSynth::new(patch.clone()).unwrap();
        let mut bent = LiveSynth::new(patch.clone()).unwrap();
        let mut tone = LiveSynth::new(patch.clone()).unwrap();
        let mut controls = LiveControls::for_patch(&patch);
        controls.bend = 1.0;
        bent.set_controls(controls.clone()).unwrap();
        controls.bend = 0.0;
        controls.filter.as_mut().unwrap().cutoff_hz = 300.0;
        tone.set_controls(controls).unwrap();
        for synth in [&mut plain, &mut bent, &mut tone] {
            synth.note_on(60, 100).unwrap();
        }
        let a: Vec<_> = (0..1000).map(|_| plain.next_frame()).collect();
        assert_ne!(a, (0..1000).map(|_| bent.next_frame()).collect::<Vec<_>>());
        assert_ne!(a, (0..1000).map(|_| tone.next_frame()).collect::<Vec<_>>());
        for pitch in 0..128 {
            plain.note_on(pitch, 100).unwrap();
        }
        assert_eq!(plain.voice_count(), MAX_VOICES);
        assert!((0..10_000).all(|_| {
            plain
                .next_frame()
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1.0)
        }));
    }
}
