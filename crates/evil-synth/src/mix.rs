use std::collections::BTreeMap;

use crate::delay::apply_stereo_delay_with_tempo;
use crate::effects::{
    apply_insert_automated, apply_insert_with_tempo, effect_parameter, insert_is_bypassed,
    insert_tail_frames, set_effect_parameter,
};
use crate::insert_automation::{
    apply_stereo_delay_automation_with_tempo, apply_stereo_waveshaper_automation,
    has_delay_automation, has_waveshaper_automation,
};
use crate::render::block_dc;
use crate::waveshaper::apply_stereo_waveshaper;
use crate::{
    Delay, Error, INSERT_LIMIT, InsertEffect, RenderedSound, SEND_COUNT, SendBus, SynthController,
    SynthTempoChange, Waveshaper,
};
use evil_performance::MixerParameter;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynthMixerAutomation {
    pub time_seconds: f64,
    pub parameter: MixerParameter,
    pub value: f32,
}

pub struct MonoTrack<'a> {
    pub sound: &'a RenderedSound,
    pub gain: f32,
    /// Stereo position from -1 (left) to 1 (right).
    pub pan: f32,
    pub waveshaper: Option<&'a Waveshaper>,
    pub delay: Option<&'a Delay>,
    pub controllers: &'a [SynthController],
    pub automation: &'a [SynthMixerAutomation],
    /// Additional ordered channel inserts, after the legacy instrument inserts.
    pub inserts: &'a [InsertEffect],
    pub send_gains: [f32; SEND_COUNT],
    /// Remove DC introduced by oscillator sync and nonlinear limiting.
    pub dc_block: bool,
}

pub struct StereoSound {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

/// Master peaks and clipping measured before the final saturator.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixAnalysis {
    pub pre_saturator_peak: [f32; 2],
    pub clipped_frames: usize,
}

impl MixAnalysis {
    pub fn is_clipped(&self) -> bool {
        self.clipped_frames > 0
    }
}

pub struct AnalyzedMix {
    pub sound: StereoSound,
    pub analysis: MixAnalysis,
    pub channel_meters: Vec<ChannelMeter>,
    pub unsupported_mixer_automation_events: usize,
}

pub const METER_BLOCK_FRAMES: usize = 64;

/// Bounded post-insert channel levels aligned to the analyzed section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelMeter {
    pub pre_sum_peak: [f32; 2],
    pub clipped_frames: usize,
    pub meter_blocks: Vec<[f32; 2]>,
}

pub struct StereoTrack<'a> {
    pub sound: &'a StereoSound,
    pub gain: f32,
    /// Stereo balance from -1 (left) to 1 (right).
    pub pan: f32,
    pub automation: &'a [SynthMixerAutomation],
    pub inserts: &'a [InsertEffect],
    pub send_gains: [f32; SEND_COUNT],
}

#[derive(Default)]
struct ChannelAutomation {
    lanes: BTreeMap<MixerParameter, AutomationLane>,
}

struct AutomationLane {
    base: f32,
    events: Vec<SynthMixerAutomation>,
    next: usize,
}

impl AutomationLane {
    fn value_at(&mut self, time: f64) -> f32 {
        while self
            .events
            .get(self.next)
            .is_some_and(|event| event.time_seconds <= time)
        {
            self.next += 1;
        }
        if self.next == 0 {
            return self.base;
        }
        let previous = self.events[self.next - 1];
        let Some(next) = self.events.get(self.next) else {
            return previous.value;
        };
        if next.time_seconds <= time {
            return next.value;
        }
        let fraction = ((time - previous.time_seconds)
            / (next.time_seconds - previous.time_seconds))
            .clamp(0.0, 1.0) as f32;
        previous.value + (next.value - previous.value) * fraction
    }
}

impl ChannelAutomation {
    fn new(
        events: &[SynthMixerAutomation],
        inserts: &[InsertEffect],
        gain: f32,
        pan: f32,
        send_gains: [f32; SEND_COUNT],
    ) -> Result<(Self, usize), Error> {
        let mut grouped: BTreeMap<MixerParameter, Vec<SynthMixerAutomation>> = BTreeMap::new();
        let mut previous_time = 0.0;
        for event in events {
            if !event.time_seconds.is_finite()
                || event.time_seconds < previous_time
                || !event.parameter.is_valid_value(event.value)
            {
                return Err(Error::InvalidAutomation(
                    "mixer automation must be ordered, finite, and within its parameter range"
                        .to_owned(),
                ));
            }
            previous_time = event.time_seconds;
            grouped.entry(event.parameter).or_default().push(*event);
        }
        let mut automation = Self::default();
        let mut unsupported = 0;
        for (parameter, events) in grouped {
            let base = match parameter {
                MixerParameter::Gain => gain,
                MixerParameter::Pan => pan,
                MixerParameter::SendGain { bus } => send_gains[usize::from(bus)],
                MixerParameter::Insert { slot, parameter } => {
                    let Some(base) = inserts
                        .get(usize::from(slot))
                        .and_then(|effect| effect_parameter(effect, parameter))
                    else {
                        unsupported += events.len();
                        continue;
                    };
                    base
                }
            };
            automation.lanes.insert(
                parameter,
                AutomationLane {
                    base,
                    events,
                    next: 0,
                },
            );
        }
        Ok((automation, unsupported))
    }

    fn has_gain_or_pan(&self) -> bool {
        self.lanes.contains_key(&MixerParameter::Gain)
            || self.lanes.contains_key(&MixerParameter::Pan)
    }

    fn gain_and_pan(&mut self, time: f64, gain: f32, pan: f32) -> (f32, f32) {
        let gain = self
            .lanes
            .get_mut(&MixerParameter::Gain)
            .map_or(gain, |lane| lane.value_at(time));
        let pan = self
            .lanes
            .get_mut(&MixerParameter::Pan)
            .map_or(pan, |lane| lane.value_at(time));
        (gain, pan)
    }

    fn has_insert(&self, slot: usize) -> bool {
        self.lanes.keys().any(|parameter| {
            matches!(parameter, MixerParameter::Insert { slot: target, .. } if usize::from(*target) == slot)
        })
    }

    fn insert_tail_frames(
        &self,
        slot: usize,
        effect: &InsertEffect,
        sample_rate: u32,
        tempo_changes: &[SynthTempoChange],
    ) -> usize {
        let mut automated = effect.clone();
        for (target, lane) in &self.lanes {
            let MixerParameter::Insert {
                slot: target_slot,
                parameter,
            } = *target
            else {
                continue;
            };
            if usize::from(target_slot) == slot {
                for event in lane.events.iter().filter(|event| event.time_seconds <= 0.0) {
                    set_effect_parameter(&mut automated, parameter, event.value);
                }
            }
        }
        let mut maximum = if insert_is_bypassed(&automated) {
            0
        } else {
            insert_tail_frames(&automated, sample_rate, tempo_changes)
        };
        for (target, lane) in &self.lanes {
            let MixerParameter::Insert {
                slot: target_slot,
                parameter,
            } = *target
            else {
                continue;
            };
            if usize::from(target_slot) != slot {
                continue;
            }
            for event in lane.events.iter().filter(|event| event.time_seconds > 0.0) {
                set_effect_parameter(&mut automated, parameter, event.value);
                if !insert_is_bypassed(&automated) {
                    maximum =
                        maximum.max(insert_tail_frames(&automated, sample_rate, tempo_changes));
                }
            }
        }
        maximum
    }

    fn automate_insert(
        &mut self,
        slot: usize,
        time: f64,
        processor: &mut crate::effects::InsertProcessor,
        sample_rate: u32,
    ) {
        for (target, lane) in &mut self.lanes {
            let MixerParameter::Insert {
                slot: target_slot,
                parameter,
            } = *target
            else {
                continue;
            };
            if usize::from(target_slot) == slot {
                let value = lane.value_at(time);
                processor.set_parameter(parameter, value, sample_rate);
            }
        }
    }

    fn send_gains(&mut self, time: f64, mut gains: [f32; SEND_COUNT]) -> [f32; SEND_COUNT] {
        for (parameter, lane) in &mut self.lanes {
            if let MixerParameter::SendGain { bus } = *parameter {
                gains[usize::from(bus)] = lane.value_at(time);
            }
        }
        gains
    }

    fn has_send(&self) -> bool {
        self.lanes
            .keys()
            .any(|parameter| matches!(parameter, MixerParameter::SendGain { .. }))
    }
}

pub fn mix_stereo(tracks: &[MonoTrack<'_>], master_gain: f32) -> Result<StereoSound, Error> {
    mix_stereo_with_tracks(tracks, &[], master_gain)
}

pub fn mix_stereo_analyzed(
    tracks: &[MonoTrack<'_>],
    master_gain: f32,
) -> Result<AnalyzedMix, Error> {
    mix_stereo_with_sends_and_tempo_analyzed(
        tracks,
        &[],
        &[SendBus::default(), SendBus::default()],
        master_gain,
        &[],
    )
}

pub fn mix_stereo_with_tracks(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    master_gain: f32,
) -> Result<StereoSound, Error> {
    mix_stereo_with_sends(
        tracks,
        stereo_tracks,
        &[SendBus::default(), SendBus::default()],
        master_gain,
    )
}

pub fn mix_stereo_with_sends(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    send_buses: &[SendBus; SEND_COUNT],
    master_gain: f32,
) -> Result<StereoSound, Error> {
    mix_stereo_with_sends_and_tempo(tracks, stereo_tracks, send_buses, master_gain, &[])
}

pub fn mix_stereo_with_sends_and_tempo(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    send_buses: &[SendBus; SEND_COUNT],
    master_gain: f32,
    tempo_changes: &[SynthTempoChange],
) -> Result<StereoSound, Error> {
    Ok(mix_stereo_with_sends_and_tempo_analyzed(
        tracks,
        stereo_tracks,
        send_buses,
        master_gain,
        tempo_changes,
    )?
    .sound)
}

pub fn mix_stereo_with_sends_and_tempo_analyzed(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    send_buses: &[SendBus; SEND_COUNT],
    master_gain: f32,
    tempo_changes: &[SynthTempoChange],
) -> Result<AnalyzedMix, Error> {
    mix_stereo_with_sends_and_tempo_analyzed_range(
        tracks,
        stereo_tracks,
        send_buses,
        master_gain,
        tempo_changes,
        None,
    )
}

/// Analyze only `analysis_range` while rendering the complete sound and its tails.
/// The range is clamped to the rendered frame count and uses `(start, end)` indexing.
pub fn mix_stereo_with_sends_and_tempo_analyzed_range(
    tracks: &[MonoTrack<'_>],
    stereo_tracks: &[StereoTrack<'_>],
    send_buses: &[SendBus; SEND_COUNT],
    master_gain: f32,
    tempo_changes: &[SynthTempoChange],
    analysis_range: Option<(usize, usize)>,
) -> Result<AnalyzedMix, Error> {
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
    let mut buffers = MixBuffers {
        left: vec![0.0; frame_count],
        right: vec![0.0; frame_count],
        send_left: std::array::from_fn(|_| Vec::new()),
        send_right: std::array::from_fn(|_| Vec::new()),
    };
    let mut unsupported_mixer_automation_events = 0;
    let mut channel_meters = Vec::new();
    let (analysis_start, requested_analysis_end) = analysis_range.unwrap_or((0, usize::MAX));

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
        let (mut automation, unsupported) = ChannelAutomation::new(
            track.automation,
            track.inserts,
            track.gain,
            track.pan,
            track.send_gains,
        )?;
        unsupported_mixer_automation_events += unsupported;
        let angle = (track.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let mut left_gain = angle.cos() * track.gain * master_gain;
        let mut right_gain = angle.sin() * track.gain * master_gain;
        let has_gain_or_pan = automation.has_gain_or_pan();
        let mut track_left = vec![0.0; track.sound.samples.len()];
        let mut track_right = vec![0.0; track.sound.samples.len()];
        for (frame, sample) in track.sound.samples.iter().enumerate() {
            if has_gain_or_pan {
                let time = frame as f64 / f64::from(sample_rate);
                let (gain, pan) = automation.gain_and_pan(time, track.gain, track.pan);
                let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
                left_gain = angle.cos() * gain * master_gain;
                right_gain = angle.sin() * gain * master_gain;
            }
            track_left[frame] = sample * left_gain;
            track_right[frame] = sample * right_gain;
        }
        if let Some(waveshaper) = track.waveshaper {
            waveshaper.validate()?;
            if has_waveshaper_automation(track.controllers) {
                apply_stereo_waveshaper_automation(
                    &mut track_left,
                    &mut track_right,
                    sample_rate,
                    waveshaper,
                    track.controllers,
                );
            } else {
                apply_stereo_waveshaper(&mut track_left, &mut track_right, waveshaper);
            }
        }
        if let Some(delay) = track.delay {
            delay.validate()?;
            if has_delay_automation(track.controllers) {
                apply_stereo_delay_automation_with_tempo(
                    &mut track_left,
                    &mut track_right,
                    sample_rate,
                    delay,
                    track.controllers,
                    tempo_changes,
                );
            } else {
                apply_stereo_delay_with_tempo(
                    &mut track_left,
                    &mut track_right,
                    sample_rate,
                    delay,
                    tempo_changes,
                );
            }
        }
        channel_meters.push(mix_channel(
            &mut buffers,
            ProcessedChannel {
                left: track_left,
                right: track_right,
                inserts: track.inserts,
                send_gains: track.send_gains,
                automation: &mut automation,
            },
            sample_rate,
            tempo_changes,
            (analysis_start, requested_analysis_end),
        )?);
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
        let (mut automation, unsupported) = ChannelAutomation::new(
            track.automation,
            track.inserts,
            track.gain,
            track.pan,
            track.send_gains,
        )?;
        unsupported_mixer_automation_events += unsupported;
        let mut left_balance = if track.pan < 0.0 {
            1.0
        } else {
            1.0 - track.pan
        };
        let mut right_balance = if track.pan > 0.0 {
            1.0
        } else {
            1.0 + track.pan
        };
        let mut gain = track.gain;
        let has_gain_or_pan = automation.has_gain_or_pan();
        let mut track_left = track.sound.left.clone();
        let mut track_right = track.sound.right.clone();
        for frame in 0..track_left.len() {
            if has_gain_or_pan {
                let time = frame as f64 / f64::from(sample_rate);
                let pan;
                (gain, pan) = automation.gain_and_pan(time, track.gain, track.pan);
                left_balance = if pan < 0.0 { 1.0 } else { 1.0 - pan };
                right_balance = if pan > 0.0 { 1.0 } else { 1.0 + pan };
            }
            track_left[frame] *= gain * master_gain * left_balance;
            track_right[frame] *= gain * master_gain * right_balance;
        }
        channel_meters.push(mix_channel(
            &mut buffers,
            ProcessedChannel {
                left: track_left,
                right: track_right,
                inserts: track.inserts,
                send_gains: track.send_gains,
                automation: &mut automation,
            },
            sample_rate,
            tempo_changes,
            (analysis_start, requested_analysis_end),
        )?);
    }
    for (index, bus) in send_buses.iter().enumerate() {
        bus.validate()?;
        if buffers.send_left[index].is_empty() || bus.gain == 0.0 {
            continue;
        }
        let mut bus_left = std::mem::take(&mut buffers.send_left[index]);
        let mut bus_right = std::mem::take(&mut buffers.send_right[index]);
        if let Some(effect) = &bus.effect {
            apply_insert_with_tempo(
                &mut bus_left,
                &mut bus_right,
                sample_rate,
                effect,
                tempo_changes,
            )?;
        }
        if buffers.left.len() < bus_left.len() {
            buffers.left.resize(bus_left.len(), 0.0);
            buffers.right.resize(bus_left.len(), 0.0);
        }
        for frame in 0..bus_left.len() {
            buffers.left[frame] += bus_left[frame] * bus.gain;
            buffers.right[frame] += bus_right[frame] * bus.gain;
        }
    }
    let (analysis_start, analysis_end) = analysis_range.unwrap_or((0, buffers.left.len()));
    let analysis_end = analysis_end.min(buffers.left.len());
    let analysis_start = analysis_start.min(analysis_end);
    let meter_block_count = (analysis_end - analysis_start).div_ceil(METER_BLOCK_FRAMES);
    for meter in &mut channel_meters {
        meter.meter_blocks.resize(meter_block_count, [0.0; 2]);
    }
    let mut analysis = MixAnalysis::default();
    for (frame, (left, right)) in buffers.left.iter_mut().zip(&mut buffers.right).enumerate() {
        if (analysis_start..analysis_end).contains(&frame) {
            let frame_peak = [left.abs(), right.abs()];
            analysis.pre_saturator_peak[0] = analysis.pre_saturator_peak[0].max(frame_peak[0]);
            analysis.pre_saturator_peak[1] = analysis.pre_saturator_peak[1].max(frame_peak[1]);
            analysis.clipped_frames += usize::from(frame_peak[0] > 1.0 || frame_peak[1] > 1.0);
        }
        *left = left.tanh();
        *right = right.tanh();
    }
    if tracks.iter().any(|track| track.dc_block) {
        block_dc(&mut buffers.left, sample_rate);
        block_dc(&mut buffers.right, sample_rate);
    }
    Ok(AnalyzedMix {
        sound: StereoSound {
            left: buffers.left,
            right: buffers.right,
            sample_rate,
        },
        analysis,
        channel_meters,
        unsupported_mixer_automation_events,
    })
}

struct MixBuffers {
    left: Vec<f32>,
    right: Vec<f32>,
    send_left: [Vec<f32>; SEND_COUNT],
    send_right: [Vec<f32>; SEND_COUNT],
}

struct ProcessedChannel<'a> {
    left: Vec<f32>,
    right: Vec<f32>,
    inserts: &'a [InsertEffect],
    send_gains: [f32; SEND_COUNT],
    automation: &'a mut ChannelAutomation,
}

fn mix_channel(
    buffers: &mut MixBuffers,
    channel: ProcessedChannel<'_>,
    sample_rate: u32,
    tempo_changes: &[SynthTempoChange],
    analysis_range: (usize, usize),
) -> Result<ChannelMeter, Error> {
    let ProcessedChannel {
        left: mut track_left,
        right: mut track_right,
        inserts,
        send_gains,
        automation,
    } = channel;
    let (analysis_start, analysis_end) = analysis_range;
    if inserts.len() > INSERT_LIMIT {
        return Err(Error::InvalidMix(
            "a channel supports at most four inserts".to_owned(),
        ));
    }
    if send_gains
        .iter()
        .any(|gain| !gain.is_finite() || !(0.0..=1.0).contains(gain))
    {
        return Err(Error::InvalidMix("send gains must be 0..=1".to_owned()));
    }
    for (slot, effect) in inserts.iter().enumerate() {
        if automation.has_insert(slot) {
            let tail_frames =
                automation.insert_tail_frames(slot, effect, sample_rate, tempo_changes);
            apply_insert_automated(
                &mut track_left,
                &mut track_right,
                sample_rate,
                effect,
                tempo_changes,
                tail_frames,
                |processor, time| {
                    automation.automate_insert(slot, time, processor, sample_rate);
                },
            )?;
        } else {
            apply_insert_with_tempo(
                &mut track_left,
                &mut track_right,
                sample_rate,
                effect,
                tempo_changes,
            )?;
        }
    }
    if buffers.left.len() < track_left.len() {
        buffers.left.resize(track_left.len(), 0.0);
        buffers.right.resize(track_left.len(), 0.0);
    }
    for frame in 0..track_left.len() {
        buffers.left[frame] += track_left[frame];
        buffers.right[frame] += track_right[frame];
    }
    if automation.has_send() {
        for index in 0..SEND_COUNT {
            if buffers.send_left[index].len() < track_left.len() {
                buffers.send_left[index].resize(track_left.len(), 0.0);
                buffers.send_right[index].resize(track_left.len(), 0.0);
            }
        }
        for frame in 0..track_left.len() {
            let time = frame as f64 / f64::from(sample_rate);
            let gains = automation.send_gains(time, send_gains);
            for (index, gain) in gains.into_iter().enumerate() {
                buffers.send_left[index][frame] += track_left[frame] * gain;
                buffers.send_right[index][frame] += track_right[frame] * gain;
            }
        }
    } else {
        for (index, gain) in send_gains.iter().enumerate() {
            let gain = *gain;
            if gain == 0.0 {
                continue;
            }
            if buffers.send_left[index].len() < track_left.len() {
                buffers.send_left[index].resize(track_left.len(), 0.0);
                buffers.send_right[index].resize(track_left.len(), 0.0);
            }
            for frame in 0..track_left.len() {
                buffers.send_left[index][frame] += track_left[frame] * gain;
                buffers.send_right[index][frame] += track_right[frame] * gain;
            }
        }
    }
    let mut meter = ChannelMeter::default();
    let analysis_end = analysis_end.min(track_left.len());
    let analysis_start = analysis_start.min(analysis_end);
    for frame in analysis_start..analysis_end {
        let peak = [track_left[frame].abs(), track_right[frame].abs()];
        meter.pre_sum_peak[0] = meter.pre_sum_peak[0].max(peak[0]);
        meter.pre_sum_peak[1] = meter.pre_sum_peak[1].max(peak[1]);
        meter.clipped_frames += usize::from(peak[0] > 1.0 || peak[1] > 1.0);
        let block = (frame - analysis_start) / METER_BLOCK_FRAMES;
        if meter.meter_blocks.len() <= block {
            meter.meter_blocks.push([0.0; 2]);
        }
        meter.meter_blocks[block][0] = meter.meter_blocks[block][0].max(peak[0]);
        meter.meter_blocks[block][1] = meter.meter_blocks[block][1].max(peak[1]);
    }
    Ok(meter)
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
                    waveshaper: None,
                    delay: None,
                    controllers: &[],
                    automation: &[],
                    inserts: &[],
                    send_gains: [0.0; SEND_COUNT],
                    dc_block: false,
                },
                MonoTrack {
                    sound: &long,
                    gain: 1.0,
                    pan: 1.0,
                    waveshaper: None,
                    delay: None,
                    controllers: &[],
                    automation: &[],
                    inserts: &[],
                    send_gains: [0.0; SEND_COUNT],
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
            beats_per_cycle: None,
        };
        let stereo = mix_stereo(
            &[MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: -1.0,
                waveshaper: None,
                delay: Some(&delay),
                controllers: &[],
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
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
    fn stereo_insert_automation_matches_static_extremes() {
        let sound = RenderedSound {
            samples: (0..32)
                .map(|frame| (frame as f32 * 0.3).sin() * 0.5)
                .collect(),
            sample_rate: 8_000,
        };
        let waveshaper = Waveshaper {
            drive: 32.0,
            contour: 1.0,
            level: 1.0,
        };
        let delay = Delay {
            time_seconds: 2.0,
            feedback: 0.95,
            mix: 1.0,
            mode: crate::DelayMode::StereoCross,
            beats_per_cycle: None,
        };
        let controllers = (20..=25)
            .map(|controller| SynthController {
                time_seconds: 0.0,
                controller,
                value: 127,
            })
            .collect::<Vec<_>>();
        let static_mix = mix_stereo(
            &[MonoTrack {
                sound: &sound,
                gain: 0.8,
                pan: -0.3,
                waveshaper: Some(&waveshaper),
                delay: Some(&delay),
                controllers: &[],
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            }],
            1.0,
        )
        .unwrap();
        let dynamic_mix = mix_stereo(
            &[MonoTrack {
                sound: &sound,
                gain: 0.8,
                pan: -0.3,
                waveshaper: Some(&waveshaper),
                delay: Some(&delay),
                controllers: &controllers,
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            }],
            1.0,
        )
        .unwrap();
        assert_eq!(dynamic_mix.left, static_mix.left);
        assert_eq!(dynamic_mix.right, static_mix.right);
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
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
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
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
            }],
            1.0,
        )
        .unwrap();
        assert!(left.right.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn four_inserts_and_two_sends_share_post_channel_audio_and_bus_tails() {
        let sound = RenderedSound {
            samples: vec![0.3; 4],
            sample_rate: 1_000,
        };
        let inserts = vec![
            InsertEffect::Bitcrush(crate::Bitcrush {
                mix: 0.0,
                ..crate::Bitcrush::default()
            }),
            InsertEffect::Bitcrush(crate::Bitcrush {
                mix: 0.0,
                ..crate::Bitcrush::default()
            }),
            InsertEffect::Bitcrush(crate::Bitcrush {
                mix: 0.0,
                ..crate::Bitcrush::default()
            }),
            InsertEffect::Bitcrush(crate::Bitcrush {
                bits: 2,
                downsample: 1,
                mix: 1.0,
            }),
        ];
        let buses = [
            SendBus {
                effect: None,
                gain: 0.5,
            },
            SendBus {
                effect: Some(InsertEffect::Delay(Delay {
                    time_seconds: 0.002,
                    feedback: 0.0,
                    mix: 1.0,
                    mode: crate::DelayMode::Mono,
                    beats_per_cycle: None,
                })),
                gain: 1.0,
            },
        ];
        let mixed = mix_stereo_with_sends(
            &[MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: -1.0,
                waveshaper: None,
                delay: None,
                controllers: &[],
                automation: &[],
                inserts: &inserts,
                send_gains: [0.5, 0.25],
                dc_block: false,
            }],
            &[],
            &buses,
            1.0,
        )
        .unwrap();
        assert_eq!(mixed.left.len(), 20);
        assert!((mixed.left[0] - 0.75_f32.tanh()).abs() < 1e-6);
        assert!((mixed.left[2] - 0.875_f32.tanh()).abs() < 1e-6);
        assert!(mixed.right.iter().all(|sample| *sample == 0.0));

        let too_many = (0..5).map(|_| inserts[0].clone()).collect::<Vec<_>>();
        assert!(
            mix_stereo_with_sends(
                &[MonoTrack {
                    sound: &sound,
                    gain: 1.0,
                    pan: 0.0,
                    waveshaper: None,
                    delay: None,
                    controllers: &[],
                    automation: &[],
                    inserts: &too_many,
                    send_gains: [0.0; SEND_COUNT],
                    dc_block: false,
                }],
                &[],
                &[SendBus::default(), SendBus::default()],
                1.0,
            )
            .is_err()
        );
    }

    #[test]
    fn analysis_detects_pre_saturator_clipping_and_supports_section_ranges() {
        let sound = RenderedSound {
            samples: vec![1.0, 0.5, 0.0],
            sample_rate: 48_000,
        };
        let tracks = [
            MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: 0.0,
                waveshaper: None,
                delay: None,
                controllers: &[],
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            },
            MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: 0.0,
                waveshaper: None,
                delay: None,
                controllers: &[],
                automation: &[],
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            },
        ];
        let mixed = mix_stereo_analyzed(&tracks, 1.0).unwrap();
        assert_eq!(mixed.analysis.pre_saturator_peak, [2.0_f32.sqrt(); 2]);
        assert_eq!(mixed.analysis.clipped_frames, 1);
        assert!(mixed.sound.left.iter().all(|sample| *sample < 1.0));

        let section = mix_stereo_with_sends_and_tempo_analyzed_range(
            &tracks,
            &[],
            &[SendBus::default(), SendBus::default()],
            1.0,
            &[],
            Some((1, 3)),
        )
        .unwrap();
        assert_eq!(section.sound.left, mixed.sound.left);
        assert_eq!(
            section.analysis.pre_saturator_peak,
            [2.0_f32.sqrt() / 2.0; 2]
        );
        assert_eq!(section.analysis.clipped_frames, 0);
        assert!(!section.analysis.is_clipped());
    }

    #[test]
    fn every_typed_insert_parameter_matches_static_audio_at_time_zero() {
        use evil_performance::EffectParameter;

        let sound = RenderedSound {
            samples: (0..32)
                .map(|frame| (frame as f32 * 0.17).sin() * 0.4)
                .collect(),
            sample_rate: 1_000,
        };
        for parameter in EffectParameter::ALL {
            let base = [
                InsertEffect::Waveshaper(Waveshaper::default()),
                InsertEffect::Delay(Delay::default()),
                InsertEffect::Bitcrush(crate::Bitcrush::default()),
                InsertEffect::Equalizer(crate::Equalizer::default()),
                InsertEffect::Compressor(crate::Compressor::default()),
                InsertEffect::Chorus(crate::Chorus::default()),
                InsertEffect::Flanger(crate::Flanger::default()),
                InsertEffect::Reverb(crate::Reverb::default()),
            ]
            .into_iter()
            .find(|effect| effect.automatable_parameters().contains(&parameter))
            .unwrap();
            let mut static_effect = base.clone();
            let current = effect_parameter(&static_effect, parameter).unwrap();
            let value = if current == *parameter.range().end() {
                *parameter.range().start()
            } else {
                *parameter.range().end()
            };
            assert!(set_effect_parameter(&mut static_effect, parameter, value));
            static_effect.validate().unwrap();
            let automation = [SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Insert { slot: 0, parameter },
                value,
            }];
            let static_mix = mix_stereo(
                &[MonoTrack {
                    sound: &sound,
                    gain: 0.8,
                    pan: 0.2,
                    waveshaper: None,
                    delay: None,
                    controllers: &[],
                    automation: &[],
                    inserts: std::slice::from_ref(&static_effect),
                    send_gains: [0.0; SEND_COUNT],
                    dc_block: false,
                }],
                0.9,
            )
            .unwrap();
            let automated = mix_stereo_with_sends_and_tempo_analyzed(
                &[MonoTrack {
                    sound: &sound,
                    gain: 0.8,
                    pan: 0.2,
                    waveshaper: None,
                    delay: None,
                    controllers: &[],
                    automation: &automation,
                    inserts: std::slice::from_ref(&base),
                    send_gains: [0.0; SEND_COUNT],
                    dc_block: false,
                }],
                &[],
                &[SendBus::default(), SendBus::default()],
                0.9,
                &[],
            )
            .unwrap();
            assert_eq!(automated.unsupported_mixer_automation_events, 0);
            assert_eq!(automated.sound.left, static_mix.left, "{parameter:?} left");
            assert_eq!(
                automated.sound.right, static_mix.right,
                "{parameter:?} right"
            );
        }
    }

    #[test]
    fn gain_pan_and_send_automation_interpolate_and_hold_the_last_value() {
        let sound = RenderedSound {
            samples: vec![1.0; 5],
            sample_rate: 4,
        };
        let automation = [
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Gain,
                value: 0.0,
            },
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::SendGain { bus: 0 },
                value: 0.0,
            },
            SynthMixerAutomation {
                time_seconds: 1.0,
                parameter: MixerParameter::Gain,
                value: 1.0,
            },
            SynthMixerAutomation {
                time_seconds: 1.0,
                parameter: MixerParameter::SendGain { bus: 0 },
                value: 1.0,
            },
        ];
        let mixed = mix_stereo_with_sends(
            &[MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: -1.0,
                waveshaper: None,
                delay: None,
                controllers: &[],
                automation: &automation,
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            }],
            &[],
            &[
                SendBus {
                    effect: None,
                    gain: 1.0,
                },
                SendBus::default(),
            ],
            1.0,
        )
        .unwrap();
        let expected = [0.0, 0.3125, 0.75, 1.3125, 2.0].map(f32::tanh);
        for (actual, expected) in mixed.left.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6, "{:?}", mixed.left);
        }
        assert!(mixed.right.iter().all(|sample| sample.abs() < 1e-6));
    }

    #[test]
    fn stereo_tracks_interpolate_gain_and_balance_automation() {
        let sound = StereoSound {
            left: vec![1.0; 5],
            right: vec![0.5; 5],
            sample_rate: 4,
        };
        let automation = [
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Gain,
                value: 0.0,
            },
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Pan,
                value: -1.0,
            },
            SynthMixerAutomation {
                time_seconds: 1.0,
                parameter: MixerParameter::Gain,
                value: 1.0,
            },
            SynthMixerAutomation {
                time_seconds: 1.0,
                parameter: MixerParameter::Pan,
                value: 1.0,
            },
        ];
        let mixed = mix_stereo_with_tracks(
            &[],
            &[StereoTrack {
                sound: &sound,
                gain: 1.0,
                pan: 0.0,
                automation: &automation,
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
            }],
            1.0,
        )
        .unwrap();
        let expected_left = [0.0, 0.25, 0.5, 0.375, 0.0].map(f32::tanh);
        let expected_right = [0.0, 0.0625, 0.25, 0.375, 0.5].map(f32::tanh);
        for frame in 0..5 {
            assert!((mixed.left[frame] - expected_left[frame]).abs() < 1e-6);
            assert!((mixed.right[frame] - expected_right[frame]).abs() < 1e-6);
        }
    }

    #[test]
    fn unsupported_insert_targets_are_reported_without_changing_other_lanes() {
        let sound = RenderedSound {
            samples: vec![0.5; 4],
            sample_rate: 4,
        };
        let automation = [
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Insert {
                    slot: 0,
                    parameter: evil_performance::EffectParameter::ReverbMix,
                },
                value: 1.0,
            },
            SynthMixerAutomation {
                time_seconds: 0.0,
                parameter: MixerParameter::Gain,
                value: 0.5,
            },
        ];
        let mixed = mix_stereo_analyzed(
            &[MonoTrack {
                sound: &sound,
                gain: 1.0,
                pan: 0.0,
                waveshaper: None,
                delay: None,
                controllers: &[],
                automation: &automation,
                inserts: &[],
                send_gains: [0.0; SEND_COUNT],
                dc_block: false,
            }],
            1.0,
        )
        .unwrap();
        assert_eq!(mixed.unsupported_mixer_automation_events, 1);
        let angle = std::f32::consts::FRAC_PI_4;
        let expected = (0.25 * angle.cos()).tanh();
        assert!((mixed.sound.left[0] - expected).abs() < 1e-6);
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
                        waveshaper: None,
                        delay: None,
                        controllers: &[],
                        automation: &[],
                        inserts: &[],
                        send_gains: [0.0; SEND_COUNT],
                        dc_block: false,
                    },
                    MonoTrack {
                        sound: &second,
                        gain: 1.0,
                        pan: 0.0,
                        waveshaper: None,
                        delay: None,
                        controllers: &[],
                        automation: &[],
                        inserts: &[],
                        send_gains: [0.0; SEND_COUNT],
                        dc_block: false,
                    },
                ],
                1.0,
            )
            .is_err()
        );
    }
}
