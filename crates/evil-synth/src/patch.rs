use serde::{Deserialize, Serialize};

use crate::{
    Delay, Envelope, Error, Filter, Lfo, LfoDestination, ModalPartial, Oscillator, OscillatorMode,
    OscillatorSync, Unison, VoiceMode,
};

const MAX_DURATION_SECONDS: f32 = 60.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Patch {
    pub name: String,
    pub sample_rate: u32,
    pub duration_seconds: f32,
    pub gain: f32,
    /// General MIDI program number (0-based), if the patch has a suitable mapping.
    pub midi_program: Option<u8>,
    /// Scale oscillator frequencies from MIDI notes when rendering a performance.
    pub pitch_tracking: bool,
    /// Additional gain change per octave away from middle C; positive favors low notes.
    pub pitch_gain_per_octave: f32,
    pub pitch_bend_range_semitones: f32,
    /// Seconds to slide from the previous note's pitch; zero disables glide.
    pub glide_seconds: f32,
    /// Velocity at which accent is enabled; zero disables accent.
    pub accent_velocity_threshold: u8,
    pub accent_filter_octaves: f32,
    /// Additional gain multiplier applied to accented notes.
    pub accent_gain: f32,
    pub seed: u32,
    pub oscillators: Vec<Oscillator>,
    pub oscillator_mode: OscillatorMode,
    /// Independently decaying inharmonic partials for modal instruments.
    pub modal_partials: Vec<ModalPartial>,
    /// Sync ratio and compatibility with patches that enabled sync with this field alone.
    pub sync: Option<OscillatorSync>,
    pub voice_mode: VoiceMode,
    pub unison: Option<Unison>,
    pub envelope: Envelope,
    pub filter: Option<Filter>,
    /// Independent filter ADSR; `None` routes the amplitude envelope to the filter.
    pub filter_envelope: Option<Envelope>,
    /// Primary LFO, retained for compatibility with the original patch format.
    pub lfo: Option<Lfo>,
    pub modulation: Vec<Lfo>,
    pub delay: Option<Delay>,
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            name: "Untitled".to_owned(),
            sample_rate: 48_000,
            duration_seconds: 0.25,
            gain: 0.8,
            midi_program: None,
            pitch_tracking: true,
            pitch_gain_per_octave: 0.0,
            pitch_bend_range_semitones: 2.0,
            glide_seconds: 0.0,
            accent_velocity_threshold: 0,
            accent_filter_octaves: 0.0,
            accent_gain: 0.0,
            seed: 0x5255_5354,
            oscillators: vec![Oscillator::default()],
            oscillator_mode: OscillatorMode::Mix,
            modal_partials: Vec::new(),
            sync: None,
            voice_mode: VoiceMode::Poly,
            unison: None,
            envelope: Envelope::default(),
            filter: None,
            filter_envelope: None,
            lfo: None,
            modulation: Vec::new(),
            delay: None,
        }
    }
}

impl Patch {
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let patch: Self = serde_json::from_str(json)?;
        patch.validate()?;
        Ok(patch)
    }

    pub fn to_json_pretty(&self) -> Result<String, Error> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Resolve the original `sync: {...}` JSON representation as sync mode.
    pub fn effective_oscillator_mode(&self) -> OscillatorMode {
        if self.oscillator_mode == OscillatorMode::Mix && self.sync.is_some() {
            OscillatorMode::Sync
        } else {
            self.oscillator_mode
        }
    }

    pub fn uses_sync(&self) -> bool {
        self.effective_oscillator_mode() == OscillatorMode::Sync
    }

    pub(crate) fn sync_ratio(&self) -> f32 {
        self.sync.as_ref().map_or(1.5, |sync| sync.ratio)
    }

    pub fn lfos(&self) -> impl Iterator<Item = &Lfo> {
        self.lfo.iter().chain(self.modulation.iter())
    }

    pub fn lfo_count(&self) -> usize {
        self.lfos().count()
    }

    pub fn lfo_mut(&mut self, index: usize) -> Option<&mut Lfo> {
        match (self.lfo.is_some(), index) {
            (true, 0) => self.lfo.as_mut(),
            (true, _) => self.modulation.get_mut(index - 1),
            (false, _) => self.modulation.get_mut(index),
        }
    }

    pub fn add_lfo(&mut self) {
        if self.lfo_count() >= 8 {
            return;
        }
        if self.lfo.is_none() {
            self.lfo = Some(Lfo::default());
        } else {
            self.modulation.push(Lfo::default());
        }
    }

    pub fn remove_lfo(&mut self, index: usize) {
        if self.lfo.is_none() {
            if index < self.modulation.len() {
                self.modulation.remove(index);
            }
        } else if index == 0 {
            self.lfo = if self.modulation.is_empty() {
                None
            } else {
                Some(self.modulation.remove(0))
            };
        } else if index <= self.modulation.len() {
            self.modulation.remove(index - 1);
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !(8_000..=192_000).contains(&self.sample_rate) {
            return Err(invalid("sample_rate must be between 8000 and 192000"));
        }
        if !self.duration_seconds.is_finite()
            || self.duration_seconds <= 0.0
            || self.duration_seconds > MAX_DURATION_SECONDS
        {
            return Err(invalid("duration_seconds must be between 0 and 60"));
        }
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err(invalid("gain must be between 0 and 1"));
        }
        if !self.pitch_gain_per_octave.is_finite()
            || !(-2.0..=2.0).contains(&self.pitch_gain_per_octave)
        {
            return Err(invalid("pitch_gain_per_octave must be between -2 and 2"));
        }
        if !self.pitch_bend_range_semitones.is_finite()
            || self.pitch_bend_range_semitones <= 0.0
            || self.pitch_bend_range_semitones > 24.0
        {
            return Err(invalid(
                "pitch bend range must be between 0 and 24 semitones",
            ));
        }
        if !self.glide_seconds.is_finite()
            || !(0.0..=MAX_DURATION_SECONDS).contains(&self.glide_seconds)
        {
            return Err(invalid("glide_seconds must be between 0 and 60"));
        }
        if self.accent_velocity_threshold > 127
            || !self.accent_filter_octaves.is_finite()
            || !self.accent_gain.is_finite()
            || !(0.0..=1.0).contains(&self.accent_gain)
        {
            return Err(invalid("accent settings must be valid"));
        }
        if self.midi_program.is_some_and(|program| program > 127) {
            return Err(invalid("midi_program must be between 0 and 127"));
        }
        if self.oscillators.is_empty() || self.oscillators.len() > 8 {
            return Err(invalid("a patch needs between 1 and 8 oscillators"));
        }
        if self.oscillator_mode == OscillatorMode::Ring && self.oscillators.len() < 2 {
            return Err(invalid("ring mode needs Osc A and Osc B"));
        }

        let nyquist = self.sample_rate as f32 / 2.0;
        for oscillator in &self.oscillators {
            if !oscillator.amplitude.is_finite() || !(0.0..=1.0).contains(&oscillator.amplitude) {
                return Err(invalid("oscillator amplitude must be between 0 and 1"));
            }
            if !oscillator.detune_cents.is_finite() {
                return Err(invalid("oscillator detune_cents must be finite"));
            }
            if !oscillator.pulse_width.is_finite()
                || !(0.01..=0.99).contains(&oscillator.pulse_width)
            {
                return Err(invalid(
                    "oscillator pulse_width must be between 0.01 and 0.99",
                ));
            }
            for frequency in [oscillator.start_frequency_hz, oscillator.end_frequency_hz] {
                if !frequency.is_finite() || frequency <= 0.0 || frequency > nyquist {
                    return Err(invalid(
                        "oscillator frequencies must be positive and no higher than Nyquist",
                    ));
                }
            }
        }

        if self.modal_partials.len() > 16 {
            return Err(invalid("a patch supports at most 16 modal partials"));
        }
        for partial in &self.modal_partials {
            partial.validate()?;
        }
        if let Some(sync) = &self.sync {
            sync.validate()?;
        }
        if let Some(unison) = &self.unison {
            unison.validate()?;
        }

        validate_envelope(&self.envelope)?;
        if self.duration_seconds + self.envelope.release_seconds > MAX_DURATION_SECONDS {
            return Err(invalid("duration plus release cannot exceed 60 seconds"));
        }
        if let Some(envelope) = &self.filter_envelope {
            validate_envelope(envelope)?;
            if self.duration_seconds + envelope.release_seconds > MAX_DURATION_SECONDS {
                return Err(invalid(
                    "duration plus filter-envelope release cannot exceed 60 seconds",
                ));
            }
        }
        if let Some(filter) = &self.filter
            && (!filter.cutoff_hz.is_finite()
                || filter.cutoff_hz <= 0.0
                || filter.cutoff_hz >= nyquist
                || !filter.envelope_amount.is_finite()
                || !filter.resonance.is_finite()
                || !(0.0..=1.0).contains(&filter.resonance)
                || !(1..=4).contains(&filter.stages))
        {
            return Err(invalid("filter cutoff and envelope_amount must be valid"));
        }
        if self.lfo_count() > 8 {
            return Err(invalid("a patch supports at most 8 LFO lanes"));
        }
        for (index, lfo) in self.lfos().enumerate() {
            if !lfo.rate_hz.is_finite()
                || lfo.rate_hz < 0.0
                || lfo
                    .beats_per_cycle
                    .is_some_and(|beats| !beats.is_finite() || !(0.001..=64.0).contains(&beats))
                || !lfo.depth_cents.is_finite()
                || !lfo.pulse_width_depth.is_finite()
                || !(-0.99..=0.99).contains(&lfo.pulse_width_depth)
                || !lfo.sync_ratio_depth.is_finite()
                || !(-16.0..=16.0).contains(&lfo.sync_ratio_depth)
                || !lfo.filter_cutoff_depth.is_finite()
                || !(-12.0..=12.0).contains(&lfo.filter_cutoff_depth)
                || !lfo.amplitude_depth.is_finite()
                || !(-1.0..=1.0).contains(&lfo.amplitude_depth)
                || !lfo.lfo_rate_depth.is_finite()
                || !(-4.0..=4.0).contains(&lfo.lfo_rate_depth)
                || (lfo.destination == LfoDestination::LfoRate
                    && lfo
                        .target_lfo
                        .is_none_or(|target| target >= self.lfo_count() || target == index))
            {
                return Err(invalid("lfo rate and destination depths must be valid"));
            }
        }
        if let Some(delay) = &self.delay {
            delay.validate()?;
        }
        Ok(())
    }
}

fn validate_envelope(envelope: &Envelope) -> Result<(), Error> {
    for duration in [
        envelope.attack_seconds,
        envelope.decay_seconds,
        envelope.release_seconds,
    ] {
        if !duration.is_finite() || !(0.0..=MAX_DURATION_SECONDS).contains(&duration) {
            return Err(invalid("envelope durations must be between 0 and 60"));
        }
    }
    if !envelope.sustain_level.is_finite() || !(0.0..=1.0).contains(&envelope.sustain_level) {
        return Err(invalid("envelope sustain_level must be between 0 and 1"));
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::InvalidPatch(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Waveform;

    #[test]
    fn json_round_trip_preserves_the_patch() {
        let mut patch = Patch {
            name: "Test tone".to_owned(),
            midi_program: Some(80),
            oscillator_mode: OscillatorMode::Sync,
            sync: Some(OscillatorSync::default()),
            voice_mode: VoiceMode::Mono,
            unison: Some(Unison::default()),
            filter: Some(Filter {
                mode: crate::FilterMode::BandPass,
                stages: 4,
                ..Filter::default()
            }),
            filter_envelope: Some(Envelope {
                attack_seconds: 0.02,
                decay_seconds: 0.04,
                sustain_level: 0.3,
                release_seconds: 0.08,
            }),
            delay: Some(Delay::default()),
            lfo: Some(Lfo {
                rate_hz: 6.0,
                beats_per_cycle: Some(0.25),
                depth_cents: 25.0,
                ..Lfo::default()
            }),
            modulation: vec![Lfo {
                destination: crate::LfoDestination::FilterCutoff,
                ..Lfo::default()
            }],
            ..Patch::default()
        };
        patch.oscillators[0].waveform = Waveform::Triangle;
        let json = patch.to_json_pretty().unwrap();
        let decoded = Patch::from_json(&json).unwrap();
        assert_eq!(decoded, patch);
    }

    #[test]
    fn missing_json_fields_use_useful_defaults() {
        let patch = Patch::from_json("{}").unwrap();
        assert_eq!(patch.sample_rate, 48_000);
        assert_eq!(patch.oscillators.len(), 1);
        assert_eq!(patch.filter_envelope, None);
    }

    #[test]
    fn legacy_sync_json_and_explicit_modes_remain_compatible() {
        let legacy = Patch::from_json(r#"{"sync":{"ratio":2.0}}"#).unwrap();
        assert_eq!(legacy.effective_oscillator_mode(), OscillatorMode::Sync);
        assert_eq!(legacy.voice_mode, VoiceMode::Poly);
        assert_eq!(Patch::from_json("{}").unwrap().voice_mode, VoiceMode::Poly);

        let mut ring = Patch {
            oscillator_mode: OscillatorMode::Ring,
            ..Patch::default()
        };
        assert!(ring.validate().is_err());
        ring.oscillators.push(Oscillator::default());
        ring.voice_mode = VoiceMode::Legato;
        let decoded = Patch::from_json(&ring.to_json_pretty().unwrap()).unwrap();
        assert_eq!(decoded, ring);
    }

    #[test]
    fn modulation_lanes_add_remove_and_preserve_order() {
        let mut patch = Patch::default();
        patch.add_lfo();
        patch.add_lfo();
        patch.add_lfo();
        for index in 0..3 {
            patch.lfo_mut(index).unwrap().rate_hz = (index + 1) as f32;
        }
        patch.remove_lfo(0);
        assert_eq!(patch.lfo_count(), 2);
        assert_eq!(patch.lfo_mut(0).unwrap().rate_hz, 2.0);
        assert_eq!(patch.lfo_mut(1).unwrap().rate_hz, 3.0);
        patch.remove_lfo(1);
        patch.remove_lfo(0);
        assert_eq!(patch.lfo_count(), 0);
        patch.add_lfo();
        assert_eq!(patch.lfo_count(), 1);
    }

    #[test]
    fn validation_rejects_invalid_audio_parameters() {
        let mut patch = Patch::default();
        patch.oscillators.clear();
        assert!(patch.validate().is_err());
        let patch = Patch {
            sample_rate: 0,
            ..Patch::default()
        };
        assert!(patch.validate().is_err());
        let mut patch = Patch::default();
        patch.oscillators[0].start_frequency_hz = f32::NAN;
        assert!(patch.validate().is_err());
        patch.oscillators[0].start_frequency_hz = 440.0;
        patch.filter_envelope = Some(Envelope {
            attack_seconds: f32::NAN,
            ..Envelope::default()
        });
        assert!(patch.validate().is_err());
    }
}
