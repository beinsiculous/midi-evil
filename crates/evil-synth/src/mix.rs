use crate::delay::apply_stereo_delay;
use crate::render::block_dc;
use crate::{Delay, Error, RenderedSound};

pub struct MonoTrack<'a> {
    pub sound: &'a RenderedSound,
    pub gain: f32,
    /// Stereo position from -1 (left) to 1 (right).
    pub pan: f32,
    pub delay: Option<&'a Delay>,
    /// Remove DC introduced by oscillator sync and nonlinear limiting.
    pub dc_block: bool,
}

pub struct StereoSound {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

pub struct StereoTrack<'a> {
    pub sound: &'a StereoSound,
    pub gain: f32,
    /// Stereo balance from -1 (left) to 1 (right).
    pub pan: f32,
}

pub fn mix_stereo(tracks: &[MonoTrack<'_>], master_gain: f32) -> Result<StereoSound, Error> {
    mix_stereo_with_tracks(tracks, &[], master_gain)
}

pub fn mix_stereo_with_tracks(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    master_gain: f32,
) -> Result<StereoSound, Error> {
    if tracks.is_empty() && stereo_tracks.is_empty() {
        return Err(Error::InvalidMix(
            "a mix needs at least one track".to_owned(),
        ));
    }
    if !master_gain.is_finite() || !(0.0..=1.0).contains(&master_gain) {
        return Err(Error::InvalidMix(
            "master gain must be between 0 and 1".to_owned(),
        ));
    }
    let sample_rate = tracks.first().map_or_else(
        || stereo_tracks[0].sound.sample_rate,
        |track| track.sound.sample_rate,
    );
    let frame_count = tracks
        .iter()
        .map(|track| track.sound.samples.len())
        .chain(stereo_tracks.iter().map(|track| track.sound.left.len()))
        .max()
        .unwrap_or(0);
    let mut left = vec![0.0; frame_count];
    let mut right = vec![0.0; frame_count];

    for track in tracks {
        if track.sound.sample_rate != sample_rate {
            return Err(Error::InvalidMix(
                "all tracks must use the same sample rate".to_owned(),
            ));
        }
        if !track.gain.is_finite() || !(0.0..=1.0).contains(&track.gain) {
            return Err(Error::InvalidMix(
                "track gain must be between 0 and 1".to_owned(),
            ));
        }
        if !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan) {
            return Err(Error::InvalidMix(
                "track pan must be between -1 and 1".to_owned(),
            ));
        }
        let angle = (track.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let left_gain = angle.cos() * track.gain * master_gain;
        let right_gain = angle.sin() * track.gain * master_gain;
        let mut track_left = vec![0.0; track.sound.samples.len()];
        let mut track_right = vec![0.0; track.sound.samples.len()];
        for (frame, sample) in track.sound.samples.iter().enumerate() {
            track_left[frame] = sample * left_gain;
            track_right[frame] = sample * right_gain;
        }
        if let Some(delay) = track.delay {
            delay.validate()?;
            apply_stereo_delay(&mut track_left, &mut track_right, sample_rate, delay);
        }
        if left.len() < track_left.len() {
            left.resize(track_left.len(), 0.0);
            right.resize(track_left.len(), 0.0);
        }
        for frame in 0..track_left.len() {
            left[frame] += track_left[frame];
            right[frame] += track_right[frame];
        }
    }
    for track in stereo_tracks {
        if track.sound.sample_rate != sample_rate
            || track.sound.left.len() != track.sound.right.len()
        {
            return Err(Error::InvalidMix(
                "stereo tracks must have matching channels and sample rates".to_owned(),
            ));
        }
        if !track.gain.is_finite() || !(0.0..=1.0).contains(&track.gain) {
            return Err(Error::InvalidMix(
                "track gain must be between 0 and 1".to_owned(),
            ));
        }
        if !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan) {
            return Err(Error::InvalidMix(
                "track pan must be between -1 and 1".to_owned(),
            ));
        }
        let left_balance = if track.pan < 0.0 {
            1.0
        } else {
            1.0 - track.pan
        };
        let right_balance = if track.pan > 0.0 {
            1.0
        } else {
            1.0 + track.pan
        };
        for frame in 0..track.sound.left.len() {
            left[frame] += track.sound.left[frame] * track.gain * master_gain * left_balance;
            right[frame] += track.sound.right[frame] * track.gain * master_gain * right_balance;
        }
    }
    for sample in left.iter_mut().chain(right.iter_mut()) {
        *sample = sample.tanh();
    }
    if tracks.iter().any(|track| track.dc_block) {
        block_dc(&mut left, sample_rate);
        block_dc(&mut right, sample_rate);
    }
    Ok(StereoSound {
        left,
        right,
        sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_different_lengths_with_equal_power_panning() {
        let short = RenderedSound {
            samples: vec![0.5; 100],
            sample_rate: 48_000,
        };
        let long = RenderedSound {
            samples: vec![0.25; 200],
            sample_rate: 48_000,
        };
        let stereo = mix_stereo(
            &[
                MonoTrack {
                    sound: &short,
                    gain: 1.0,
                    pan: -1.0,
                    delay: None,
                    dc_block: false,
                },
                MonoTrack {
                    sound: &long,
                    gain: 1.0,
                    pan: 1.0,
                    delay: None,
                    dc_block: false,
                },
            ],
            1.0,
        )
        .unwrap();
        assert_eq!(stereo.left.len(), 200);
        assert_eq!(stereo.right.len(), 200);
        assert!(stereo.right[100].abs() > 0.01);
        assert!(stereo.left[100].abs() < 0.001);
        assert!(
            stereo
                .left
                .iter()
                .chain(&stereo.right)
                .all(|sample| sample.is_finite())
        );
    }

    #[test]
    fn stereo_cross_delay_moves_echoes_between_channels() {
        let sound = RenderedSound {
            samples: std::iter::once(1.0)
                .chain(std::iter::repeat_n(0.0, 99))
                .collect(),
            sample_rate: 1_000,
        };
        let delay = Delay {
            time_seconds: 0.05,
            feedback: 0.5,
            mix: 1.0,
            mode: crate::DelayMode::StereoCross,
        };
        let stereo = mix_stereo(
            &[MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: -1.0,
                delay: Some(&delay),
                dc_block: false,
            }],
            1.0,
        )
        .unwrap();

        assert!(stereo.right[50] > 0.5);
        assert!(stereo.left[50].abs() < 0.001);
        assert!(stereo.left[100] > 0.2);
        assert!(stereo.right[100].abs() < 0.001);
    }

    #[test]
    fn stereo_only_mix_preserves_center_channels_and_applies_balance() {
        let sound = StereoSound {
            left: vec![0.5, 0.25],
            right: vec![0.25, 0.5],
            sample_rate: 48_000,
        };
        let centered = mix_stereo_with_tracks(
            &[],
            &[StereoTrack {
                sound: &sound,
                gain: 1.0,
                pan: 0.0,
            }],
            1.0,
        )
        .unwrap();
        assert!((centered.left[0] - 0.5_f32.tanh()).abs() < 1e-6);
        assert!((centered.right[0] - 0.25_f32.tanh()).abs() < 1e-6);
        let left = mix_stereo_with_tracks(
            &[],
            &[StereoTrack {
                sound: &sound,
                gain: 1.0,
                pan: -1.0,
            }],
            1.0,
        )
        .unwrap();
        assert!(left.right.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn rejects_mismatched_sample_rates() {
        let first = RenderedSound {
            samples: vec![0.0; 1],
            sample_rate: 48_000,
        };
        let second = RenderedSound {
            samples: vec![0.0; 1],
            sample_rate: 44_100,
        };
        assert!(
            mix_stereo(
                &[
                    MonoTrack {
                        sound: &first,
                        gain: 1.0,
                        pan: 0.0,
                        delay: None,
                        dc_block: false,
                    },
                    MonoTrack {
                        sound: &second,
                        gain: 1.0,
                        pan: 0.0,
                        delay: None,
                        dc_block: false,
                    },
                ],
                1.0,
            )
            .is_err()
        );
    }
}
