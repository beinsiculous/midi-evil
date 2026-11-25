use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::EventSequence;

use crate::piano_roll::{self, RollState};
use crate::theme;

mod points;

use points::{
    add_point, next_point, point_at, points, ramp_controller_to_next, ramp_grid, remove_selected,
    selected_index, selected_point, update_point,
};
pub(crate) use points::{insert_ordered, update_ordered};

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
    Mixer,
}

impl Lane {
    fn label(self) -> &'static str {
        match self {
            Self::PitchBend => "Pitch bend",
            Self::ChannelAftertouch => "Channel aftertouch",
            Self::PolyAftertouch => "Poly aftertouch",
            Self::Controller => "Controller (CC/RPN)",
            Self::Mixer => "Mixer (native)",
        }
    }
}

#[derive(Debug)]
pub struct AutomationState {
    lane: Lane,
    mixer: crate::mixer_automation::MixerAutomationState,
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
            mixer: crate::mixer_automation::MixerAutomationState::default(),
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
        self.mixer.clear_selection();
    }

    fn title(&self) -> String {
        match self.lane {
            Lane::Controller => format!("CC {}", self.controller),
            Lane::Mixer => self.mixer.title(),
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
    let mixer_targets =
        (state.lane == Lane::Mixer).then(|| crate::mixer_automation::targets(project, track_index));
    let pattern_length = pattern_index.map(|index| project.patterns[index].length_ticks);
    let sequence: &mut dyn EventSequence = match pattern_index {
        Some(index) => &mut project.patterns[index],
        None => &mut project.tracks[track_index],
    };
    if let Some(targets) = mixer_targets {
        return crate::mixer_automation::show(
            ui,
            sequence,
            &mut state.mixer,
            &targets,
            roll,
            context,
            pattern_length,
        );
    }
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
    if state.lane == Lane::Mixer {
        return format!("{} automation edited", state.title());
    }
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
    unsupported_mixer_automation_events: usize,
    skipped_drum_notes: usize,
) -> String {
    if unsupported_controller_events > 0 {
        status = format!(
            "{status}; {unsupported_controller_events} CC/RPN event(s) preserved for MIDI export but not rendered"
        );
    }
    if unsupported_mixer_automation_events > 0 {
        status = format!(
            "{status}; {unsupported_mixer_automation_events} mixer automation event(s) target missing or different inserts and were not rendered"
        );
    }
    if skipped_drum_notes > 0 {
        status = format!(
            "{status}; {skipped_drum_notes} drum note(s) outside the kit mapping or on empty pads"
        );
    }
    status
}

pub fn midi_export_status(mut status: String, native_only_events: usize) -> String {
    if native_only_events > 0 {
        status = format!(
            "{status}; {native_only_events} typed effect/send automation event(s) remain in the .evil project and are not MIDI events"
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
                ui.selectable_value(&mut state.lane, Lane::Mixer, "Mixer (native)");
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
    // A lane painter never takes focus by itself; without it a clicked point could not be
    // deleted, or the Delete would go to whichever editor still had focus.
    if response.clicked_by(egui::PointerButton::Primary) || response.drag_started() {
        response.request_focus();
    }
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
    crate::custom_editors::register(ui.ctx(), response.id);
    let name = crate::speech::describe_lane("Automation lane", &state.title(), points.len());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Canvas);
        node.set_label(name);
    });
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
    // Delete belongs to the focused editor alone, so one key can never delete in two places.
    if response.has_focus()
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
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

fn point_position(point: Point, rect: Rect, pixels_per_tick: f32, lane: Lane) -> Pos2 {
    Pos2::new(
        rect.left() + point.tick as f32 * pixels_per_tick,
        value_position(point.value, rect, lane),
    )
}

pub(crate) fn position_to_tick(position: Pos2, rect: Rect, pixels_per_tick: f32, snap: u64) -> u64 {
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
pub(crate) mod tests;
