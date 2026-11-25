//! The automation points of a lane: reading them out of a sequence, and adding, moving,
//! ramping and removing them in order.

use evil_project::{Aftertouch, ControllerEvent, EventSequence, PitchBend};

use eframe::egui::Pos2;
use eframe::egui::Rect;

use super::{AutomationState, HIT_RADIUS, Lane, Point, point_position};

pub(super) fn ramp_grid(ticks_per_quarter: u16, division: u8) -> u64 {
    (u64::from(ticks_per_quarter) * 4 / u64::from(division)).max(1)
}

pub(super) fn next_point(
    track: &dyn EventSequence,
    state: &AutomationState,
    point: Point,
) -> Option<Point> {
    let points = points(track, state);
    let position = points.iter().position(|candidate| *candidate == point)?;
    points.get(position + 1).copied()
}

/// Replace one controller segment with concrete MIDI events so the same ramp
/// is heard by the native renderer and received by external MIDI instruments.
pub(super) fn ramp_controller_to_next(
    track: &mut dyn EventSequence,
    state: &mut AutomationState,
    grid_ticks: u64,
) -> Option<usize> {
    if state.lane != Lane::Controller || grid_ticks == 0 {
        return None;
    }
    let from = selected_point(track, state)?;
    let to = next_point(track, state, from)?;
    if to.tick <= from.tick {
        return None;
    }

    let controller = state.controller;
    let events = track.controllers_mut();
    events.retain(|event| {
        event.controller != controller || event.tick <= from.tick || event.tick >= to.tick
    });
    let duration = to.tick - from.tick;
    let delta = i128::from(to.value - from.value);
    let mut tick = from.tick;
    let mut inserted = 0;
    while let Some(next_tick) = tick.checked_add(grid_ticks) {
        tick = next_tick;
        if tick >= to.tick {
            break;
        }
        let numerator = delta * i128::from(tick - from.tick);
        let rounded = if numerator >= 0 {
            (numerator + i128::from(duration) / 2) / i128::from(duration)
        } else {
            (numerator - i128::from(duration) / 2) / i128::from(duration)
        };
        insert_ordered(
            events,
            ControllerEvent {
                tick,
                controller,
                value: (i128::from(from.value) + rounded) as u8,
            },
            |event| event.tick,
        );
        inserted += 1;
    }
    state.selected = events.iter().position(|event| {
        event.controller == controller && event.tick == from.tick && event.value == from.value as u8
    });
    Some(inserted)
}

pub(super) fn points(track: &dyn EventSequence, state: &AutomationState) -> Vec<Point> {
    match state.lane {
        Lane::PitchBend => track
            .pitch_bends()
            .iter()
            .enumerate()
            .map(|(index, event)| Point {
                index,
                tick: event.tick,
                value: i32::from(event.value),
            })
            .collect(),
        Lane::ChannelAftertouch | Lane::PolyAftertouch => track
            .aftertouch()
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event.note
                    == match state.lane {
                        Lane::ChannelAftertouch => None,
                        _ => Some(state.poly_note),
                    }
            })
            .map(|(index, event)| Point {
                index,
                tick: event.tick,
                value: i32::from(event.value),
            })
            .collect(),
        Lane::Controller => track
            .controllers()
            .iter()
            .enumerate()
            .filter(|(_, event)| event.controller == state.controller)
            .map(|(index, event)| Point {
                index,
                tick: event.tick,
                value: i32::from(event.value),
            })
            .collect(),
        Lane::Mixer => unreachable!("mixer lanes use the native mixer editor"),
    }
}

pub(super) fn selected_point(track: &dyn EventSequence, state: &AutomationState) -> Option<Point> {
    let index = selected_index(track, state)?;
    points(track, state)
        .into_iter()
        .find(|point| point.index == index)
}

pub(super) fn selected_index(track: &dyn EventSequence, state: &AutomationState) -> Option<usize> {
    let index = state.selected?;
    points(track, state)
        .iter()
        .any(|point| point.index == index)
        .then_some(index)
}

pub(super) fn add_point(
    track: &mut dyn EventSequence,
    state: &AutomationState,
    tick: u64,
    value: i32,
) -> usize {
    let value = clamp_value(state.lane, value);
    match state.lane {
        Lane::PitchBend => insert_ordered(
            track.pitch_bends_mut(),
            PitchBend {
                tick,
                value: value as i16,
            },
            |event| event.tick,
        ),
        Lane::ChannelAftertouch | Lane::PolyAftertouch => insert_ordered(
            track.aftertouch_mut(),
            Aftertouch {
                tick,
                note: (state.lane == Lane::PolyAftertouch).then_some(state.poly_note),
                value: value as u8,
            },
            |event| event.tick,
        ),
        Lane::Controller => insert_ordered(
            track.controllers_mut(),
            ControllerEvent {
                tick,
                controller: state.controller,
                value: value as u8,
            },
            |event| event.tick,
        ),
        Lane::Mixer => unreachable!("mixer lanes use the native mixer editor"),
    }
}

pub(super) fn update_point(
    track: &mut dyn EventSequence,
    state: &AutomationState,
    index: usize,
    tick: u64,
    value: i32,
) -> Option<usize> {
    if selected_index(track, state) != Some(index) {
        return None;
    }
    let value = clamp_value(state.lane, value);
    match state.lane {
        Lane::PitchBend => update_ordered(
            track.pitch_bends_mut(),
            index,
            |event| event.tick,
            |event| {
                event.tick = tick;
                event.value = value as i16;
            },
        ),
        Lane::ChannelAftertouch | Lane::PolyAftertouch => update_ordered(
            track.aftertouch_mut(),
            index,
            |event| event.tick,
            |event| {
                event.tick = tick;
                event.value = value as u8;
            },
        ),
        Lane::Controller => update_ordered(
            track.controllers_mut(),
            index,
            |event| event.tick,
            |event| {
                event.tick = tick;
                event.value = value as u8;
            },
        ),
        Lane::Mixer => unreachable!("mixer lanes use the native mixer editor"),
    }
}

pub(super) fn remove_selected(track: &mut dyn EventSequence, state: &mut AutomationState) -> bool {
    let Some(index) = selected_index(track, state) else {
        state.clear_selection();
        return false;
    };
    match state.lane {
        Lane::PitchBend => {
            track.pitch_bends_mut().remove(index);
        }
        Lane::ChannelAftertouch | Lane::PolyAftertouch => {
            track.aftertouch_mut().remove(index);
        }
        Lane::Controller => {
            track.controllers_mut().remove(index);
        }
        Lane::Mixer => unreachable!("mixer lanes use the native mixer editor"),
    }
    state.clear_selection();
    true
}

pub(crate) fn insert_ordered<T>(events: &mut Vec<T>, event: T, tick: fn(&T) -> u64) -> usize {
    let index = events.partition_point(|existing| tick(existing) <= tick(&event));
    events.insert(index, event);
    index
}

pub(crate) fn update_ordered<T: Copy>(
    events: &mut Vec<T>,
    index: usize,
    tick: fn(&T) -> u64,
    update: impl FnOnce(&mut T),
) -> Option<usize> {
    if index >= events.len() {
        return None;
    }
    let mut event = events.remove(index);
    update(&mut event);
    Some(insert_ordered(events, event, tick))
}

pub(super) fn clamp_value(lane: Lane, value: i32) -> i32 {
    match lane {
        Lane::PitchBend => value.clamp(-8192, 8191),
        _ => value.clamp(0, 127),
    }
}

pub(super) fn point_at(
    points: &[Point],
    position: Pos2,
    rect: Rect,
    pixels_per_tick: f32,
    lane: Lane,
) -> Option<Point> {
    points
        .iter()
        .filter(|point| {
            position.distance(point_position(**point, rect, pixels_per_tick, lane)) <= HIT_RADIUS
        })
        .min_by(|a, b| {
            let a_distance = position.distance(point_position(**a, rect, pixels_per_tick, lane));
            let b_distance = position.distance(point_position(**b, rect, pixels_per_tick, lane));
            a_distance.total_cmp(&b_distance)
        })
        .copied()
}
