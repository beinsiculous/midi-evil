use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use evil_synth::{ChannelMeter, METER_BLOCK_FRAMES, StereoSound};
use rodio::{DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, Player, Source};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterSnapshot {
    pub level: [f32; 2],
    pub peak: [f32; 2],
}

#[derive(Default)]
struct MeterState {
    master: MeterSnapshot,
    channels: Vec<(usize, MeterSnapshot)>,
}

#[derive(Clone, Default)]
struct PreviewMeter {
    state: Arc<Mutex<MeterState>>,
}

impl PreviewMeter {
    fn with_channels(channel_meters: &[(usize, ChannelMeter)]) -> Self {
        Self {
            state: Arc::new(Mutex::new(MeterState {
                master: MeterSnapshot::default(),
                channels: channel_meters
                    .iter()
                    .map(|(track, _)| (*track, MeterSnapshot::default()))
                    .collect(),
            })),
        }
    }

    fn push_block(
        &self,
        master: [f32; 2],
        channel_meters: &[(usize, ChannelMeter)],
        block_index: usize,
    ) {
        let mut state = self.state.lock().expect("preview meter lock");
        update_snapshot(&mut state.master, master);
        for ((_, snapshot), (_, meter)) in state.channels.iter_mut().zip(channel_meters) {
            let peak = meter
                .meter_blocks
                .get(block_index)
                .copied()
                .unwrap_or([0.0; 2]);
            update_snapshot(snapshot, peak);
        }
    }

    fn get(&self) -> MeterSnapshot {
        self.state.lock().expect("preview meter lock").master
    }

    fn channels(&self) -> Vec<(usize, MeterSnapshot)> {
        self.state
            .lock()
            .expect("preview meter lock")
            .channels
            .clone()
    }
}

fn update_snapshot(snapshot: &mut MeterSnapshot, block_peak: [f32; 2]) {
    for (channel, block_peak) in block_peak.into_iter().enumerate() {
        snapshot.level[channel] = block_peak.max(snapshot.level[channel] * 0.65);
        snapshot.peak[channel] = snapshot.peak[channel].max(block_peak);
    }
}

pub struct PreviewPlayer {
    _stream: MixerDeviceSink,
    player: Player,
    meter: PreviewMeter,
}

impl PreviewPlayer {
    pub fn play_with_meters(
        sound: StereoSound,
        channel_meters: Vec<(usize, ChannelMeter)>,
        looping: bool,
    ) -> Result<Self, DeviceSinkError> {
        Self::play_with_loop(sound, looping, channel_meters)
    }

    fn play_with_loop(
        sound: StereoSound,
        looping: bool,
        channel_meters: Vec<(usize, ChannelMeter)>,
    ) -> Result<Self, DeviceSinkError> {
        let mut stream = DeviceSinkBuilder::open_default_sink()?;
        stream.log_on_drop(false);
        let player = Player::connect_new(stream.mixer());
        let meter = PreviewMeter::with_channels(&channel_meters);
        player.append(StereoSource::new_with_loop_and_meters(
            sound,
            looping,
            meter.clone(),
            channel_meters,
        ));
        Ok(Self {
            _stream: stream,
            player,
            meter,
        })
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }

    pub fn position(&self) -> Duration {
        self.player.get_pos()
    }

    pub fn meter(&self) -> MeterSnapshot {
        self.meter.get()
    }

    pub fn channel_meters(&self) -> Vec<(usize, MeterSnapshot)> {
        self.meter.channels()
    }
}

struct StereoSource {
    left: Vec<f32>,
    right: Vec<f32>,
    sample_rate: u32,
    sample_index: usize,
    looping: bool,
    meter: PreviewMeter,
    channel_meters: Vec<(usize, ChannelMeter)>,
    block_peak: [f32; 2],
    block_frames: usize,
    meter_block_index: usize,
}

impl StereoSource {
    #[cfg(test)]
    fn new_with_loop(sound: StereoSound, looping: bool, meter: PreviewMeter) -> Self {
        Self::new_with_loop_and_meters(sound, looping, meter, Vec::new())
    }

    fn new_with_loop_and_meters(
        sound: StereoSound,
        looping: bool,
        meter: PreviewMeter,
        channel_meters: Vec<(usize, ChannelMeter)>,
    ) -> Self {
        debug_assert_eq!(sound.left.len(), sound.right.len());
        Self {
            left: sound.left,
            right: sound.right,
            sample_rate: sound.sample_rate,
            sample_index: 0,
            looping,
            meter,
            channel_meters,
            block_peak: [0.0; 2],
            block_frames: 0,
            meter_block_index: 0,
        }
    }

    fn push_meter_frame(&mut self, frame: usize) {
        self.block_peak[0] = self.block_peak[0].max(self.left[frame].abs());
        self.block_peak[1] = self.block_peak[1].max(self.right[frame].abs());
        self.block_frames += 1;
        if self.block_frames == METER_BLOCK_FRAMES {
            self.flush_meter();
        }
    }

    fn flush_meter(&mut self) {
        if self.block_frames > 0 {
            self.meter.push_block(
                self.block_peak,
                &self.channel_meters,
                self.meter_block_index,
            );
            self.block_peak = [0.0; 2];
            self.block_frames = 0;
            self.meter_block_index += 1;
        }
    }
}

impl Iterator for StereoSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.sample_index / 2;
        let left_channel = self.sample_index.is_multiple_of(2);
        let sample = if left_channel {
            self.left.get(frame)
        } else {
            self.right.get(frame)
        }
        .copied();
        if sample.is_some() {
            self.sample_index += 1;
            if !left_channel {
                self.push_meter_frame(frame);
            }
        } else if self.looping && !self.left.is_empty() {
            self.flush_meter();
            self.sample_index = 0;
            self.meter_block_index = 0;
            return self.next();
        } else {
            self.flush_meter();
        }
        sample
    }
}

impl Source for StereoSource {
    fn current_span_len(&self) -> Option<usize> {
        if self.looping {
            None
        } else if self.sample_index >= self.left.len() + self.right.len() {
            Some(0)
        } else {
            None
        }
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).expect("stereo has two channels")
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(self.sample_rate).expect("validated sample rates are nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        if self.looping {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.left.len() as f64 / f64::from(self.sample_rate),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_source_interleaves_channels_and_reports_duration() {
        let meter = PreviewMeter::default();
        let mut source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.1, 0.2],
                right: vec![-0.1, -0.2],
                sample_rate: 48_000,
            },
            false,
            meter.clone(),
        );
        assert_eq!(source.channels().get(), 2);
        assert_eq!(source.sample_rate().get(), 48_000);
        assert_eq!(source.next(), Some(0.1));
        assert_eq!(source.next(), Some(-0.1));
        assert_eq!(source.next(), Some(0.2));
        assert_eq!(source.next(), Some(-0.2));
        assert_eq!(source.next(), None);
        assert!(source.is_exhausted());
        assert_eq!(meter.get().peak, [0.2, 0.2]);
        assert_eq!(meter.get().level, [0.2, 0.2]);
        let source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.0; 48_000],
                right: vec![0.0; 48_000],
                sample_rate: 48_000,
            },
            false,
            PreviewMeter::default(),
        );
        assert_eq!(source.total_duration(), Some(Duration::from_secs(1)));
    }

    #[test]
    fn native_mixer_edits_render_and_feed_preview_meters_without_an_audio_device() {
        use evil_project::{Note, Project, Track};
        use evil_synth::{Bitcrush, Chorus, Delay, Equalizer, InsertEffect, Reverb};

        let mut project = Project::new("Mixer preview workflow");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project.master_gain = 1.0;
        project.instruments[0].gain = 1.0;
        project.instruments[0].pan = -0.25;
        project.instruments[0].inserts = vec![
            InsertEffect::Bitcrush(Bitcrush::default()),
            InsertEffect::Equalizer(Equalizer::default()),
            InsertEffect::Chorus(Chorus::default()),
            InsertEffect::Reverb(Reverb::default()),
        ];
        project.instruments[0].send_gains = [0.5, 0.25];
        project.send_buses[0].effect = Some(InsertEffect::Delay(Delay::default()));
        project.send_buses[0].gain = 0.5;
        project.send_buses[1].effect = Some(InsertEffect::Equalizer(Equalizer::default()));
        project.send_buses[1].gain = 0.25;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mixer.evil");
        project.save(&path).unwrap();
        let loaded = Project::load(&path).unwrap();
        assert_eq!(
            loaded.instruments[0].inserts,
            project.instruments[0].inserts
        );
        assert_eq!(loaded.send_buses, project.send_buses);
        let rendered = evil_render::render_project(&loaded, directory.path()).unwrap();
        assert_eq!(rendered.note_count, 1);
        assert_eq!(rendered.track_count, 1);
        assert!(rendered.mix_analysis.pre_saturator_peak[0] > 0.0);

        let expected_peak = [
            rendered
                .sound
                .left
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs())),
            rendered
                .sound
                .right
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs())),
        ];
        let meter = PreviewMeter::with_channels(&rendered.channel_meters);
        let expected_channels = rendered.channel_meters.clone();
        let mut source = StereoSource::new_with_loop_and_meters(
            rendered.sound,
            false,
            meter.clone(),
            rendered.channel_meters,
        );
        let sample_count = source.left.len() * 2;
        assert_eq!(source.by_ref().count(), sample_count);
        assert_eq!(meter.get().peak, expected_peak);
        let channels = meter.channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].0, 0);
        assert_eq!(channels[0].1.peak, expected_channels[0].1.pre_sum_peak);
    }

    #[test]
    fn looping_source_restarts_at_the_section_boundary() {
        let source = StereoSource::new_with_loop(
            StereoSound {
                left: vec![0.1, 0.2],
                right: vec![-0.1, -0.2],
                sample_rate: 48_000,
            },
            true,
            PreviewMeter::default(),
        );
        let samples: Vec<_> = source.take(8).collect();
        assert_eq!(samples, [0.1, -0.1, 0.2, -0.2, 0.1, -0.1, 0.2, -0.2]);
    }
}
