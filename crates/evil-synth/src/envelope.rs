use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Envelope {
    pub attack_seconds: f32,
    pub decay_seconds: f32,
    pub sustain_level: f32,
    pub release_seconds: f32,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack_seconds: 0.005,
            decay_seconds: 0.05,
            sustain_level: 0.8,
            release_seconds: 0.1,
        }
    }
}

impl Envelope {
    pub fn amplitude(&self, time_seconds: f32, gate_seconds: f32) -> f32 {
        self.amplitude_with_inversion(time_seconds, gate_seconds, false)
    }

    pub(crate) fn amplitude_with_inversion(
        &self,
        time_seconds: f32,
        gate_seconds: f32,
        inverted: bool,
    ) -> f32 {
        let level = if time_seconds <= gate_seconds {
            self.level(time_seconds)
        } else {
            if self.release_seconds == 0.0 || time_seconds >= gate_seconds + self.release_seconds {
                return 0.0;
            }
            let release_progress = (time_seconds - gate_seconds) / self.release_seconds;
            self.level(gate_seconds) * (1.0 - release_progress).max(0.0)
        };
        if inverted { 1.0 - level } else { level }
    }

    fn level(&self, time_seconds: f32) -> f32 {
        if self.attack_seconds > 0.0 && time_seconds < self.attack_seconds {
            return time_seconds / self.attack_seconds;
        }
        let decay_time = time_seconds - self.attack_seconds;
        if self.decay_seconds > 0.0 && decay_time < self.decay_seconds {
            let progress = decay_time / self.decay_seconds;
            return 1.0 - (1.0 - self.sustain_level) * progress;
        }
        self.sustain_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_attacks_decays_sustains_and_releases() {
        let envelope = Envelope {
            attack_seconds: 0.1,
            decay_seconds: 0.1,
            sustain_level: 0.5,
            release_seconds: 0.2,
        };
        assert_eq!(envelope.amplitude(0.0, 0.3), 0.0);
        assert_eq!(envelope.amplitude(0.05, 0.3), 0.5);
        assert_eq!(envelope.amplitude(0.1, 0.3), 1.0);
        assert_eq!(envelope.amplitude(0.15, 0.3), 0.75);
        assert_eq!(envelope.amplitude(0.25, 0.3), 0.5);
        assert_eq!(envelope.amplitude(0.4, 0.3), 0.25);
        assert_eq!(envelope.amplitude(0.5, 0.3), 0.0);
    }

    #[test]
    fn short_gate_releases_from_the_interrupted_attack_level() {
        let envelope = Envelope {
            attack_seconds: 1.0,
            decay_seconds: 0.0,
            sustain_level: 1.0,
            release_seconds: 0.5,
        };
        assert_eq!(envelope.amplitude(0.25, 0.25), 0.25);
        assert_eq!(envelope.amplitude(0.5, 0.25), 0.125);
    }

    #[test]
    fn inversion_reverses_the_active_shape_and_stops_after_release() {
        let envelope = Envelope {
            attack_seconds: 0.1,
            decay_seconds: 0.1,
            sustain_level: 0.5,
            release_seconds: 0.2,
        };
        for time in [0.0, 0.05, 0.1, 0.15, 0.25, 0.3, 0.4] {
            assert_eq!(
                envelope.amplitude_with_inversion(time, 0.3, true),
                1.0 - envelope.amplitude(time, 0.3)
            );
        }
        assert_eq!(envelope.amplitude_with_inversion(0.5, 0.3, true), 0.0);
        assert_eq!(envelope.amplitude_with_inversion(0.6, 0.3, true), 0.0);
    }
}
