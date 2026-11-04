use evil_synth::StereoSound;

use crate::{
    DrumHit, DrumKit, Error, PAD_COUNT, PreparedPad,
    routing::{BusMixer, VoiceMix},
    voice::{PlaybackVoice, VOICE_LIMIT, allocate_voice},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KitHit {
    pub pad_index: usize,
    pub start_seconds: f64,
    /// An optional held-note gate. `None` uses the pad's configured gate.
    pub duration_seconds: Option<f64>,
    pub velocity: u8,
}

/// Render a complete kit through the same voice, playback and stereo panning
/// model used by live performance.
pub fn render_kit(kit: &DrumKit, hits: &[KitHit], sample_rate: u32) -> Result<StereoSound, Error> {
    kit.validate()?;
    if !(8_000..=192_000).contains(&sample_rate) {
        return Err(Error::InvalidKit(
            "render sample rate must be 8000..=192000".to_owned(),
        ));
    }
    let pads: Vec<Option<PreparedPad>> = kit
        .pads
        .iter()
        .map(|pad| pad.as_ref().map(PreparedPad::new).transpose())
        .collect::<Result<_, _>>()?;
    let mut buses = BusMixer::new(kit.buses);

    let mut scheduled: Vec<(usize, usize, PlaybackVoice)> = Vec::with_capacity(hits.len());
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
        let Some(pad) = &pads[hit.pad_index] else {
            continue;
        };
        if pad.muted {
            continue;
        }
        let voice = pad.playback(
            hit.pad_index,
            sample_rate,
            hit.velocity,
            hit.duration_seconds,
        );
        let duration = voice.duration_seconds().ok_or(Error::UngatedLoop(index))?;
        if hit.start_seconds + duration > crate::MAX_RENDER_SECONDS {
            return Err(Error::RenderTooLong);
        }
        let start_frame = (hit.start_seconds * f64::from(sample_rate)).round() as usize;
        scheduled.push((start_frame, index, voice));
    }
    scheduled.sort_by_key(|(start, index, _)| (*start, *index));

    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut active: Vec<PlaybackVoice> = Vec::with_capacity(VOICE_LIMIT);
    let mut scheduled = scheduled.into_iter().peekable();
    while !active.is_empty() || scheduled.peek().is_some() {
        active.retain(|voice| !voice.is_finished());
        if active.is_empty()
            && buses.is_quiet()
            && let Some((start, _, _)) = scheduled.peek()
            && *start > left.len()
        {
            left.resize(*start, 0.0);
            right.resize(*start, 0.0);
        }
        let frame = left.len();
        if frame as f64 >= crate::MAX_RENDER_SECONDS * f64::from(sample_rate) {
            return Err(Error::RenderTooLong);
        }
        while scheduled
            .peek()
            .is_some_and(|(start, _, _)| *start == frame)
        {
            let (_, _, voice) = scheduled.next().unwrap();
            allocate_voice(&mut active, voice);
        }
        let mut voice_mix = VoiceMix::default();
        for voice in &mut active {
            if let Some(sample) = voice.next_sample() {
                voice_mix.add(pads[voice.slot].as_ref().unwrap(), sample);
            }
        }
        let mixed = buses.mix(voice_mix, kit.gain, sample_rate);
        left.push(mixed[0]);
        right.push(mixed[1]);
        active.retain(|voice| !voice.is_finished());
    }

    const MAX_BUS_TAIL_SECONDS: u32 = 10;
    let tail_limit = (MAX_BUS_TAIL_SECONDS * sample_rate) as usize;
    let mut tail_frames = 0;
    while !buses.is_quiet() && tail_frames < tail_limit {
        if left.len() as f64 >= crate::MAX_RENDER_SECONDS * f64::from(sample_rate) {
            return Err(Error::RenderTooLong);
        }
        let mixed = buses.mix(VoiceMix::default(), kit.gain, sample_rate);
        left.push(mixed[0]);
        right.push(mixed[1]);
        tail_frames += 1;
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

    #[test]
    fn choke_groups_and_the_shared_voice_limit_apply_across_pads() {
        let mut kit = DrumKit::empty("Groups");
        kit.gain = 1.0;
        for (slot, group) in [(0, 1), (1, 1), (2, 2)] {
            let mut pad = KitPad::new(DrumPad::new(vec![1.0; 64], 48_000));
            pad.voice.gain = if slot == 1 { 0.5 } else { 1.0 };
            pad.voice.envelope.release_seconds = 0.0;
            pad.pan = -1.0;
            pad.choke_group = group;
            kit.pads[slot] = Some(pad);
        }
        let hit = |pad_index, start_seconds| KitHit {
            pad_index,
            start_seconds,
            duration_seconds: None,
            velocity: 127,
        };
        let output = render_kit(
            &kit,
            &[hit(0, 0.0), hit(2, 0.0), hit(1, 2.0 / 48_000.0)],
            48_000,
        )
        .unwrap();
        let left_gain = 1.0;
        assert!((output.left[0] - 2.0 * left_gain).abs() < 1e-6);
        assert!((output.left[1] - 2.0 * left_gain).abs() < 1e-6);
        assert!((output.left[2] - 1.5 * left_gain).abs() < 1e-6);

        let mut short_tail = kit.clone();
        short_tail.pads[2] = None;
        short_tail.pads[1].as_mut().unwrap().gate_seconds = Some(4.0 / 48_000.0);
        let choked =
            render_kit(&short_tail, &[hit(0, 0.0), hit(1, 2.0 / 48_000.0)], 48_000).unwrap();
        assert_eq!(choked.left.len(), 6);

        let hits: Vec<_> = (0..VOICE_LIMIT + 1)
            .map(|index| hit(index % 2, 0.0))
            .collect();
        let limited = render_kit(&kit, &hits, 48_000).unwrap();
        // Each group-1 hit chokes its predecessor, leaving the final full-gain voice.
        assert!((limited.left[0] - left_gain).abs() < 1e-6);
        kit.pads[2].as_mut().unwrap().choke_group = 0;
        let independent: Vec<_> = (0..VOICE_LIMIT + 1).map(|_| hit(2, 0.0)).collect();
        let limited = render_kit(&kit, &independent, 48_000).unwrap();
        assert!((limited.left[0] - VOICE_LIMIT as f32 * left_gain).abs() < 1e-5);
    }

    #[test]
    fn unfiltered_sends_route_post_pad_gain_to_three_buses() {
        let mut kit = DrumKit::empty("Sends");
        kit.gain = 0.5;
        let mut pad = KitPad::new(DrumPad::new(vec![1.0], 48_000));
        pad.voice.gain = 0.8;
        pad.voice.envelope.release_seconds = 0.0;
        pad.pan = -1.0;
        pad.bus_sends = [0.5, 0.25, 0.125];
        kit.pads[0] = Some(pad);
        kit.buses[0].gain = 0.5;
        kit.buses[0].pan = -1.0;
        kit.buses[1].pan = 1.0;
        kit.buses[2].gain = 0.5;
        let output = render_kit(
            &kit,
            &[KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            }],
            48_000,
        )
        .unwrap();
        let center = std::f32::consts::FRAC_PI_4.cos();
        assert_eq!(output.left.len(), 1);
        assert!((output.left[0] - (0.5 + 0.025 * center)).abs() < 1e-6);
        assert!((output.right[0] - (0.1 + 0.025 * center)).abs() < 1e-6);

        kit.buses[1].muted = true;
        let muted = render_kit(
            &kit,
            &[KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 127,
            }],
            48_000,
        )
        .unwrap();
        assert!((muted.right[0] - 0.025 * center).abs() < 1e-6);
    }

    #[test]
    fn pad_and_bus_filters_match_live_with_deterministic_bounded_tails() {
        use crate::{DrumFilter, DrumMachine};
        use evil_synth::FilterMode;

        let mut kit = DrumKit::empty("Filtered routing");
        kit.gain = 0.9;
        let mut pad = KitPad::new(DrumPad::new(vec![1.0, 0.5, -0.25, 0.0], 48_000));
        pad.voice.gain = 0.7;
        pad.voice.envelope.release_seconds = 0.0;
        pad.voice.filter = Some(DrumFilter {
            mode: FilterMode::LowPass,
            cutoff_hz: 1_000.0,
            resonance: 0.2,
            stages: 2,
        });
        pad.bus_sends = [1.0, 0.5, 0.0];
        kit.pads[0] = Some(pad);
        kit.buses[0].filter = Some(DrumFilter {
            mode: FilterMode::HighPass,
            cutoff_hz: 800.0,
            resonance: 0.3,
            stages: 4,
        });
        kit.buses[0].gain = 0.7;
        kit.buses[0].pan = 1.0;
        kit.buses[1].filter = Some(DrumFilter {
            mode: FilterMode::LowPass,
            cutoff_hz: 600.0,
            resonance: 0.1,
            stages: 2,
        });
        kit.buses[1].gain = 0.4;
        kit.buses[1].pan = -1.0;
        let hits = [
            KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 100,
            },
            KitHit {
                pad_index: 0,
                start_seconds: 8.0 / 48_000.0,
                duration_seconds: None,
                velocity: 80,
            },
        ];
        let offline = render_kit(&kit, &hits, 48_000).unwrap();
        let repeated = render_kit(&kit, &hits, 48_000).unwrap();
        assert_eq!(offline.left, repeated.left);
        assert_eq!(offline.right, repeated.right);
        assert!(offline.left.len() > 12);
        assert!(offline.left.len() < 48_000);
        assert!(offline.left.iter().all(|sample| sample.is_finite()));
        assert!(offline.right.iter().all(|sample| sample.is_finite()));

        let mut live = DrumMachine::default();
        live.set_gain(kit.gain);
        live.set_buses(kit.buses);
        live.set_pad(
            0,
            Some(PreparedPad::new(kit.pads[0].as_ref().unwrap()).unwrap()),
        );
        live.set_input(1, 100, None, 120.0).unwrap();
        for frame in 0..offline.left.len() {
            if frame == 8 {
                live.set_input(0, 80, None, 120.0).unwrap();
                live.set_input(1, 80, None, 120.0).unwrap();
            }
            let actual = live.next_frame().map(f32::atanh);
            assert!((actual[0] - offline.left[frame]).abs() < 1e-5);
            assert!((actual[1] - offline.right[frame]).abs() < 1e-5);
        }
    }

    #[test]
    fn trimmed_reverse_loops_match_live_playback_sample_for_sample() {
        use crate::{DrumMachine, SampleDirection};

        let mut kit = DrumKit::empty("Live parity");
        kit.gain = 1.0;
        let mut pad = KitPad::new(DrumPad::new(vec![0.0, 0.25, 0.5, 0.75, 1.0], 48_000));
        pad.voice.gain = 0.8;
        pad.voice.envelope.release_seconds = 0.0;
        pad.voice.playback_start_frame = 1;
        pad.voice.playback_end_frame = Some(4);
        pad.voice.direction = SampleDirection::Reverse;
        pad.voice.loop_enabled = true;
        pad.gate_seconds = Some(8.0 / 48_000.0);
        pad.pan = -1.0;
        kit.pads[0] = Some(pad);
        let offline = render_kit(
            &kit,
            &[KitHit {
                pad_index: 0,
                start_seconds: 0.0,
                duration_seconds: None,
                velocity: 100,
            }],
            48_000,
        )
        .unwrap();

        let mut live = DrumMachine::default();
        live.set_gain(kit.gain);
        live.set_pad(
            0,
            Some(PreparedPad::new(kit.pads[0].as_ref().unwrap()).unwrap()),
        );
        live.set_input(1, 100, None, 120.0).unwrap();
        for expected in offline.left {
            let actual = live.next_frame()[0].atanh();
            assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
        }
    }
}
