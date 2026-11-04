use serde::{Deserialize, Serialize};

use crate::{Error, Patch};

/// Symmetric drive and contour shaping for the first instrument insert slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Waveshaper {
    /// Input drive from clean (1) to saturated (32).
    pub drive: f32,
    /// Softer (-1), smooth (0), or hard-clipped (1) symmetric contour.
    pub contour: f32,
    /// Output level after shaping.
    pub level: f32,
}

impl Default for Waveshaper {
    fn default() -> Self {
        Self {
            drive: 6.0,
            contour: 0.0,
            level: 0.8,
        }
    }
}

impl Waveshaper {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.drive.is_finite()
            || !(1.0..=32.0).contains(&self.drive)
            || !self.contour.is_finite()
            || !(-1.0..=1.0).contains(&self.contour)
            || !self.level.is_finite()
            || !(0.0..=1.0).contains(&self.level)
        {
            return Err(Error::InvalidPatch(
                "waveshaper requires drive in 1..=32, contour in -1..=1, and level in 0..=1"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn process(&self, input: f32) -> f32 {
        let driven = input * self.drive;
        let smooth = driven.tanh() / self.drive.tanh();
        let shaped = if self.contour >= 0.0 {
            let hard = driven.clamp(-1.0, 1.0) / self.drive.tanh();
            smooth + (hard - smooth) * self.contour
        } else {
            let soft = driven / (1.0 + driven.abs()) / (self.drive / (1.0 + self.drive));
            smooth + (soft - smooth) * -self.contour
        };
        (shaped * self.level).clamp(-1.0, 1.0)
    }
}

pub(crate) fn apply_waveshaper(patch: &Patch, samples: &mut [f32]) {
    if let Some(waveshaper) = &patch.waveshaper {
        for sample in samples {
            *sample = waveshaper.process(*sample);
        }
    }
}

pub(crate) fn apply_stereo_waveshaper(
    left: &mut [f32],
    right: &mut [f32],
    waveshaper: &Waveshaper,
) {
    debug_assert_eq!(left.len(), right.len());
    for (left, right) in left.iter_mut().zip(right) {
        *left = waveshaper.process(*left);
        *right = waveshaper.process(*right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveshaper_is_symmetric_bounded_and_responds_to_controls() {
        for contour in [-1.0, 0.0, 1.0] {
            let shaper = Waveshaper {
                contour,
                ..Waveshaper::default()
            };
            shaper.validate().unwrap();
            let positive = shaper.process(0.5);
            let negative = shaper.process(-0.5);
            assert_eq!(positive, -negative);
            assert!(positive.is_finite() && positive.abs() <= 1.0);
            assert_ne!(positive, 0.5);
        }
        assert_eq!(Waveshaper::default().process(0.0), 0.0);
        assert_eq!(
            Waveshaper {
                level: 0.0,
                ..Waveshaper::default()
            }
            .process(0.5),
            0.0
        );
        for drive in [0.0, 33.0, f32::NAN] {
            assert!(
                Waveshaper {
                    drive,
                    ..Waveshaper::default()
                }
                .validate()
                .is_err()
            );
        }
    }
}
