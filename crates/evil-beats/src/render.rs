use evil_synth::StereoSound;

use crate::{DrumHit, DrumKit, Error, PAD_COUNT};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KitHit {
    pub pad_index: usize,
    pub start_seconds: f64,
    /// An optional held-note gate. `None` uses the pad's configured gate.
    pub duration_seconds: Option<f64>,
    pub velocity: u8,
}

/// Render a complete kit through the same per-pad voice and stereo panning
/// model used by live performance.
pub fn render_kit(kit: &DrumKit, hits: &[KitHit], sample_rate: u32) -> Result<StereoSound, Error> {
    kit.validate()?;
    if !(8_000..=192_000).contains(&sample_rate) {
        return Err(Error::InvalidKit(
            "render sample rate must be 8000..=192000".to_owned(),
        ));
    }
    let mut pad_hits: [Vec<DrumHit>; PAD_COUNT] = std::array::from_fn(|_| Vec::new());
    for (index, hit) in hits.iter().enumerate() {
        if hit.pad_index >= PAD_COUNT {
            return Err(Error::InvalidHit(index));
        }
        DrumHit {
            start_seconds: hit.start_seconds,
            duration_seconds: hit.duration_seconds,
            velocity: hit.velocity,
        }
        .validate(index)?;
        pad_hits[hit.pad_index].push(DrumHit {
            start_seconds: hit.start_seconds,
            duration_seconds: hit.duration_seconds.or_else(|| {
                kit.pads[hit.pad_index]
                    .as_ref()
                    .and_then(|pad| pad.gate_seconds)
            }),
            velocity: hit.velocity,
        });
    }

    let mut rendered = Vec::new();
    let mut frame_count = 0_usize;
    for (pad_index, hits) in pad_hits.iter().enumerate() {
        let Some(pad) = &kit.pads[pad_index] else {
            continue;
        };
        if hits.is_empty() || pad.muted {
            continue;
        }
        let source = pad.voice.render(hits)?;
        let source_rate = source.sample_rate;
        let frames = (source.samples.len() as u64 * u64::from(sample_rate))
            .div_ceil(u64::from(source_rate)) as usize;
        frame_count = frame_count.max(frames);
        rendered.push((source, pad.pan));
    }

    let mut left = vec![0.0; frame_count];
    let mut right = vec![0.0; frame_count];
    for (source, pan) in rendered {
        let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let left_gain = angle.cos() * kit.gain;
        let right_gain = angle.sin() * kit.gain;
        let ratio = f64::from(source.sample_rate) / f64::from(sample_rate);
        for frame in 0..frame_count {
            let position = frame as f64 * ratio;
            let index = position.floor() as usize;
            if index >= source.samples.len() {
                break;
            }
            let next = (index + 1).min(source.samples.len() - 1);
            let sample = source.samples[index]
                + (source.samples[next] - source.samples[index]) * position.fract() as f32;
            left[frame] += sample * left_gain;
            right[frame] += sample * right_gain;
        }
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
    use crate::{DrumPad, KitPad};

    #[test]
    fn kit_render_maps_hits_to_stereo_pads_and_resamples() {
        let mut kit = DrumKit::empty("Render");
        let mut left_pad = KitPad::new(DrumPad::new(vec![0.5; 480], 48_000));
        left_pad.voice.envelope.release_seconds = 0.0;
        left_pad.pan = -1.0;
        kit.pads[0] = Some(left_pad);
        let mut right_pad = KitPad::new(DrumPad::new(vec![0.25; 240], 24_000));
        right_pad.voice.envelope.release_seconds = 0.0;
        right_pad.pan = 1.0;
        kit.pads[1] = Some(right_pad);
        let hits = [
            KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            },
            KitHit {
                pad_index: 1,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            },
            KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            },
        ];
        let first = render_kit(&kit, &hits, 48_000).unwrap();
        let second = render_kit(&kit, &hits, 48_000).unwrap();
        assert_eq!(first.left, second.left);
        assert_eq!(first.right, second.right);
        assert!(first.left[0] > first.right[0]);
        assert!(first.right[0] > 0.0);
        let downsampled = render_kit(&kit, &hits, 24_000).unwrap();
        assert_eq!(downsampled.sample_rate, 24_000);
        assert!(downsampled.left.len() < first.left.len());
    }

    #[test]
    fn kit_render_honors_mute_and_rejects_unknown_pads() {
        let mut kit = DrumKit::empty("Muted");
        let mut pad = KitPad::new(DrumPad::new(vec![0.5; 10], 48_000));
        pad.muted = true;
        kit.pads[0] = Some(pad);
        let hit = KitHit {
            pad_index: 0,
            start_seconds: 0.0,
            duration_seconds: None,
            velocity: 127,
        };
        let output = render_kit(&kit, &[hit], 48_000).unwrap();
        assert!(output.left.is_empty());
        assert!(
            render_kit(
                &kit,
                &[KitHit {
                    pad_index: 16,
                    ..hit
                }],
                48_000
            )
            .is_err()
        );
    }
}
