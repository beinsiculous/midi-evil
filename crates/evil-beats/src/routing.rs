use evil_synth::{Filter, FilterMode, FilterProcessor};
use serde::{Deserialize, Serialize};

use crate::{Error, PreparedPad};

pub const BUS_COUNT: usize = 3;

/// Static filter settings shared by pad voices and TRG output buses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumFilter {
    pub mode: FilterMode,
    pub cutoff_hz: f32,
    pub resonance: f32,
    /// Cascaded 6 dB stages: 2 = 12 dB/octave and 4 = 24 dB/octave.
    pub stages: u8,
}

impl Default for DrumFilter {
    fn default() -> Self {
        Self {
            mode: FilterMode::LowPass,
            cutoff_hz: 4_000.0,
            resonance: 0.0,
            stages: 2,
        }
    }
}

impl DrumFilter {
    pub(crate) fn processor(self) -> FilterProcessor {
        FilterProcessor::new(Filter {
            mode: self.mode,
            cutoff_hz: self.cutoff_hz,
            resonance: self.resonance,
            stages: self.stages,
            ..Filter::default()
        })
    }

    pub(crate) fn validate(self) -> Result<(), Error> {
        if !self.cutoff_hz.is_finite()
            || self.cutoff_hz <= 0.0
            || self.cutoff_hz > 96_000.0
            || !self.resonance.is_finite()
            || !(0.0..=1.0).contains(&self.resonance)
            || !(1..=4).contains(&self.stages)
        {
            return Err(Error::InvalidKit(
                "drum filters need 0 < cutoff <= 96000 Hz, resonance 0..=1, and 1..=4 stages"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumBus {
    pub filter: Option<DrumFilter>,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
}

impl Default for DrumBus {
    fn default() -> Self {
        Self {
            filter: None,
            gain: 1.0,
            pan: 0.0,
            muted: false,
        }
    }
}

impl DrumBus {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if let Some(filter) = self.filter {
            filter.validate()?;
        }
        if !self.gain.is_finite()
            || !(0.0..=1.0).contains(&self.gain)
            || !self.pan.is_finite()
            || !(-1.0..=1.0).contains(&self.pan)
        {
            return Err(Error::InvalidKit(
                "bus gain must be 0..=1 and pan must be -1..=1".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct VoiceMix {
    direct: [f32; 2],
    sends: [f32; BUS_COUNT],
}

impl VoiceMix {
    pub(crate) fn add(&mut self, pad: &PreparedPad, sample: f32) {
        let sample = sample * pad.voice.gain;
        let angle = (pad.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        self.direct[0] += sample * angle.cos();
        self.direct[1] += sample * angle.sin();
        for (send, amount) in self.sends.iter_mut().zip(pad.bus_sends) {
            *send += sample * amount;
        }
    }
}

/// Mono sends feed three filtered, panned buses; the direct path stays stereo.
pub(crate) struct BusMixer {
    buses: [DrumBus; BUS_COUNT],
    filters: [Option<FilterProcessor>; BUS_COUNT],
}

impl BusMixer {
    pub(crate) fn new(buses: [DrumBus; BUS_COUNT]) -> Self {
        Self {
            filters: std::array::from_fn(|index| buses[index].filter.map(DrumFilter::processor)),
            buses,
        }
    }

    pub(crate) fn is_quiet(&self) -> bool {
        self.filters.iter().flatten().all(FilterProcessor::is_quiet)
    }

    pub(crate) fn set_buses(&mut self, buses: [DrumBus; BUS_COUNT]) {
        for (index, bus) in buses.iter().enumerate() {
            if self.buses[index].filter != bus.filter {
                self.filters[index] = bus.filter.map(DrumFilter::processor);
            }
        }
        self.buses = buses;
    }

    pub(crate) fn reset(&mut self) {
        for filter in self.filters.iter_mut().flatten() {
            filter.reset();
        }
    }

    pub(crate) fn mix(
        &mut self,
        voice_mix: VoiceMix,
        master_gain: f32,
        sample_rate: u32,
    ) -> [f32; 2] {
        let mut output = voice_mix.direct;
        for (index, bus) in self.buses.iter().enumerate() {
            let input = if bus.muted {
                0.0
            } else {
                voice_mix.sends[index]
            };
            let sample = self.filters[index]
                .as_mut()
                .map_or(input, |filter| filter.process(input, sample_rate));
            if bus.muted {
                continue;
            }
            let sample = sample * bus.gain;
            let angle = (bus.pan + 1.0) * std::f32::consts::FRAC_PI_4;
            output[0] += sample * angle.cos();
            output[1] += sample * angle.sin();
        }
        output.map(|sample| sample * master_gain)
    }
}
