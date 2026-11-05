use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};
use evil_project::{EventSequence, MixerAutomationEvent, MixerParameter, Project};

use crate::{
    automation::{insert_ordered, position_to_tick, update_ordered},
    effect_ui::EffectType,
    piano_roll::{self, RollContext, RollState},
    theme,
};

const LANE_HEIGHT: f32 = 104.0;
const POINT_RADIUS: f32 = 4.0;
const HIT_RADIUS: f32 = 9.0;

#[derive(Debug)]
pub struct MixerAutomationState {
    parameter: MixerParameter,
    selected: Option<usize>,
    dragging: bool,
}

impl Default for MixerAutomationState {
    fn default() -> Self {
        Self {
            parameter: MixerParameter::Gain,
            selected: None,
            dragging: false,
        }
    }
}

impl MixerAutomationState {
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.dragging = false;
    }

    pub fn title(&self) -> String {
        match self.parameter {
            MixerParameter::Gain => "Channel gain".to_owned(),
            MixerParameter::Pan => "Channel pan".to_owned(),
            MixerParameter::SendGain { bus } => format!("Send {} gain", bus + 1),
            MixerParameter::Insert { slot, parameter } => {
                format!("Insert {} {}", slot + 1, parameter.label())
            }
        }
    }
}

pub struct MixerTarget {
    parameter: MixerParameter,
    title: String,
    value: f32,
}

pub fn targets(project: &Project, track_index: usize) -> Vec<MixerTarget> {
    let mut targets = vec![
        MixerTarget {
            parameter: MixerParameter::Gain,
            title: "Channel gain".to_owned(),
            value: 0.8,
        },
        MixerTarget {
            parameter: MixerParameter::Pan,
            title: "Channel pan".to_owned(),
            value: 0.0,
        },
        MixerTarget {
            parameter: MixerParameter::SendGain { bus: 0 },
            title: "Send A gain".to_owned(),
            value: 0.0,
        },
        MixerTarget {
            parameter: MixerParameter::SendGain { bus: 1 },
            title: "Send B gain".to_owned(),
            value: 0.0,
        },
    ];
    let Ok(instrument) = project.instrument_for_track(track_index) else {
        return targets;
    };
    targets[0].value = instrument.gain;
    targets[1].value = instrument.pan;
    targets[2].value = instrument.send_gains[0];
    targets[3].value = instrument.send_gains[1];
    for (slot, effect) in instrument.inserts.iter().enumerate() {
        for parameter in effect.automatable_parameters() {
            let parameter = *parameter;
            targets.push(MixerTarget {
                parameter: MixerParameter::Insert {
                    slot: slot as u8,
                    parameter,
                },
                title: format!(
                    "Insert {} · {} · {}",
                    slot + 1,
                    EffectType::of(effect).label(),
                    parameter.label()
                ),
                value: evil_synth::effect_parameter(effect, parameter).unwrap_or(0.0),
            });
        }
    }
    targets
}

pub fn show(
    ui: &mut egui::Ui,
    sequence: &mut dyn EventSequence,
    state: &mut MixerAutomationState,
    targets: &[MixerTarget],
    roll: &RollState,
    context: RollContext,
    pattern_length: Option<u64>,
) -> bool {
    if !targets
        .iter()
        .any(|target| target.parameter == state.parameter)
    {
        state.parameter = MixerParameter::Gain;
        state.clear_selection();
    }
    let selected_title = targets
        .iter()
        .find(|target| target.parameter == state.parameter)
        .map_or_else(|| state.title(), |target| target.title.clone());
    ui.horizontal_wrapped(|ui| {
        theme::label(ui, "NATIVE MIXER PARAMETER");
        egui::ComboBox::from_id_salt("mixer-automation-parameter")
            .selected_text(selected_title)
            .show_ui(ui, |ui| {
                for target in targets {
                    if ui
                        .selectable_label(target.parameter == state.parameter, &target.title)
                        .clicked()
                    {
                        state.parameter = target.parameter;
                        state.clear_selection();
                        ui.close();
                    }
                }
            });
    });
    ui.label(
        egui::RichText::new(
            "Click to add · Drag to move · Delete to remove · Linear ramps · Gain/pan export as CC 7/10; effect/send lanes stay in .evil",
        )
        .small()
        .color(theme::MUTED),
    );

    let ticks_per_quarter = context.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let snap = (beat / 4).max(1);
    let timeline_ticks =
        pattern_length.unwrap_or_else(|| piano_roll::timeline_ticks(sequence, ticks_per_quarter));
    let pixels_per_tick = roll.pixels_per_tick(ticks_per_quarter);
    let mut changed = egui::ScrollArea::both()
        .id_salt("mixer-automation-lane")
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
                context.bar_ticks,
            )
        })
        .inner;
    changed |= show_point_inspector(ui, sequence, state, pattern_length);
    changed
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    index: usize,
    tick: u64,
    value: f32,
}

#[allow(clippy::too_many_arguments)]
fn show_lane(
    ui: &mut egui::Ui,
    sequence: &mut dyn EventSequence,
    state: &mut MixerAutomationState,
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
    for division in 0..=4 {
        let y = rect.top() + rect.height() * division as f32 / 4.0;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, theme::BORDER),
        );
    }
    let mut tick = 0;
    while tick <= timeline_ticks {
        let x = rect.left() + tick as f32 * pixels_per_tick;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(if tick % bar == 0 { 1.0 } else { 0.5 }, theme::BORDER),
        );
        tick += beat;
    }

    let points = points(sequence, state);
    for pair in points.windows(2) {
        painter.line_segment(
            [
                point_position(pair[0], rect, pixels_per_tick, state.parameter),
                point_position(pair[1], rect, pixels_per_tick, state.parameter),
            ],
            Stroke::new(1.5, theme::PURPLE),
        );
    }
    for point in &points {
        let selected = state.selected == Some(point.index);
        painter.circle_filled(
            point_position(*point, rect, pixels_per_tick, state.parameter),
            if selected {
                POINT_RADIUS + 1.0
            } else {
                POINT_RADIUS
            },
            if selected {
                theme::IVORY
            } else {
                theme::PURPLE
            },
        );
    }

    let mut changed = false;
    if response.drag_started()
        && let Some(position) = ui.input(|input| input.pointer.press_origin())
    {
        state.selected = point_at(&points, position, rect, pixels_per_tick, state.parameter)
            .map(|point| point.index);
        state.dragging = state.selected.is_some();
    }
    if response.dragged()
        && state.dragging
        && let Some(position) = response.interact_pointer_pos()
        && let Some(index) = selected_index(sequence, state)
    {
        let tick = position_to_tick(position, rect, pixels_per_tick, snap)
            .min(timeline_ticks.saturating_sub(1));
        let value = value_at(position, rect, state.parameter);
        if let Some(point) = points.iter().copied().find(|point| point.index == index)
            && (point.tick != tick || point.value != value)
        {
            state.selected = update_point(sequence, state, index, tick, value);
            changed = true;
        }
    }
    if response.drag_stopped() {
        state.dragging = false;
    }
    if response.clicked()
        && let Some(position) = response.interact_pointer_pos()
    {
        if let Some(point) = point_at(&points, position, rect, pixels_per_tick, state.parameter) {
            state.selected = Some(point.index);
        } else {
            let tick = position_to_tick(position, rect, pixels_per_tick, snap)
                .min(timeline_ticks.saturating_sub(1));
            let value = value_at(position, rect, state.parameter);
            state.selected = Some(add_point(sequence, state, tick, value));
            changed = true;
        }
    }
    if response.hovered()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|input| input.key_pressed(egui::Key::Delete))
        && remove_selected(sequence, state)
    {
        changed = true;
    }
    changed
}

fn show_point_inspector(
    ui: &mut egui::Ui,
    sequence: &mut dyn EventSequence,
    state: &mut MixerAutomationState,
    length_ticks: Option<u64>,
) -> bool {
    let Some(point) = selected_point(sequence, state) else {
        ui.label("No mixer automation point selected");
        return false;
    };
    let mut tick = point.tick;
    let mut value = point.value;
    let mut changed = false;
    let mut remove = false;
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
        let range = state.parameter.range();
        let speed = ((*range.end() - *range.start()) / 100.0).max(0.000_1);
        changed |= if state.parameter.is_integer() {
            ui.add(egui::DragValue::new(&mut value).range(range).speed(1.0))
                .changed()
        } else {
            ui.add(egui::DragValue::new(&mut value).range(range).speed(speed))
                .changed()
        };
        remove = ui.button("Delete point").clicked();
    });
    if remove {
        return remove_selected(sequence, state);
    }
    if changed {
        let value = if state.parameter.is_integer() {
            value.round()
        } else {
            value
        };
        state.selected = update_point(sequence, state, point.index, tick, value);
    }
    changed
}

fn points(sequence: &dyn EventSequence, state: &MixerAutomationState) -> Vec<Point> {
    sequence
        .mixer_automation()
        .iter()
        .enumerate()
        .filter(|(_, event)| event.parameter == state.parameter)
        .map(|(index, event)| Point {
            index,
            tick: event.tick,
            value: event.value,
        })
        .collect()
}

fn selected_point(sequence: &dyn EventSequence, state: &MixerAutomationState) -> Option<Point> {
    let index = selected_index(sequence, state)?;
    points(sequence, state)
        .into_iter()
        .find(|point| point.index == index)
}

fn selected_index(sequence: &dyn EventSequence, state: &MixerAutomationState) -> Option<usize> {
    let index = state.selected?;
    points(sequence, state)
        .iter()
        .any(|point| point.index == index)
        .then_some(index)
}

fn add_point(
    sequence: &mut dyn EventSequence,
    state: &MixerAutomationState,
    tick: u64,
    value: f32,
) -> usize {
    let range = state.parameter.range();
    let value = value.clamp(*range.start(), *range.end());
    let value = if state.parameter.is_integer() {
        value.round()
    } else {
        value
    };
    insert_ordered(
        sequence.mixer_automation_mut(),
        MixerAutomationEvent {
            tick,
            parameter: state.parameter,
            value,
        },
        |event| event.tick,
    )
}

fn update_point(
    sequence: &mut dyn EventSequence,
    state: &MixerAutomationState,
    index: usize,
    tick: u64,
    value: f32,
) -> Option<usize> {
    if selected_index(sequence, state) != Some(index) {
        return None;
    }
    let range = state.parameter.range();
    let value = value.clamp(*range.start(), *range.end());
    let value = if state.parameter.is_integer() {
        value.round()
    } else {
        value
    };
    update_ordered(
        sequence.mixer_automation_mut(),
        index,
        |event| event.tick,
        |event| {
            event.tick = tick;
            event.value = value;
        },
    )
}

fn remove_selected(sequence: &mut dyn EventSequence, state: &mut MixerAutomationState) -> bool {
    let Some(index) = selected_index(sequence, state) else {
        state.clear_selection();
        return false;
    };
    sequence.mixer_automation_mut().remove(index);
    state.clear_selection();
    true
}

fn point_at(
    points: &[Point],
    position: Pos2,
    rect: Rect,
    pixels_per_tick: f32,
    parameter: MixerParameter,
) -> Option<Point> {
    points
        .iter()
        .filter(|point| {
            position.distance(point_position(**point, rect, pixels_per_tick, parameter))
                <= HIT_RADIUS
        })
        .min_by(|a, b| {
            let a_distance =
                position.distance(point_position(**a, rect, pixels_per_tick, parameter));
            let b_distance =
                position.distance(point_position(**b, rect, pixels_per_tick, parameter));
            a_distance.total_cmp(&b_distance)
        })
        .copied()
}

fn point_position(
    point: Point,
    rect: Rect,
    pixels_per_tick: f32,
    parameter: MixerParameter,
) -> Pos2 {
    Pos2::new(
        rect.left() + point.tick as f32 * pixels_per_tick,
        value_position(point.value, rect, parameter),
    )
}

fn value_at(position: Pos2, rect: Rect, parameter: MixerParameter) -> f32 {
    let normalized = ((rect.bottom() - position.y) / rect.height()).clamp(0.0, 1.0);
    let range = parameter.range();
    let value = *range.start() + normalized * (*range.end() - *range.start());
    if parameter.is_integer() {
        value.round()
    } else {
        value
    }
}

fn value_position(value: f32, rect: Rect, parameter: MixerParameter) -> f32 {
    let range = parameter.range();
    let normalized = ((value - *range.start()) / (*range.end() - *range.start())).clamp(0.0, 1.0);
    rect.bottom() - normalized * rect.height()
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::{EffectParameter, Track};

    #[test]
    fn typed_point_edits_preserve_other_lanes_and_integer_values() {
        let mut track = Track::default();
        let mut state = MixerAutomationState::default();
        add_point(&mut track, &state, 480, 0.75);
        state.parameter = MixerParameter::Insert {
            slot: 0,
            parameter: EffectParameter::BitcrushBits,
        };
        let bits = add_point(&mut track, &state, 240, 7.5);
        state.selected = Some(bits);
        assert_eq!(track.mixer_automation[bits].value, 8.0);
        state.selected = update_point(&mut track, &state, bits, 120, 8.0);
        assert_eq!(track.mixer_automation[state.selected.unwrap()].tick, 120);
        assert!(
            track
                .mixer_automation
                .iter()
                .any(|event| event.parameter == MixerParameter::Gain && event.value == 0.75)
        );
        assert!(remove_selected(&mut track, &mut state));
        assert_eq!(track.mixer_automation.len(), 1);
        assert_eq!(track.mixer_automation[0].value, 0.75);
    }

    #[test]
    fn parameter_values_use_typed_ranges() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        let pan = value_at(Pos2::new(0.0, 50.0), rect, MixerParameter::Pan);
        assert!(pan.abs() < 0.01);
        let bits = value_at(
            Pos2::new(0.0, 0.0),
            rect,
            MixerParameter::Insert {
                slot: 0,
                parameter: EffectParameter::BitcrushBits,
            },
        );
        assert_eq!(bits, 16.0);
        assert_eq!(value_position(0.0, rect, MixerParameter::Pan), 50.0);
    }
}
