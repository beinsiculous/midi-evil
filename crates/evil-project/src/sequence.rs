use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Aftertouch, ControllerEvent, Error, MixerAutomationEvent, MixerParameter, Note, Pattern,
    PitchBend, Track, invalid,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridModifier {
    #[default]
    Straight,
    Dotted,
    Triplet,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuantizeSettings {
    pub denominator: u32,
    pub modifier: GridModifier,
    pub swing: f32,
    pub strength: f32,
    pub quantize_durations: bool,
}

impl Default for QuantizeSettings {
    fn default() -> Self {
        Self {
            denominator: 16,
            modifier: GridModifier::Straight,
            swing: 0.0,
            strength: 1.0,
            quantize_durations: true,
        }
    }
}

impl QuantizeSettings {
    pub fn validate(self) -> Result<(), Error> {
        if !(1..=64).contains(&self.denominator) || !self.denominator.is_power_of_two() {
            return Err(invalid(
                "quantize denominator must be a power of two between 1 and 64",
            ));
        }
        if !self.swing.is_finite() || !(0.0..=1.0).contains(&self.swing) {
            return Err(invalid("quantize swing must be between 0 and 1"));
        }
        if !self.strength.is_finite() || !(0.0..=1.0).contains(&self.strength) {
            return Err(invalid("quantize strength must be between 0 and 1"));
        }
        Ok(())
    }

    pub fn step_ticks_f64(self, ticks_per_quarter: u16) -> Result<f64, Error> {
        if ticks_per_quarter == 0 {
            return Err(invalid("ticks_per_quarter must be greater than zero"));
        }
        self.validate()?;
        let base = 4.0 * f64::from(ticks_per_quarter) / f64::from(self.denominator);
        let factor = match self.modifier {
            GridModifier::Straight => 1.0,
            GridModifier::Dotted => 1.5,
            GridModifier::Triplet => 2.0 / 3.0,
        };
        Ok(base * factor)
    }
}

/// The event storage shared by unique track performances and reusable patterns.
/// Editors use this interface so both contexts have identical editing behavior.
pub trait EventSequence {
    fn notes(&self) -> &[Note];
    fn notes_mut(&mut self) -> &mut Vec<Note>;
    fn pitch_bends(&self) -> &[PitchBend];
    fn pitch_bends_mut(&mut self) -> &mut Vec<PitchBend>;
    fn aftertouch(&self) -> &[Aftertouch];
    fn aftertouch_mut(&mut self) -> &mut Vec<Aftertouch>;
    fn controllers(&self) -> &[ControllerEvent];
    fn controllers_mut(&mut self) -> &mut Vec<ControllerEvent>;
    fn mixer_automation(&self) -> &[MixerAutomationEvent];
    fn mixer_automation_mut(&mut self) -> &mut Vec<MixerAutomationEvent>;
}

macro_rules! impl_event_sequence {
    ($kind:ty) => {
        impl EventSequence for $kind {
            fn notes(&self) -> &[Note] {
                &self.notes
            }
            fn notes_mut(&mut self) -> &mut Vec<Note> {
                &mut self.notes
            }
            fn pitch_bends(&self) -> &[PitchBend] {
                &self.pitch_bends
            }
            fn pitch_bends_mut(&mut self) -> &mut Vec<PitchBend> {
                &mut self.pitch_bends
            }
            fn aftertouch(&self) -> &[Aftertouch] {
                &self.aftertouch
            }
            fn aftertouch_mut(&mut self) -> &mut Vec<Aftertouch> {
                &mut self.aftertouch
            }
            fn controllers(&self) -> &[ControllerEvent] {
                &self.controllers
            }
            fn controllers_mut(&mut self) -> &mut Vec<ControllerEvent> {
                &mut self.controllers
            }
            fn mixer_automation(&self) -> &[MixerAutomationEvent] {
                &self.mixer_automation
            }
            fn mixer_automation_mut(&mut self) -> &mut Vec<MixerAutomationEvent> {
                &mut self.mixer_automation
            }
        }
    };
}

impl_event_sequence!(Track);
impl_event_sequence!(Pattern);

pub fn quantize_notes(
    sequence: &mut (impl EventSequence + ?Sized),
    grid_ticks: u64,
    selected: Option<usize>,
    length_ticks: Option<u64>,
) -> Result<(), Error> {
    if grid_ticks == 0 {
        return Err(invalid("quantize grid must be greater than zero"));
    }
    let updates: Result<Vec<_>, Error> = sequence
        .notes()
        .iter()
        .enumerate()
        .filter(|(index, _)| selected.is_none_or(|selected| selected == *index))
        .map(|(index, note)| {
            let start_tick =
                note.start_tick.saturating_add(grid_ticks / 2) / grid_ticks * grid_ticks;
            let duration_ticks = (note.duration_ticks.saturating_add(grid_ticks / 2) / grid_ticks
                * grid_ticks)
                .max(grid_ticks);
            start_tick
                .checked_add(duration_ticks)
                .ok_or_else(|| invalid("quantized note end exceeds the supported range"))?;
            Ok((index, start_tick, duration_ticks))
        })
        .collect();
    for (index, start_tick, duration_ticks) in updates? {
        let note = &mut sequence.notes_mut()[index];
        note.start_tick = start_tick;
        note.duration_ticks = duration_ticks;
    }
    if let Some(length) = length_ticks {
        sequence.notes_mut().retain(|note| note.start_tick < length);
    }
    Ok(())
}

pub fn transpose_notes(
    sequence: &mut (impl EventSequence + ?Sized),
    semitones: i16,
    selected: Option<usize>,
) -> Result<(), Error> {
    let notes = sequence.notes_mut();
    if notes.iter().enumerate().any(|(index, note)| {
        (selected.is_none() || selected == Some(index))
            && !(0..=127).contains(&(i16::from(note.pitch) + semitones))
    }) {
        return Err(invalid(
            "transpose would move a note outside the MIDI range",
        ));
    }
    for (index, note) in notes.iter_mut().enumerate() {
        if selected.is_none() || selected == Some(index) {
            note.pitch = (i16::from(note.pitch) + semitones) as u8;
        }
    }
    Ok(())
}

/// Shorten same-pitch overlaps without changing notes on other pitches.
pub fn clean_overlaps(sequence: &mut (impl EventSequence + ?Sized)) {
    let mut notes: Vec<_> = sequence
        .notes_mut()
        .drain(..)
        .enumerate()
        .collect::<Vec<_>>();
    notes.sort_by_key(|(_, note)| (note.pitch, note.start_tick));
    for index in 0..notes.len().saturating_sub(1) {
        let next_start = notes[index + 1].1.start_tick;
        let note = &mut notes[index].1;
        if note.start_tick < next_start {
            note.duration_ticks = note.duration_ticks.min(next_start - note.start_tick);
        }
    }
    notes.sort_by_key(|(original_index, note)| (note.start_tick, *original_index));
    *sequence.notes_mut() = notes.into_iter().map(|(_, note)| note).collect();
}

/// Quantize notes with straight, dotted, or triplet grids, swing, partial strength, and optional duration snap.
pub fn quantize_notes_advanced(
    sequence: &mut (impl EventSequence + ?Sized),
    ticks_per_quarter: u16,
    settings: QuantizeSettings,
    selected: Option<&[usize]>,
    length_ticks: Option<u64>,
) -> Result<(), Error> {
    let step_ticks = settings.step_ticks_f64(ticks_per_quarter)?;
    let updates: Result<Vec<_>, Error> = sequence
        .notes()
        .iter()
        .enumerate()
        .filter(|(index, _)| is_selected(*index, selected))
        .map(|(index, note)| {
            let approx_step = (note.start_tick as f64 / step_ticks).floor().max(0.0) as u64;
            let mut best_target = swung_grid_tick(approx_step, step_ticks, settings.swing);
            let mut best_distance = best_target.abs_diff(note.start_tick);
            for step in approx_step.saturating_sub(1)..=approx_step.saturating_add(1) {
                let candidate = swung_grid_tick(step, step_ticks, settings.swing);
                let distance = candidate.abs_diff(note.start_tick);
                if distance < best_distance {
                    best_distance = distance;
                    best_target = candidate;
                }
            }
            let start_tick = (note.start_tick as f64
                + (best_target as f64 - note.start_tick as f64) * f64::from(settings.strength))
            .round()
            .max(0.0) as u64;
            let duration_ticks = if settings.quantize_durations {
                let target_steps = (note.duration_ticks as f64 / step_ticks).round().max(1.0);
                let target_duration = (target_steps * step_ticks).round().max(1.0);
                (note.duration_ticks as f64
                    + (target_duration - note.duration_ticks as f64) * f64::from(settings.strength))
                .round()
                .max(1.0) as u64
            } else {
                note.duration_ticks
            };
            start_tick
                .checked_add(duration_ticks)
                .ok_or_else(|| invalid("quantized note end exceeds the supported range"))?;
            Ok((index, start_tick, duration_ticks))
        })
        .collect();
    for (index, start_tick, duration_ticks) in updates? {
        let note = &mut sequence.notes_mut()[index];
        note.start_tick = start_tick;
        note.duration_ticks = duration_ticks;
    }
    if let Some(length) = length_ticks {
        sequence.notes_mut().retain(|note| note.start_tick < length);
    }
    Ok(())
}

pub fn select_all_notes(sequence: &(impl EventSequence + ?Sized)) -> Vec<usize> {
    (0..sequence.notes().len()).collect()
}

pub fn select_same_pitch_notes(sequence: &(impl EventSequence + ?Sized), pitch: u8) -> Vec<usize> {
    sequence
        .notes()
        .iter()
        .enumerate()
        .filter_map(|(index, note)| (note.pitch == pitch).then_some(index))
        .collect()
}

pub fn invert_note_selection(
    sequence: &(impl EventSequence + ?Sized),
    selected: &[usize],
) -> Vec<usize> {
    let selected_set: BTreeSet<usize> = selected.iter().copied().collect();
    (0..sequence.notes().len())
        .filter(|index| !selected_set.contains(index))
        .collect()
}

pub fn transpose_selected_notes(
    sequence: &mut (impl EventSequence + ?Sized),
    semitones: i16,
    selected: Option<&[usize]>,
) -> Result<(), Error> {
    let notes = sequence.notes_mut();
    if notes.iter().enumerate().any(|(index, note)| {
        is_selected(index, selected) && !(0..=127).contains(&(i16::from(note.pitch) + semitones))
    }) {
        return Err(invalid(
            "transpose would move a note outside the MIDI range",
        ));
    }
    for (index, note) in notes.iter_mut().enumerate() {
        if is_selected(index, selected) {
            note.pitch = (i16::from(note.pitch) + semitones) as u8;
        }
    }
    Ok(())
}

pub fn set_notes_velocity(
    sequence: &mut (impl EventSequence + ?Sized),
    selected: Option<&[usize]>,
    velocity: u8,
) -> Result<usize, Error> {
    if !(1..=127).contains(&velocity) {
        return Err(invalid("note velocity must be between 1 and 127"));
    }
    let mut updated = 0;
    for (index, note) in sequence.notes_mut().iter_mut().enumerate() {
        if is_selected(index, selected) {
            note.velocity = velocity;
            updated += 1;
        }
    }
    Ok(updated)
}

pub fn scale_notes_velocity(
    sequence: &mut (impl EventSequence + ?Sized),
    selected: Option<&[usize]>,
    factor: f32,
) -> Result<usize, Error> {
    if !factor.is_finite() || factor < 0.0 {
        return Err(invalid("velocity scale factor must be finite and >= 0"));
    }
    let mut updated = 0;
    for (index, note) in sequence.notes_mut().iter_mut().enumerate() {
        if is_selected(index, selected) {
            note.velocity = (f32::from(note.velocity) * factor)
                .round()
                .clamp(1.0, 127.0) as u8;
            updated += 1;
        }
    }
    Ok(updated)
}

pub fn ramp_notes_velocity(
    sequence: &mut (impl EventSequence + ?Sized),
    selected: Option<&[usize]>,
    start_velocity: u8,
    end_velocity: u8,
) -> Result<usize, Error> {
    if !(1..=127).contains(&start_velocity) || !(1..=127).contains(&end_velocity) {
        return Err(invalid("velocity ramp endpoints must be between 1 and 127"));
    }
    let notes = sequence.notes_mut();
    let mut targets: Vec<usize> = (0..notes.len())
        .filter(|index| is_selected(*index, selected))
        .collect();
    if targets.is_empty() {
        return Ok(0);
    }
    targets.sort_by_key(|&index| (notes[index].start_tick, index));
    let count = targets.len();
    if count == 1 {
        notes[targets[0]].velocity = start_velocity;
        return Ok(1);
    }
    let start = f32::from(start_velocity);
    let end = f32::from(end_velocity);
    for (position, &index) in targets.iter().enumerate() {
        let fraction = position as f32 / (count - 1) as f32;
        notes[index].velocity = (start + (end - start) * fraction).round().clamp(1.0, 127.0) as u8;
    }
    Ok(count)
}

/// Remove exact duplicate notes and controller/automation events while keeping first-occurrence order.
pub fn clean_identical_events(sequence: &mut (impl EventSequence + ?Sized)) -> usize {
    let mut removed = 0;
    removed += dedup_preserving_order(sequence.notes_mut(), |note| {
        (
            note.pitch,
            note.velocity,
            note.start_tick,
            note.duration_ticks,
        )
    });
    removed += dedup_preserving_order(sequence.pitch_bends_mut(), |event| {
        (event.tick, event.value)
    });
    removed += dedup_preserving_order(sequence.aftertouch_mut(), |event| {
        (event.tick, event.note, event.value)
    });
    removed += dedup_preserving_order(sequence.controllers_mut(), |event| {
        (event.tick, event.controller, event.value)
    });
    removed += dedup_preserving_order(sequence.mixer_automation_mut(), |event| {
        (event.tick, event.parameter, event.value.to_bits())
    });
    removed
}

/// Collapse same-tick overwrites and consecutive duplicate values per CC lane.
pub fn thin_controller_events(
    sequence: &mut (impl EventSequence + ?Sized),
    controller_filter: Option<u8>,
) -> usize {
    let controllers = sequence.controllers_mut();
    let original_len = controllers.len();
    let mut untouched = Vec::new();
    let mut matching = Vec::new();
    for event in controllers.drain(..) {
        if controller_filter.is_none_or(|cc| event.controller == cc) {
            matching.push(event);
        } else {
            untouched.push(event);
        }
    }
    let thinned = thin_step_events(
        matching,
        |event| event.tick,
        |event| event.controller,
        |event| event.value,
    );
    untouched.extend(thinned);
    untouched.sort_by_key(|event| event.tick);
    *controllers = untouched;
    original_len - controllers.len()
}

/// Collapse same-tick overwrites and consecutive duplicate values on pitch bend.
pub fn thin_pitch_bends(sequence: &mut (impl EventSequence + ?Sized)) -> usize {
    let bends = sequence.pitch_bends_mut();
    let original_len = bends.len();
    let events = std::mem::take(bends);
    *bends = thin_step_events(events, |event| event.tick, |_| (), |event| event.value);
    original_len - bends.len()
}

/// Collapse same-tick overwrites and consecutive duplicate values per aftertouch target.
pub fn thin_aftertouch(sequence: &mut (impl EventSequence + ?Sized)) -> usize {
    let aftertouch = sequence.aftertouch_mut();
    let original_len = aftertouch.len();
    let events = std::mem::take(aftertouch);
    *aftertouch = thin_step_events(
        events,
        |event| event.tick,
        |event| event.note,
        |event| event.value,
    );
    original_len - aftertouch.len()
}

/// Collapse same-tick overwrites and interior flat points while preserving linear-ramp breakpoints.
pub fn thin_mixer_automation(sequence: &mut (impl EventSequence + ?Sized)) -> usize {
    let automation = sequence.mixer_automation_mut();
    let original_len = automation.len();
    let mut events = std::mem::take(automation);
    events.sort_by_key(|event| event.tick);

    let mut per_parameter: BTreeMap<MixerParameter, Vec<MixerAutomationEvent>> = BTreeMap::new();
    for event in events {
        let lane = per_parameter.entry(event.parameter).or_default();
        if let Some(last) = lane.last_mut()
            && last.tick == event.tick
        {
            *last = event;
        } else {
            lane.push(event);
        }
    }

    let mut kept = Vec::new();
    for (_, lane) in per_parameter {
        for index in 0..lane.len() {
            let interior_flat = index > 0
                && index + 1 < lane.len()
                && lane[index - 1].value == lane[index].value
                && lane[index].value == lane[index + 1].value;
            if !interior_flat {
                kept.push(lane[index]);
            }
        }
    }
    kept.sort_by_key(|event| event.tick);
    *automation = kept;
    original_len - automation.len()
}

/// Delete all pitch-bend, aftertouch, MIDI CC, and mixer-automation events, keeping notes.
pub fn clear_controller_events(sequence: &mut (impl EventSequence + ?Sized)) -> usize {
    let removed = sequence.pitch_bends().len()
        + sequence.aftertouch().len()
        + sequence.controllers().len()
        + sequence.mixer_automation().len();
    sequence.pitch_bends_mut().clear();
    sequence.aftertouch_mut().clear();
    sequence.controllers_mut().clear();
    sequence.mixer_automation_mut().clear();
    removed
}

/// Delete all events for one MIDI controller number, keeping other controllers and notes.
pub fn clear_midi_controller(
    sequence: &mut (impl EventSequence + ?Sized),
    controller: u8,
) -> usize {
    let controllers = sequence.controllers_mut();
    let original_len = controllers.len();
    controllers.retain(|event| event.controller != controller);
    original_len - controllers.len()
}

fn is_selected(index: usize, selected: Option<&[usize]>) -> bool {
    selected.is_none_or(|indices| indices.contains(&index))
}

fn swung_grid_tick(step: u64, step_ticks: f64, swing: f32) -> u64 {
    let swing_offset = if step % 2 == 1 {
        f64::from(swing) * 0.5 * step_ticks
    } else {
        0.0
    };
    (step as f64 * step_ticks + swing_offset).round().max(0.0) as u64
}

fn dedup_preserving_order<T, K: Ord>(items: &mut Vec<T>, key: impl Fn(&T) -> K) -> usize {
    let original_len = items.len();
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert(key(item)));
    original_len - items.len()
}

fn thin_step_events<T, K: Ord, V: PartialEq>(
    mut events: Vec<T>,
    tick: impl Fn(&T) -> u64,
    lane_key: impl Fn(&T) -> K,
    value: impl Fn(&T) -> V,
) -> Vec<T> {
    events.sort_by_key(&tick);
    let mut per_lane: BTreeMap<K, Vec<T>> = BTreeMap::new();
    for event in events {
        let key = lane_key(&event);
        let lane = per_lane.entry(key).or_default();
        if let Some(last) = lane.last_mut()
            && tick(last) == tick(&event)
        {
            *last = event;
        } else {
            lane.push(event);
        }
    }
    let mut kept = Vec::new();
    for (_, lane) in per_lane {
        let mut last_kept: Option<V> = None;
        for event in lane {
            let current = value(&event);
            if last_kept.as_ref().is_some_and(|prev| *prev == current) {
                continue;
            }
            last_kept = Some(current);
            kept.push(event);
        }
    }
    kept.sort_by_key(&tick);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start: u64, duration: u64) -> Note {
        Note {
            pitch,
            start_tick: start,
            duration_ticks: duration,
            ..Note::default()
        }
    }

    #[test]
    fn quantize_snaps_selected_notes_without_underflow() {
        let mut track = Track {
            notes: vec![note(60, 10, 20), note(62, 130, 20)],
            ..Track::default()
        };
        quantize_notes(&mut track, 120, Some(0), None).unwrap();
        assert_eq!(track.notes[0].start_tick, 0);
        assert_eq!(track.notes[0].duration_ticks, 120);
        assert_eq!(track.notes[1].start_tick, 130);
        quantize_notes(&mut track, 120, None, Some(1920)).unwrap();
        assert_eq!(track.notes[1].start_tick, 120);
    }

    #[test]
    fn transpose_is_atomic_at_midi_boundaries() {
        let mut track = Track {
            notes: vec![note(60, 0, 120), note(127, 120, 120)],
            ..Track::default()
        };
        assert!(transpose_notes(&mut track, 1, None).is_err());
        assert_eq!(track.notes[0].pitch, 60);
        transpose_notes(&mut track, 12, Some(0)).unwrap();
        assert_eq!(track.notes[0].pitch, 72);
        assert_eq!(track.notes[1].pitch, 127);
    }

    #[test]
    fn clean_only_shortens_same_pitch_overlaps() {
        let mut track = Track {
            notes: vec![note(60, 120, 240), note(62, 120, 240), note(60, 0, 240)],
            ..Track::default()
        };
        clean_overlaps(&mut track);
        assert_eq!(track.notes[0].duration_ticks, 120);
        assert_eq!(track.notes[1].duration_ticks, 240);
        assert_eq!(track.notes[2].duration_ticks, 240);
    }

    #[test]
    fn clean_preserves_original_order_for_simultaneous_chords() {
        let mut track = Track {
            notes: vec![note(64, 0, 240), note(60, 0, 240), note(67, 0, 240)],
            ..Track::default()
        };
        let original = track.notes.clone();
        clean_overlaps(&mut track);
        assert_eq!(track.notes, original);
    }

    #[test]
    fn advanced_quantize_supports_dotted_triplet_swing_strength_and_selections() {
        let mut straight = Track {
            notes: vec![note(60, 110, 100), note(62, 130, 100)],
            ..Track::default()
        };
        quantize_notes_advanced(
            &mut straight,
            480,
            QuantizeSettings {
                denominator: 16,
                modifier: GridModifier::Straight,
                swing: 1.0,
                strength: 1.0,
                quantize_durations: true,
            },
            Some(&[1]),
            None,
        )
        .unwrap();
        assert_eq!(straight.notes[0].start_tick, 110);
        assert_eq!(straight.notes[1].start_tick, 180);
        assert_eq!(straight.notes[1].duration_ticks, 120);

        let mut dotted = Track {
            notes: vec![note(60, 175, 190)],
            ..Track::default()
        };
        quantize_notes_advanced(
            &mut dotted,
            480,
            QuantizeSettings {
                denominator: 16,
                modifier: GridModifier::Dotted,
                swing: 0.0,
                strength: 1.0,
                quantize_durations: false,
            },
            None,
            None,
        )
        .unwrap();
        assert_eq!(dotted.notes[0].start_tick, 180);
        assert_eq!(dotted.notes[0].duration_ticks, 190);

        let mut triplet = Track {
            notes: vec![note(60, 170, 100)],
            ..Track::default()
        };
        quantize_notes_advanced(
            &mut triplet,
            480,
            QuantizeSettings {
                denominator: 8,
                modifier: GridModifier::Triplet,
                swing: 0.0,
                strength: 0.5,
                quantize_durations: false,
            },
            None,
            None,
        )
        .unwrap();
        assert_eq!(triplet.notes[0].start_tick, 165);
    }

    #[test]
    fn selection_transpose_and_velocity_helpers_operate_on_targeted_notes() {
        let mut track = Track {
            notes: vec![note(60, 200, 120), note(64, 0, 120), note(60, 100, 120)],
            ..Track::default()
        };
        assert_eq!(select_all_notes(&track), [0, 1, 2]);
        let same = select_same_pitch_notes(&track, 60);
        assert_eq!(same, [0, 2]);
        assert_eq!(invert_note_selection(&track, &same), [1]);

        transpose_selected_notes(&mut track, 12, Some(&same)).unwrap();
        assert_eq!(
            track.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            [72, 64, 72]
        );
        set_notes_velocity(&mut track, Some(&[1]), 80).unwrap();
        assert_eq!(track.notes[1].velocity, 80);
        scale_notes_velocity(&mut track, Some(&[1]), 0.5).unwrap();
        assert_eq!(track.notes[1].velocity, 40);
        assert_eq!(
            ramp_notes_velocity(&mut track, Some(&same), 40, 120).unwrap(),
            2
        );
        assert_eq!(track.notes[2].velocity, 40);
        assert_eq!(track.notes[0].velocity, 120);
    }

    #[test]
    fn cleanup_and_thinning_preserve_linear_mixer_ramps_and_note_data() {
        let mut track = Track {
            notes: vec![note(60, 0, 120), note(60, 0, 120), note(62, 120, 120)],
            pitch_bends: vec![
                PitchBend {
                    tick: 0,
                    value: 100,
                },
                PitchBend {
                    tick: 0,
                    value: 200,
                },
                PitchBend {
                    tick: 120,
                    value: 200,
                },
                PitchBend {
                    tick: 240,
                    value: 0,
                },
            ],
            aftertouch: vec![
                Aftertouch {
                    tick: 0,
                    note: None,
                    value: 64,
                },
                Aftertouch {
                    tick: 120,
                    note: None,
                    value: 64,
                },
                Aftertouch {
                    tick: 120,
                    note: Some(60),
                    value: 64,
                },
            ],
            controllers: vec![
                ControllerEvent {
                    tick: 0,
                    controller: 1,
                    value: 50,
                },
                ControllerEvent {
                    tick: 0,
                    controller: 1,
                    value: 50,
                },
                ControllerEvent {
                    tick: 120,
                    controller: 1,
                    value: 50,
                },
                ControllerEvent {
                    tick: 240,
                    controller: 1,
                    value: 80,
                },
                ControllerEvent {
                    tick: 120,
                    controller: 11,
                    value: 100,
                },
            ],
            mixer_automation: vec![
                MixerAutomationEvent {
                    tick: 0,
                    parameter: MixerParameter::Gain,
                    value: 0.5,
                },
                MixerAutomationEvent {
                    tick: 240,
                    parameter: MixerParameter::Gain,
                    value: 0.5,
                },
                MixerAutomationEvent {
                    tick: 480,
                    parameter: MixerParameter::Gain,
                    value: 0.5,
                },
                MixerAutomationEvent {
                    tick: 960,
                    parameter: MixerParameter::Gain,
                    value: 1.0,
                },
            ],
            ..Track::default()
        };

        assert_eq!(clean_identical_events(&mut track), 2);
        assert_eq!(track.notes.len(), 2);
        assert_eq!(track.controllers.len(), 4);

        assert_eq!(thin_controller_events(&mut track, Some(1)), 1);
        assert_eq!(
            track.controllers,
            [
                ControllerEvent {
                    tick: 0,
                    controller: 1,
                    value: 50,
                },
                ControllerEvent {
                    tick: 120,
                    controller: 11,
                    value: 100,
                },
                ControllerEvent {
                    tick: 240,
                    controller: 1,
                    value: 80,
                },
            ]
        );
        assert_eq!(thin_pitch_bends(&mut track), 2);
        assert_eq!(
            track.pitch_bends,
            [
                PitchBend {
                    tick: 0,
                    value: 200
                },
                PitchBend {
                    tick: 240,
                    value: 0
                },
            ]
        );
        assert_eq!(thin_aftertouch(&mut track), 1);
        assert_eq!(track.aftertouch.len(), 2);

        assert_eq!(thin_mixer_automation(&mut track), 1);
        assert_eq!(
            track
                .mixer_automation
                .iter()
                .map(|event| (event.tick, event.value))
                .collect::<Vec<_>>(),
            [(0, 0.5), (480, 0.5), (960, 1.0)]
        );

        assert_eq!(clear_midi_controller(&mut track, 1), 2);
        assert_eq!(track.controllers.len(), 1);
        assert_eq!(clear_controller_events(&mut track), 8);
        assert!(track.pitch_bends.is_empty());
        assert!(track.aftertouch.is_empty());
        assert!(track.controllers.is_empty());
        assert!(track.mixer_automation.is_empty());
        assert_eq!(track.notes.len(), 2);
    }
}
