use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, vec2};
use evil_project::{Part, Pattern, Project};

use crate::{
    piano_roll::{self, PartDragState, RollState},
    theme,
    transport::Transport,
};

#[derive(Default)]
pub struct SongResult {
    pub edit_notes: bool,
    pub status: Option<String>,
}

pub fn show(
    ui: &mut egui::Ui,
    project: &mut Project,
    roll: &mut RollState,
    transport: &mut Transport,
    playhead_tick: u64,
    allow_structural_edits: bool,
) -> SongResult {
    let mut result = SongResult::default();
    let bar = piano_roll::bar_ticks(project);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Arranger");
        if ui
            .add_enabled(allow_structural_edits, egui::Button::new("+ Pattern part"))
            .clicked()
        {
            let pattern_index = project.patterns.len();
            let pattern = Pattern {
                name: format!("Pattern {}", pattern_index + 1),
                length_ticks: bar,
                ..Pattern::default()
            };
            match project.add_pattern(pattern) {
                Ok(pattern) => {
                    let part = Part {
                        pattern,
                        start_tick: transport.start_tick,
                        length_ticks: bar,
                    };
                    match project.add_part(roll.selected_track, part) {
                        Ok(part) => {
                            roll.select_part(roll.selected_track, Some(part));
                            result.status =
                                Some(format!("Created shared pattern {}", pattern_index + 1));
                        }
                        Err(error) => result.status = Some(error.to_string()),
                    }
                }
                Err(error) => result.status = Some(error.to_string()),
            }
        }
        if ui
            .add_enabled(allow_structural_edits, egui::Button::new("Duplicate loop"))
            .clicked()
        {
            match project.duplicate_section(transport.start_tick, transport.end_tick) {
                Ok(copied) => {
                    result.status = Some(format!(
                        "Duplicated {copied} event(s)/part(s) after the selected section"
                    ));
                }
                Err(error) => result.status = Some(error.to_string()),
            }
        }
        if ui.button("Unique timeline").clicked() {
            roll.select_part(roll.selected_track, None);
            result.edit_notes = true;
        }
        ui.label(
            egui::RichText::new("Click a part to select · Double-click to edit its shared pattern")
                .small()
                .color(theme::MUTED),
        );
    });
    part_inspector(
        ui,
        project,
        roll,
        transport,
        &mut result,
        allow_structural_edits,
    );
    ui.add_space(8.0);

    let ticks = project
        .tracks
        .iter()
        .map(|track| {
            piano_roll::timeline_ticks(track, project.ticks_per_quarter).max(
                track
                    .parts
                    .iter()
                    .map(|part| part.start_tick + part.length_ticks)
                    .max()
                    .unwrap_or(0)
                    + bar * 4,
            )
        })
        .max()
        .unwrap_or(bar * 4);
    let bars = ticks.div_ceil(bar).max(4);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ui.available_width();
            let label_width = 148.0;
            let lane_width = (width - label_width).max(40.0);
            let pixels_per_tick = lane_width / (bars * bar) as f32;
            let (ruler, painter) = ui.allocate_painter(vec2(width, 30.0), egui::Sense::click());
            painter.rect_filled(ruler.rect, 2.0, theme::RAISED);
            let section = Rect::from_min_max(
                Pos2::new(
                    ruler.rect.left() + label_width + transport.start_tick as f32 * pixels_per_tick,
                    ruler.rect.top(),
                ),
                Pos2::new(
                    ruler.rect.left() + label_width + transport.end_tick as f32 * pixels_per_tick,
                    ruler.rect.bottom(),
                ),
            );
            painter.rect_filled(section, 2.0, theme::GREEN.gamma_multiply(0.18));
            draw_playhead(
                &painter,
                ruler.rect,
                label_width,
                playhead_tick,
                pixels_per_tick,
            );
            if let Some(position) = ruler.interact_pointer_pos()
                && ruler.clicked()
            {
                let tick = ((position.x - ruler.rect.left() - label_width) / pixels_per_tick)
                    .max(0.0) as u64;
                let index = (tick / bar).min(bars - 1);
                transport.set_bars(index + 1, index + 2, bar);
            }
            painter.text(
                ruler.rect.left_center() + vec2(12.0, 0.0),
                Align2::LEFT_CENTER,
                "BARS / PARTS",
                FontId::monospace(10.0),
                theme::MUTED,
            );
            let stride = ((bars as f32 * 48.0 / lane_width).ceil() as u64).max(1);
            for index in (0..bars).step_by(stride as usize) {
                let x = ruler.rect.left() + label_width + (index * bar) as f32 * pixels_per_tick;
                painter.text(
                    Pos2::new(x + 6.0, ruler.rect.center().y),
                    Align2::LEFT_CENTER,
                    format!("{:02}", index + 1),
                    FontId::monospace(11.0),
                    theme::IVORY,
                );
            }

            for track_index in 0..project.tracks.len() {
                let color = if project.tracks[track_index].muted {
                    theme::MUTED
                } else {
                    theme::track_color(track_index)
                };
                let (response, painter) =
                    ui.allocate_painter(vec2(width, 94.0), egui::Sense::click());
                let rect = response.rect;
                painter.rect_filled(
                    rect,
                    3.0,
                    if roll.selected_track == track_index {
                        theme::RAISED
                    } else {
                        theme::PANEL
                    },
                );
                painter.rect_filled(
                    Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                    0.0,
                    color,
                );
                let track = project.tracks[track_index].clone();
                painter.text(
                    rect.min + vec2(14.0, 15.0),
                    Align2::LEFT_TOP,
                    format!("{:02}  {}", track_index + 1, track.name),
                    FontId::proportional(14.0),
                    theme::TEXT,
                );
                painter.text(
                    rect.min + vec2(14.0, 39.0),
                    Align2::LEFT_TOP,
                    format!("{} unique notes", track.notes.len()),
                    FontId::monospace(10.0),
                    theme::MUTED,
                );
                painter.text(
                    rect.min + vec2(14.0, 62.0),
                    Align2::LEFT_TOP,
                    if track.muted {
                        "MUTED".into()
                    } else {
                        format!("{} parts", track.parts.len())
                    },
                    FontId::proportional(11.0),
                    color,
                );
                let lane = Rect::from_min_max(rect.min + vec2(label_width, 0.0), rect.max);
                painter.rect_filled(lane, 2.0, theme::BACKGROUND);
                draw_playhead(&painter, lane, 0.0, playhead_tick, pixels_per_tick);
                for index in (0..bars).step_by(stride as usize) {
                    let x = lane.left() + (index * bar) as f32 * pixels_per_tick;
                    painter.line_segment(
                        [Pos2::new(x, lane.top()), Pos2::new(x, lane.bottom())],
                        Stroke::new(1.0, theme::BORDER),
                    );
                }

                for part_index in 0..track.parts.len() {
                    let part = track.parts[part_index];
                    let pattern = &project.patterns[part.pattern];
                    let selected = roll.selected_track == track_index
                        && roll.selected_part == Some(part_index);
                    let part_rect = Rect::from_min_max(
                        lane.min + vec2(part.start_tick as f32 * pixels_per_tick, 8.0),
                        Pos2::new(
                            lane.left()
                                + (part.start_tick + part.length_ticks) as f32 * pixels_per_tick,
                            lane.bottom() - 8.0,
                        ),
                    );
                    let part_id = egui::Id::new(("arrangement-part", track_index, part_index));
                    let part_response =
                        ui.interact(part_rect, part_id, egui::Sense::click_and_drag());
                    painter.rect_filled(
                        part_rect,
                        3.0,
                        color.gamma_multiply(if selected { 0.38 } else { 0.2 }),
                    );
                    painter.rect_stroke(
                        part_rect,
                        3.0,
                        Stroke::new(if selected { 2.0 } else { 1.0 }, color),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        part_rect.left_top() + vec2(6.0, 5.0),
                        Align2::LEFT_TOP,
                        format!("{}  ·  P{:03}", pattern.name, part.pattern + 1),
                        FontId::monospace(10.0),
                        theme::IVORY,
                    );
                    draw_pattern_notes(&painter, part_rect, pattern, part, pixels_per_tick, color);
                    if part_response.clicked() || part_response.double_clicked() {
                        roll.select_part(track_index, Some(part_index));
                    }
                    if part_response.double_clicked() {
                        result.edit_notes = true;
                    }
                    let part_response = part_response.on_hover_text(format!(
                        "{} · {} ticks · drag to move; use the inspector to crop",
                        pattern.name, part.length_ticks
                    ));
                    if allow_structural_edits
                        && part_response.drag_started()
                        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
                    {
                        roll.part_drag = Some(PartDragState {
                            track: track_index,
                            part: part_index,
                            start_tick: part.start_tick,
                            press_x: origin.x,
                        });
                    }
                    if part_response.drag_stopped() {
                        let drag = roll
                            .part_drag
                            .take()
                            .filter(|drag| drag.track == track_index && drag.part == part_index);
                        if allow_structural_edits
                            && let Some(drag) = drag
                            && let Some(position) = ui.input(|input| input.pointer.interact_pos())
                        {
                            let start = dragged_part_start(
                                drag.start_tick,
                                bar,
                                pixels_per_tick,
                                drag.press_x,
                                position.x,
                            );
                            if start != part.start_tick
                                && let Err(error) =
                                    project.move_part(track_index, part_index, start)
                            {
                                result.status = Some(error.to_string());
                            }
                        }
                    }
                }
                if track.parts.is_empty() && track.notes.is_empty() {
                    painter.text(
                        lane.center(),
                        Align2::CENTER_CENTER,
                        "Empty · add a pattern part or edit the unique timeline",
                        FontId::proportional(12.0),
                        theme::MUTED,
                    );
                }
                if response.clicked() || response.double_clicked() {
                    roll.select_part(track_index, None);
                }
                if response.double_clicked() {
                    result.edit_notes = true;
                }
            }
            ui.add_space(16.0);
            theme::label(
                ui,
                format!(
                    "{} TRACKS  /  {} SHARED PATTERNS  /  {} PARTS",
                    project.tracks.len(),
                    project.patterns.len(),
                    project
                        .tracks
                        .iter()
                        .map(|track| track.parts.len())
                        .sum::<usize>()
                ),
            );
        });
    result
}

fn part_inspector(
    ui: &mut egui::Ui,
    project: &mut Project,
    roll: &mut RollState,
    transport: &mut Transport,
    result: &mut SongResult,
    allow_structural_edits: bool,
) {
    let Some(part) = roll.selected_part.and_then(|part| {
        project
            .tracks
            .get(roll.selected_track)
            .and_then(|track| track.parts.get(part))
            .copied()
            .map(|value| (part, value))
    }) else {
        return;
    };
    let (part_index, mut value) = part;
    let pattern_index = value.pattern;
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .inner_margin(8)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                theme::label(ui, "SELECTED PART");
                ui.add(
                    egui::TextEdit::singleline(&mut project.patterns[pattern_index].name)
                        .desired_width(150.0),
                );
                ui.label("Start");
                if ui
                    .add_enabled(
                        allow_structural_edits,
                        egui::DragValue::new(&mut value.start_tick).range(0..=u64::MAX - 1),
                    )
                    .changed()
                    && let Err(error) =
                        project.move_part(roll.selected_track, part_index, value.start_tick)
                {
                    result.status = Some(error.to_string());
                }
                ui.label("Length");
                if ui
                    .add_enabled(
                        allow_structural_edits,
                        egui::DragValue::new(&mut value.length_ticks)
                            .range(1..=u64::MAX - value.start_tick),
                    )
                    .changed()
                    && let Err(error) =
                        project.resize_part(roll.selected_track, part_index, value.length_ticks)
                {
                    result.status = Some(error.to_string());
                }
                if ui.button("Loop part").clicked() {
                    transport.start_tick = value.start_tick;
                    transport.end_tick = value.start_tick + value.length_ticks;
                    transport.repeat = true;
                }
                if ui
                    .add_enabled(allow_structural_edits, egui::Button::new("Duplicate shared"))
                    .clicked()
                {
                    match project.duplicate_part(roll.selected_track, part_index) {
                        Ok(index) => roll.select_part(roll.selected_track, Some(index)),
                        Err(error) => result.status = Some(error.to_string()),
                    }
                }
                if ui
                    .add_enabled(allow_structural_edits, egui::Button::new("Make unique"))
                    .clicked()
                {
                    match project.make_part_unique(roll.selected_track, part_index) {
                        Ok(_) => result.status = Some("Part detached into a unique pattern".into()),
                        Err(error) => result.status = Some(error.to_string()),
                    }
                }
                if ui
                    .add_enabled(allow_structural_edits, egui::Button::new("Merge to timeline"))
                    .clicked()
                {
                    match project.merge_part_into_track(roll.selected_track, part_index) {
                        Ok(()) => {
                            roll.select_part(roll.selected_track, None);
                            result.status = Some("Part merged into the unique timeline".into());
                        }
                        Err(error) => result.status = Some(error.to_string()),
                    }
                }
                if ui
                    .add_enabled(allow_structural_edits, egui::Button::new("Delete part"))
                    .clicked()
                {
                    match project.remove_part(roll.selected_track, part_index) {
                        Ok(_) => roll.select_part(roll.selected_track, None),
                        Err(error) => result.status = Some(error.to_string()),
                    }
                }
            });
            let references = project
                .tracks
                .iter()
                .flat_map(|track| &track.parts)
                .filter(|part| part.pattern == pattern_index)
                .count();
            ui.label(
                egui::RichText::new(format!(
                    "P{:03} is shared by {references} part(s). Make unique before editing if this take should differ.",
                    pattern_index + 1
                ))
                .small()
                .color(theme::MUTED),
            );
        });
}

fn dragged_part_start(
    start_tick: u64,
    bar_ticks: u64,
    pixels_per_tick: f32,
    press_x: f32,
    drop_x: f32,
) -> u64 {
    let delta = (drop_x - press_x) / pixels_per_tick;
    if delta.abs() < 1.0 {
        start_tick
    } else {
        (start_tick as f32 + delta).max(0.0) as u64 / bar_ticks * bar_ticks
    }
}

fn draw_playhead(
    painter: &egui::Painter,
    rect: Rect,
    offset: f32,
    playhead_tick: u64,
    pixels_per_tick: f32,
) {
    let x = rect.left() + offset + playhead_tick as f32 * pixels_per_tick;
    painter.line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(1.5, theme::GREEN),
    );
}

fn draw_pattern_notes(
    painter: &egui::Painter,
    rect: Rect,
    pattern: &Pattern,
    part: Part,
    pixels_per_tick: f32,
    color: Color32,
) {
    if pattern.notes.is_empty() {
        return;
    }
    let low = pattern
        .notes
        .iter()
        .map(|note| note.pitch)
        .min()
        .unwrap_or(48);
    let high = pattern
        .notes
        .iter()
        .map(|note| note.pitch)
        .max()
        .unwrap_or(72);
    for occurrence in 0..part.length_ticks.div_ceil(pattern.length_ticks) {
        for note in &pattern.notes {
            let tick = part.start_tick + occurrence * pattern.length_ticks + note.start_tick;
            if tick >= part.start_tick + part.length_ticks {
                continue;
            }
            let x = rect.left() + (tick - part.start_tick) as f32 * pixels_per_tick;
            let y = rect.bottom()
                - 5.0
                - f32::from(note.pitch - low) / f32::from((high - low).max(1))
                    * (rect.height() - 25.0);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), vec2(2.0, 2.0)),
                0.5,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_drop_uses_the_full_press_to_release_distance() {
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 984.0), 5_760);
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 408.0), 0);
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 600.05), 1_920);
    }
}
