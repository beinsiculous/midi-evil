//! The arranger: one focusable editor named "Song" with a cursor over a track and, on it, its
//! unique timeline or one of its parts, as the piano roll is one editor with a cursor over
//! notes. Its accessible name carries what the cursor is on; its description carries the key
//! help. Nothing here changes the arrangement: the buttons, the inspector and the pointer each
//! return a command, and the application runs every command, a key's included, through one
//! boundary that refuses what a take forbids.

pub mod cursor;
mod drawing;
pub mod edits;

use eframe::egui::{self, Pos2, Stroke, accesskit, vec2};
use evil_project::{Part, Project};

use self::{
    cursor::SongCursor,
    drawing::{LABEL_WIDTH, LANE_HEIGHT, Layout},
    edits::SongCommand,
};
use crate::accessibility::{self, Accessible};
use crate::announcer::Announcer;
use crate::regions::{self, Region};
use crate::speech::{self, Meter, Unavailable};
use crate::{theme, transport::Transport};

/// A part being dragged: the part as it was grabbed and where the pointer was pressed.
#[derive(Debug, Clone, Copy)]
struct PartDrag {
    track: usize,
    part: usize,
    grabbed: Part,
    press_x: f32,
}

impl PartDrag {
    /// Whether the grabbed part is still where it was. Parts have no ids, so a key that
    /// removes or moves a part while the button is down leaves the index naming another part,
    /// or the same part somewhere else; the drop must not move either.
    fn still_holds(&self, project: &Project) -> bool {
        project
            .tracks
            .get(self.track)
            .and_then(|track| track.parts.get(self.part))
            == Some(&self.grabbed)
    }
}

/// What the arranger keeps between frames.
#[derive(Debug, Default)]
pub struct SongState {
    /// The editor's widget, once it has been shown: the key dispatch and F6 address it.
    pub widget_id: Option<egui::Id>,
    /// Whether the editor had focus when the last frame ended.
    pub had_focus: bool,
    /// A key moved the cursor; the next frame brings its track into view.
    pub scroll_to_cursor: bool,
    part_drag: Option<PartDrag>,
}

/// What the arranger shows this frame, besides the project.
pub struct SongView<'a> {
    pub cursor: Option<SongCursor>,
    pub transport: &'a Transport,
    pub playhead_tick: u64,
    /// Why the commands that change the arrangement are refused now, if they are; the
    /// boundary that refuses them decides it, and the buttons only show it.
    pub structure_refused: Option<Unavailable>,
    /// The sound of the cursor's track.
    pub sound: &'a str,
}

/// Shows the arranger and returns the commands its buttons, its inspector and the pointer gave,
/// in the order they were given.
pub fn show(
    ui: &mut egui::Ui,
    project: &mut Project,
    state: &mut SongState,
    view: SongView<'_>,
    announcer: &mut Announcer,
) -> Vec<SongCommand> {
    let mut commands = Vec::new();
    let refused = view.structure_refused;
    ui.horizontal_wrapped(|ui| {
        ui.heading("Arranger");
        for (label, command) in [
            ("+ Pattern part", SongCommand::AddPatternPart),
            ("Duplicate loop", SongCommand::DuplicateLoop),
        ] {
            if structural_button(ui, label, refused).clicked() {
                commands.push(command);
            }
        }
        if ui.button("Unique timeline").clicked() {
            commands.push(SongCommand::UniqueTimeline);
        }
        ui.label(
            egui::RichText::new("Click a part to select · Double-click to edit its shared pattern")
                .small()
                .color(theme::MUTED),
        );
    });
    if let Some(cursor) = view.cursor
        && let Some(part) = cursor.part
    {
        part_inspector(ui, project, cursor.track, part, refused, &mut commands);
    }
    ui.add_space(8.0);
    let lanes = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            canvas(ui, project, state, &view, announcer, &mut commands)
        });
    accessibility::name_scroll_bars(ui.ctx(), lanes.id, "Arranger");
    commands
}

fn structural_button(
    ui: &mut egui::Ui,
    label: &str,
    refused: Option<Unavailable>,
) -> egui::Response {
    let response = ui.add_enabled(refused.is_none(), egui::Button::new(label));
    match refused {
        Some(reason) => response.unavailable_because(reason),
        None => response,
    }
}

/// The ruler and the lanes. The lanes are the one focusable editor; the ruler is a pointer's
/// shortcut whose key is the editor's B.
fn canvas(
    ui: &mut egui::Ui,
    project: &mut Project,
    state: &mut SongState,
    view: &SongView<'_>,
    announcer: &mut Announcer,
    commands: &mut Vec<SongCommand>,
) {
    let meter = Meter::of(project);
    let bar = meter.bar_ticks();
    let song_ticks = project
        .tracks
        .iter()
        .map(|track| {
            crate::piano_roll::timeline_ticks(track, project.ticks_per_quarter).max(
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
    let bars = song_ticks.div_ceil(bar).max(4);
    let width = ui.available_width();
    let pixels_per_tick = (width - LABEL_WIDTH).max(40.0) / (bars * bar) as f32;

    let (ruler, ruler_painter) = ui.allocate_painter(vec2(width, 30.0), egui::Sense::CLICK);
    let ruler = ruler.named("Bar ruler, clicked with a pointer to start playback at a bar");
    drawing::draw_ruler(
        &ruler_painter,
        ruler.rect,
        (view.transport.start_tick, view.transport.end_tick),
        bars,
        bar,
        pixels_per_tick,
        view.playhead_tick,
    );
    if ruler.clicked()
        && let Some(position) = ruler.interact_pointer_pos()
    {
        let tick = ((position.x - ruler.rect.left() - LABEL_WIDTH) / pixels_per_tick).max(0.0);
        commands.push(SongCommand::StartAtBar((tick as u64 / bar).min(bars - 1)));
    }

    let size = vec2(width, project.tracks.len() as f32 * LANE_HEIGHT);
    // A fixed id: the part inspector comes and goes above the lanes, and an id counted from
    // what is above would change with it and drop the editor's focus.
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let response = ui.interact(
        rect,
        egui::Id::new("arranger"),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    if response.clicked_by(egui::PointerButton::Primary) || response.drag_started() {
        response.request_focus();
    }
    state.widget_id = Some(response.id);
    crate::custom_editors::register(ui.ctx(), response.id);
    regions::prefer_stop(ui.ctx(), Region::Editor, response.id);
    if response.has_focus() {
        // Without the filter an arrow key moves focus off the editor; Tab stays egui's.
        let arrows = egui::EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, arrows));
    } else if state.had_focus {
        announcer.focus_lost();
    }
    let layout = Layout {
        board: response.rect,
        pixels_per_tick,
    };
    pointer_commands(ui, &response, layout, project, state, bar, commands);
    for track in 0..project.tracks.len() {
        drawing::draw_track(
            &painter,
            project,
            layout,
            track,
            view.cursor,
            (bars, bar),
            view.playhead_tick,
        );
    }
    if let Some(cursor) = view.cursor
        && std::mem::take(&mut state.scroll_to_cursor)
    {
        ui.scroll_to_rect(layout.track_rect(cursor.track), None);
    }
    if response.has_focus() {
        painter.rect_stroke(
            response.rect.expand(2.0),
            0.0,
            Stroke::new(2.0, theme::IVORY),
            egui::StrokeKind::Inside,
        );
    }
    let name = announcer.focused_name(
        response.id,
        speech::song_name(project, view.cursor, view.sound),
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Canvas);
        node.set_label(name);
        node.set_description(speech::song_key_help(speech::platform_command_key()));
    });
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
}

/// The pointer's gestures, as commands: a click selects, a double click opens, a drag moves a
/// part by whole bars.
fn pointer_commands(
    ui: &egui::Ui,
    response: &egui::Response,
    layout: Layout,
    project: &Project,
    state: &mut SongState,
    bar: u64,
    commands: &mut Vec<SongCommand>,
) {
    let hit_at = |position: Option<Pos2>| position.and_then(|at| layout.hit(project, at));
    if response.clicked_by(egui::PointerButton::Primary)
        && let Some(hit) = hit_at(response.interact_pointer_pos())
    {
        commands.push(SongCommand::Select(hit));
        if response.double_clicked() {
            commands.push(SongCommand::Open);
        }
    }
    if response.drag_started()
        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
        && let Some(hit) = hit_at(Some(origin))
        && let Some(part) = hit.part
    {
        // A grabbed part is the selected one, as a grabbed note is in the roll.
        commands.push(SongCommand::Select(hit));
        state.part_drag = Some(PartDrag {
            track: hit.track,
            part,
            grabbed: project.tracks[hit.track].parts[part],
            press_x: origin.x,
        });
    }
    if response.drag_stopped()
        && let Some(drag) = state.part_drag.take()
        && let Some(position) = ui.input(|input| input.pointer.interact_pos())
        && drag.still_holds(project)
    {
        let start = dragged_part_start(
            drag.grabbed.start_tick,
            bar,
            layout.pixels_per_tick,
            drag.press_x,
            position.x,
        );
        if start != drag.grabbed.start_tick {
            commands.push(SongCommand::Select(SongCursor {
                track: drag.track,
                part: Some(drag.part),
            }));
            commands.push(SongCommand::MoveTo(start));
        }
    }
}

fn part_inspector(
    ui: &mut egui::Ui,
    project: &mut Project,
    track: usize,
    part_index: usize,
    refused: Option<Unavailable>,
    commands: &mut Vec<SongCommand>,
) {
    let mut value = project.tracks[track].parts[part_index];
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
                )
                .named("Pattern name");
                ui.label("Start");
                if ui
                    .add_enabled(
                        refused.is_none(),
                        egui::DragValue::new(&mut value.start_tick).range(0..=u64::MAX - 1),
                    )
                    .named("Part start in ticks")
                    .changed()
                {
                    commands.push(SongCommand::MoveTo(value.start_tick));
                }
                ui.label("Length");
                if ui
                    .add_enabled(
                        refused.is_none(),
                        egui::DragValue::new(&mut value.length_ticks)
                            .range(1..=u64::MAX - value.start_tick),
                    )
                    .named("Part length in ticks")
                    .changed()
                {
                    commands.push(SongCommand::Resize(value.length_ticks));
                }
                if ui.button("Loop part").clicked() {
                    commands.push(SongCommand::LoopPart);
                }
                for (label, command) in [
                    ("Duplicate shared", SongCommand::DuplicateShared),
                    ("Make unique", SongCommand::MakeUnique),
                    ("Merge to timeline", SongCommand::MergeToTimeline),
                    ("Delete part", SongCommand::Delete),
                ] {
                    if structural_button(ui, label, refused).clicked() {
                        commands.push(command);
                    }
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "P{:03} is shared by {} part(s). Make unique before editing if this take should differ.",
                    pattern_index + 1,
                    cursor::parts_sharing(project, pattern_index)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_drop_uses_the_full_press_to_release_distance() {
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 984.0), 5_760);
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 408.0), 0);
        assert_eq!(dragged_part_start(1_920, 1_920, 0.1, 600.0, 600.05), 1_920);
    }

    #[test]
    fn a_drag_whose_part_was_removed_or_moved_under_it_no_longer_holds() {
        let mut project = Project::new("Drag");
        project.add_track(evil_project::Track::default()).unwrap();
        let pattern = project
            .add_pattern(evil_project::Pattern {
                length_ticks: 1_920,
                ..evil_project::Pattern::default()
            })
            .unwrap();
        for start_tick in [0, 3_840] {
            let part = Part {
                pattern,
                start_tick,
                length_ticks: 1_920,
            };
            project.add_part(0, part).unwrap();
        }
        let drag = PartDrag {
            track: 0,
            part: 0,
            grabbed: project.tracks[0].parts[0],
            press_x: 0.0,
        };
        assert!(drag.still_holds(&project));
        project.remove_part(0, 0).unwrap();
        assert!(
            !drag.still_holds(&project),
            "the index names the part that was after it"
        );
        project.remove_part(0, 0).unwrap();
        assert!(!drag.still_holds(&project), "no part is left at the index");
    }
}
