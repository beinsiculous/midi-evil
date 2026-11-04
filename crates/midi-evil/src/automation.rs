use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::{Aftertouch, ControllerEvent, EventSequence, PitchBend};

use crate::piano_roll::{self, RollState};
use crate::theme;

const LANE_HEIGHT: f32 = 104.0;
const POINT_RADIUS: f32 = 4.0;
const HIT_RADIUS: f32 = 9.0;
const GRID_COLOR: Color32 = theme::BORDER;
const LINE_COLOR: Color32 = theme::PURPLE;
const SELECTED_COLOR: Color32 = theme::IVORY;
const WARNING_COLOR: Color32 = Color32::from_rgb(255, 190, 90);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    PitchBend,
    ChannelAftertouch,
    PolyAftertouch,
    Controller,
}

impl Lane {
    fn label(self) -> &'static str {
        match self {
            Self::PitchBend => "Pitch bend",
            Self::ChannelAftertouch => "Channel aftertouch",
            Self::PolyAftertouch => "Poly aftertouch",
            Self::Controller => "Controller (CC/RPN)",
        }
    }
}

#[derive(Debug)]
pub struct AutomationState {
    lane: Lane,
    controller: u8,
    poly_note: u8,
    ramp_division: u8,
    track_index: usize,
    part_index: Option<usize>,
    selected: Option<usize>,
    dragging: bool,
}

impl Default for AutomationState {
    fn default() -> Self {
        Self {
            lane: Lane::PitchBend,
            controller: 1,
            poly_note: 60,
            ramp_division: 16,
            track_index: 0,
            part_index: None,
            selected: None,
            dragging: false,
        }
    }
}

impl AutomationState {
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.dragging = false;
    }

    fn title(&self) -> String {
        match self.lane {
            Lane::Controller => format!("CC {}", self.controller),
            _ => self.lane.label().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Point {
    index: usize,
    tick: u64,
    value: i32,
}

pub fn show(
    ui: &mut egui::Ui,
    project: &mut evil_project::Project,
    state: &mut AutomationState,
    roll: &RollState,
) -> bool {
    let track_index = roll.selected_track;
    let Some(track) = project.tracks.get(track_index) else {
        state.clear_selection();
        return false;
    };
    let part_index = roll
        .selected_part
        .filter(|index| *index < track.parts.len());
    if state.track_index != track_index || state.part_index != part_index {
        state.track_index = track_index;
        state.part_index = part_index;
        state.clear_selection();
    }

    show_lane_controls(ui, state);
    let context = piano_roll::RollContext::new(project);
    let pattern_index = part_index.map(|index| track.parts[index].pattern);
    let pattern_length = pattern_index.map(|index| project.patterns[index].length_ticks);
    let sequence: &mut dyn EventSequence = match pattern_index {
        Some(index) => &mut project.patterns[index],
        None => &mut project.tracks[track_index],
    };
    let unsupported = unsupported_controller_count(sequence);
    if unsupported > 0 {
        ui.colored_label(
            WARNING_COLOR,
            format!(
                "{} CC/RPN event(s) preserved for MIDI export but not rendered by the synth",
                unsupported
            ),
        );
    } else if state.lane == Lane::Controller
        && !evil_project::controller_is_rendered(state.controller)
    {
        ui.colored_label(
            WARNING_COLOR,
            format!(
                "CC {} has no synth render mapping; points are preserved for MIDI export",
                state.controller
            ),
        );
    }
    ui.label(
        egui::RichText::new(
            "Click to add · Drag to move · Delete to remove · Shares piano-roll zoom",
        )
        .small()
        .color(theme::MUTED),
    );

    let ticks_per_quarter = context.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let bar = context.bar_ticks;
    let snap = (beat / 4).max(1);
    let timeline_ticks =
        pattern_length.unwrap_or_else(|| piano_roll::timeline_ticks(sequence, ticks_per_quarter));
    let pixels_per_tick = roll.pixels_per_tick(ticks_per_quarter);
    let mut changed = egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(LANE_HEIGHT + 16.0)
        .show(ui, |ui| {
            show_lane(
                ui,
                sequence,
                state,
                timeline_ticks,
                pixels_per_tick,
                snap,
                beat,
                bar,
            )
        })
        .inner;
    changed |= show_point_inspector(ui, sequence, state, pattern_length, ticks_per_quarter);
    changed
}

pub fn edit_status(track: &dyn EventSequence, state: &AutomationState) -> String {
    let unsupported = unsupported_controller_count(track);
    if unsupported > 0 {
        format!(
            "{} automation edited; {} CC/RPN event(s) preserved for MIDI export but not rendered",
            state.title(),
            unsupported
        )
    } else {
        format!("{} automation edited", state.title())
    }
}

pub fn render_status(
    mut status: String,
    unsupported_controller_events: usize,
    skipped_drum_notes: usize,
) -> String {
    if unsupported_controller_events > 0 {
        status = format!(
            "{status}; {unsupported_controller_events} CC/RPN event(s) preserved for MIDI export but not rendered"
        );
    }
    if skipped_drum_notes > 0 {
        status = format!(
            "{status}; {skipped_drum_notes} drum note(s) outside the kit mapping or on empty pads"
        );
    }
    status
}

fn show_lane_controls(ui: &mut egui::Ui, state: &mut AutomationState) {
    ui.horizontal_wrapped(|ui| {
        theme::label(ui, "AUTOMATION");
        let previous_lane = state.lane;
        egui::ComboBox::from_id_salt("automation-lane")
            .selected_text(state.lane.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.lane, Lane::PitchBend, "Pitch bend");
                ui.selectable_value(
                    &mut state.lane,
                    Lane::ChannelAftertouch,
                    "Channel aftertouch",
                );
                ui.selectable_value(&mut state.lane, Lane::PolyAftertouch, "Poly aftertouch");
                ui.selectable_value(&mut state.lane, Lane::Controller, "Controller (CC/RPN)");
            });
        if state.lane != previous_lane {
            state.clear_selection();
        }
        match state.lane {
            Lane::PolyAftertouch => {
                ui.label("Note");
                if ui
                    .add(egui::DragValue::new(&mut state.poly_note).range(0..=127))
                    .changed()
                {
                    state.clear_selection();
                }
            }
            Lane::Controller => {
                let previous = state.controller;
                egui::ComboBox::from_id_salt("automation-controller")
                    .selected_text(controller_name(state.controller).unwrap_or("Custom controller"))
                    .show_ui(ui, |ui| {
                        for controller in [1, 11, 20, 21, 22, 23, 24, 25, 64, 6, 38, 100, 101] {
                            ui.selectable_value(
                                &mut state.controller,
                                controller,
                                format!(
                                    "CC {controller} · {}",
                                    controller_name(controller).unwrap()
                                ),
                            );
                        }
                    });
                if state.controller != previous {
                    state.clear_selection();
                }
                ui.label("CC");
                if ui
                    .add(egui::DragValue::new(&mut state.controller).range(0..=127))
                    .changed()
                {
                    state.clear_selection();
                }
                ui.label("Ramp grid");
                egui::ComboBox::from_id_salt("automation-ramp-grid")
                    .selected_text(format!("1/{}", state.ramp_division))
                    .show_ui(ui, |ui| {
                        for division in [4, 8, 16, 32, 64] {
                            ui.selectable_value(
                                &mut state.ramp_division,
                                division,
                                format!("1/{division}"),
                            );
                        }
                    })
                    .response
                    .on_hover_text("Note grid for points inserted by Ramp to next");
            }
            _ => {}
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn show_lane(
    ui: &mut egui::Ui,
    track: &mut dyn EventSequence,
    state: &mut AutomationState,
    timeline_ticks: u64,
    pixels_per_tick: f32,
    snap: u64,
    beat: u64,
    bar: u64,
) -> bool {
    let size = Vec2::new(timeline_ticks as f32 * pixels_per_tick, LANE_HEIGHT);
    let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, theme::BACKGROUND);
    draw_grid(
        &painter,
        rect,
        timeline_ticks,
        pixels_per_tick,
        beat,
        bar,
        state.lane,
    );

    let points = points(track, state);
    for pair in points.windows(2) {
        let from = point_position(pair[0], rect, pixels_per_tick, state.lane);
        let to = point_position(pair[1], rect, pixels_per_tick, state.lane);
        let corner = Pos2::new(to.x, from.y);
        painter.line_segment([from, corner], Stroke::new(1.5, LINE_COLOR));
        painter.line_segment([corner, to], Stroke::new(1.5, LINE_COLOR));
    }
    for point in &points {
        let selected = state.selected == Some(point.index);
        painter.circle_filled(
            point_position(*point, rect, pixels_per_tick, state.lane),
            if selected {
                POINT_RADIUS + 1.0
            } else {
                POINT_RADIUS
            },
            if selected { SELECTED_COLOR } else { LINE_COLOR },
        );
    }

    let mut changed = false;
    if response.drag_started()
        && let Some(position) = ui.input(|input| input.pointer.press_origin())
    {
        state.selected =
            point_at(&points, position, rect, pixels_per_tick, state.lane).map(|point| point.index);
        state.dragging = state.selected.is_some();
    }
    if response.dragged()
        && state.dragging
        && let Some(position) = response.interact_pointer_pos()
        && let Some(index) = selected_index(track, state)
    {
        let tick = position_to_tick(position, rect, pixels_per_tick, snap)
            .min(timeline_ticks.saturating_sub(1));
        let value = value_at(position, rect, state.lane);
        if let Some(point) = points.iter().copied().find(|point| point.index == index)
            && (point.tick != tick || point.value != value)
        {
            state.selected = update_point(track, state, index, tick, value);
            changed = true;
        }
    }
    if response.drag_stopped() {
        state.dragging = false;
    }
    if response.clicked()
        && let Some(position) = response.interact_pointer_pos()
    {
        if let Some(point) = point_at(&points, position, rect, pixels_per_tick, state.lane) {
            state.selected = Some(point.index);
        } else {
            let tick = position_to_tick(position, rect, pixels_per_tick, snap)
                .min(timeline_ticks.saturating_sub(1));
            let value = value_at(position, rect, state.lane);
            state.selected = Some(add_point(track, state, tick, value));
            changed = true;
        }
    }
    if response.hovered()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|input| input.key_pressed(egui::Key::Delete))
        && remove_selected(track, state)
    {
        changed = true;
    }
    changed
}

fn show_point_inspector(
    ui: &mut egui::Ui,
    track: &mut dyn EventSequence,
    state: &mut AutomationState,
    length_ticks: Option<u64>,
    ticks_per_quarter: u16,
) -> bool {
    let Some(point) = selected_point(track, state) else {
        ui.label("No automation point selected");
        return false;
    };
    let mut tick = point.tick;
    let mut value = point.value;
    let mut changed = false;
    let mut remove = false;
    let mut ramp = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(state.title());
        ui.label("Tick");
        changed |= ui
            .add(
                egui::DragValue::new(&mut tick)
                    .range(0..=length_ticks.unwrap_or(u64::MAX).saturating_sub(1)),
            )
            .changed();
        ui.label("Value");
        changed |= match state.lane {
            Lane::PitchBend => ui
                .add(egui::DragValue::new(&mut value).range(-8192..=8191))
                .changed(),
            _ => ui
                .add(egui::DragValue::new(&mut value).range(0..=127))
                .changed(),
        };
        remove = ui.button("Delete point").clicked();
        if state.lane == Lane::Controller {
            let grid = ramp_grid(ticks_per_quarter, state.ramp_division);
            let has_room = next_point(track, state, point)
                .is_some_and(|next| next.tick > point.tick && grid < next.tick - point.tick);
            ramp = ui
                .add_enabled(has_room, egui::Button::new("Ramp to next"))
                .on_hover_text(
                    "Replace this CC segment with linearly interpolated points on the ramp grid",
                )
                .clicked();
        }
    });
    if remove {
        return remove_selected(track, state);
    }
    if changed {
        state.selected = update_point(track, state, point.index, tick, value);
    }
    if ramp {
        return ramp_controller_to_next(
            track,
            state,
            ramp_grid(ticks_per_quarter, state.ramp_division),
        )
        .is_some();
    }
    changed
}

fn ramp_grid(ticks_per_quarter: u16, division: u8) -> u64 {
    (u64::from(ticks_per_quarter) * 4 / u64::from(division)).max(1)
}

fn next_point(track: &dyn EventSequence, state: &AutomationState, point: Point) -> Option<Point> {
    let points = points(track, state);
    let position = points.iter().position(|candidate| *candidate == point)?;
    points.get(position + 1).copied()
}

/// Replace one controller segment with concrete MIDI events so the same ramp
/// is heard by the native renderer and received by external MIDI instruments.
fn ramp_controller_to_next(
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

fn points(track: &dyn EventSequence, state: &AutomationState) -> Vec<Point> {
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
    }
}

fn selected_point(track: &dyn EventSequence, state: &AutomationState) -> Option<Point> {
    let index = selected_index(track, state)?;
    points(track, state)
        .into_iter()
        .find(|point| point.index == index)
}

fn selected_index(track: &dyn EventSequence, state: &AutomationState) -> Option<usize> {
    let index = state.selected?;
    points(track, state)
        .iter()
        .any(|point| point.index == index)
        .then_some(index)
}

fn add_point(
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
    }
}

fn update_point(
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
    }
}

fn remove_selected(track: &mut dyn EventSequence, state: &mut AutomationState) -> bool {
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
    }
    state.clear_selection();
    true
}

fn insert_ordered<T>(events: &mut Vec<T>, event: T, tick: fn(&T) -> u64) -> usize {
    let index = events.partition_point(|existing| tick(existing) <= tick(&event));
    events.insert(index, event);
    index
}

fn update_ordered<T: Copy>(
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

fn point_at(
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

fn point_position(point: Point, rect: Rect, pixels_per_tick: f32, lane: Lane) -> Pos2 {
    Pos2::new(
        rect.left() + point.tick as f32 * pixels_per_tick,
        value_position(point.value, rect, lane),
    )
}

fn position_to_tick(position: Pos2, rect: Rect, pixels_per_tick: f32, snap: u64) -> u64 {
    let tick = ((position.x - rect.left()) / pixels_per_tick).max(0.0) as u64;
    piano_roll::snap_tick(tick, snap)
}

fn value_at(position: Pos2, rect: Rect, lane: Lane) -> i32 {
    let normalized = ((rect.bottom() - position.y) / rect.height()).clamp(0.0, 1.0);
    match lane {
        Lane::PitchBend => (-8192.0 + normalized * 16_383.0).round() as i32,
        _ => (normalized * 127.0).round() as i32,
    }
}

fn value_position(value: i32, rect: Rect, lane: Lane) -> f32 {
    let normalized = match lane {
        Lane::PitchBend => (value + 8192) as f32 / 16_383.0,
        _ => value as f32 / 127.0,
    }
    .clamp(0.0, 1.0);
    rect.bottom() - normalized * rect.height()
}

fn clamp_value(lane: Lane, value: i32) -> i32 {
    match lane {
        Lane::PitchBend => value.clamp(-8192, 8191),
        _ => value.clamp(0, 127),
    }
}

fn draw_grid(
    painter: &egui::Painter,
    rect: Rect,
    timeline_ticks: u64,
    pixels_per_tick: f32,
    beat: u64,
    bar: u64,
    lane: Lane,
) {
    for division in 0..=4 {
        let y = rect.top() + rect.height() * division as f32 / 4.0;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, GRID_COLOR),
        );
    }
    let mut tick = 0;
    while tick <= timeline_ticks {
        let x = rect.left() + tick as f32 * pixels_per_tick;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(if tick % bar == 0 { 1.0 } else { 0.5 }, GRID_COLOR),
        );
        tick += beat;
    }
    if lane == Lane::PitchBend {
        let y = value_position(0, rect, lane);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0, Color32::from_rgb(90, 115, 125)),
        );
    }
}

fn unsupported_controller_count(track: &dyn EventSequence) -> usize {
    track
        .controllers()
        .iter()
        .filter(|event| !evil_project::controller_is_rendered(event.controller))
        .count()
}

fn controller_name(controller: u8) -> Option<&'static str> {
    match controller {
        0 => Some("Bank select MSB · not rendered"),
        1 => Some("Modulation · rendered"),
        2 => Some("Breath · not rendered"),
        6 => Some("RPN data MSB · not rendered"),
        7 => Some("Channel volume · not rendered as automation"),
        10 => Some("Pan · not rendered as automation"),
        11 => Some("Expression · rendered"),
        evil_synth::WAVESHAPER_DRIVE_CC => Some("Waveshaper drive"),
        evil_synth::WAVESHAPER_CONTOUR_CC => Some("Waveshaper contour"),
        evil_synth::WAVESHAPER_LEVEL_CC => Some("Waveshaper level"),
        evil_synth::DELAY_TIME_CC => Some("Delay time"),
        evil_synth::DELAY_FEEDBACK_CC => Some("Delay feedback"),
        evil_synth::DELAY_MIX_CC => Some("Delay wet mix"),
        32 => Some("Bank select LSB · not rendered"),
        38 => Some("RPN data LSB · not rendered"),
        64 => Some("Sustain · not rendered"),
        98 => Some("NRPN LSB · not rendered"),
        99 => Some("NRPN MSB · not rendered"),
        100 => Some("RPN LSB · not rendered"),
        101 => Some("RPN MSB · not rendered"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::Track;

    #[test]
    fn edits_keep_combined_aftertouch_lanes_ordered_and_scoped() {
        let mut track = Track::default();
        let mut state = AutomationState {
            lane: Lane::PolyAftertouch,
            poly_note: 60,
            ..AutomationState::default()
        };
        add_point(&mut track, &state, 480, 80);
        state.lane = Lane::ChannelAftertouch;
        let channel = add_point(&mut track, &state, 240, 40);
        state.selected = Some(channel);
        state.selected = update_point(&mut track, &state, channel, 720, 90);
        assert_eq!(
            track
                .aftertouch
                .iter()
                .map(|event| (event.tick, event.note, event.value))
                .collect::<Vec<_>>(),
            vec![(480, Some(60), 80), (720, None, 90)]
        );
        assert_eq!(points(&track, &state).len(), 1);
        state.lane = Lane::PolyAftertouch;
        assert_eq!(points(&track, &state).len(), 1);
        state.poly_note = 61;
        assert!(points(&track, &state).is_empty());
    }

    #[test]
    fn pitch_bend_and_controller_values_stay_in_midi_ranges() {
        let mut track = Track::default();
        let mut state = AutomationState::default();
        let index = add_point(&mut track, &state, 0, 20_000);
        assert_eq!(track.pitch_bends_mut()[index].value, 8191);
        state.selected = Some(index);
        update_point(&mut track, &state, index, 0, -20_000);
        assert_eq!(track.pitch_bends_mut()[0].value, -8192);

        state.lane = Lane::Controller;
        state.controller = 2;
        let index = add_point(&mut track, &state, 0, 200);
        assert_eq!(track.controllers_mut()[index].value, 127);
        assert_eq!(unsupported_controller_count(&track), 1);
        assert!(render_status("Preview".to_owned(), 1, 0).contains("not rendered"));
        assert!(render_status("Preview".to_owned(), 0, 2).contains("outside the kit"));
    }

    #[test]
    fn deleting_one_cc_lane_preserves_other_controllers() {
        let mut track = Track::default();
        let mut state = AutomationState {
            lane: Lane::Controller,
            controller: 1,
            ..AutomationState::default()
        };
        add_point(&mut track, &state, 0, 20);
        state.controller = 2;
        let index = add_point(&mut track, &state, 120, 30);
        state.selected = Some(index);
        assert!(remove_selected(&mut track, &mut state));
        assert_eq!(track.controllers_mut().len(), 1);
        assert_eq!(track.controllers_mut()[0].controller, 1);
        assert!(state.selected.is_none());
    }

    #[test]
    fn controller_ramp_materializes_midi_points_without_touching_other_lanes() {
        let mut track = Track {
            controllers: vec![
                ControllerEvent {
                    tick: 0,
                    controller: 1,
                    value: 0,
                },
                ControllerEvent {
                    tick: 120,
                    controller: 11,
                    value: 90,
                },
                ControllerEvent {
                    tick: 480,
                    controller: 1,
                    value: 100,
                },
                ControllerEvent {
                    tick: 960,
                    controller: 1,
                    value: 0,
                },
            ],
            ..Track::default()
        };
        let mut state = AutomationState {
            lane: Lane::Controller,
            controller: 1,
            selected: Some(0),
            ..AutomationState::default()
        };

        assert_eq!(
            ramp_controller_to_next(&mut track, &mut state, 120),
            Some(3)
        );
        let lane: Vec<_> = points(&track, &state)
            .into_iter()
            .map(|point| (point.tick, point.value))
            .collect();
        assert_eq!(
            lane,
            [
                (0, 0),
                (120, 25),
                (240, 50),
                (360, 75),
                (480, 100),
                (960, 0)
            ]
        );
        assert!(
            track
                .controllers
                .iter()
                .any(|event| { event.controller == 11 && event.tick == 120 && event.value == 90 })
        );
        assert_eq!(state.selected, Some(0));
    }

    #[test]
    fn descending_controller_ramp_rounds_values_and_preserves_endpoints() {
        let mut track = Track::default();
        let mut state = AutomationState {
            lane: Lane::Controller,
            controller: 1,
            ..AutomationState::default()
        };
        add_point(&mut track, &state, 0, 127);
        add_point(&mut track, &state, 480, 0);
        state.selected = Some(0);

        assert_eq!(
            ramp_controller_to_next(&mut track, &mut state, 120),
            Some(3)
        );
        assert_eq!(
            track
                .controllers
                .iter()
                .map(|event| (event.tick, event.value))
                .collect::<Vec<_>>(),
            [(0, 127), (120, 95), (240, 63), (360, 32), (480, 0)]
        );
        assert_eq!(ramp_grid(480, 16), 120);
        assert_eq!(ramp_grid(384, 64), 24);
    }
}
