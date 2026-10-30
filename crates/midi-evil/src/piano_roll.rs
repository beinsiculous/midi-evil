use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::{Note, Project, Track};

use crate::theme;

const KEY_HEIGHT: f32 = 18.0;
const KEYBOARD_WIDTH: f32 = 62.0;
const RULER_HEIGHT: f32 = 26.0;
const MAX_PITCH: u8 = 127;
const DEFAULT_NOTE_VELOCITY: u8 = 100;

#[derive(Debug)]
pub struct RollState {
    pub selected_track: usize,
    pub selected_note: Option<usize>,
    dragging_note: Option<usize>,
    resizing_note: Option<usize>,
    zoom: f32,
    scroll_to_middle: bool,
}

impl Default for RollState {
    fn default() -> Self {
        Self {
            selected_track: 0,
            selected_note: None,
            dragging_note: None,
            resizing_note: None,
            zoom: 1.0,
            scroll_to_middle: true,
        }
    }
}

impl RollState {
    pub fn select_track(&mut self, index: usize) {
        self.selected_track = index;
        self.selected_note = None;
        self.dragging_note = None;
        self.resizing_note = None;
        self.scroll_to_middle = true;
    }

    pub fn clamp(&mut self, project: &Project) {
        self.dragging_note = None;
        self.resizing_note = None;
        if project.tracks.is_empty() {
            self.selected_track = 0;
            self.selected_note = None;
            return;
        }
        self.selected_track = self.selected_track.min(project.tracks.len() - 1);
        let note_count = project.tracks[self.selected_track].notes.len();
        self.selected_note = self.selected_note.filter(|index| *index < note_count);
    }

    pub fn pixels_per_tick(&self, ticks_per_quarter: u16) -> f32 {
        64.0 * self.zoom / f32::from(ticks_per_quarter)
    }
}

pub fn timeline_ticks(track: &Track, ticks_per_quarter: u16) -> u64 {
    let beat = u64::from(ticks_per_quarter.max(1));
    let longest_event = track
        .notes
        .iter()
        .map(|note| note.start_tick + note.duration_ticks)
        .chain(track.pitch_bends.iter().map(|event| event.tick))
        .chain(track.aftertouch.iter().map(|event| event.tick))
        .chain(track.controllers.iter().map(|event| event.tick))
        .max()
        .unwrap_or(0);
    (longest_event.saturating_add(4 * beat)).max(16 * beat)
}

pub fn bar_ticks(project: &Project) -> u64 {
    (u64::from(project.ticks_per_quarter.max(1)) * 4 * u64::from(project.beats_per_bar.max(1))
        / u64::from(project.beat_unit.max(1)))
    .max(1)
}

pub fn show(ui: &mut egui::Ui, project: &mut Project, state: &mut RollState) -> bool {
    if project.tracks.get(state.selected_track).is_none() {
        ui.label("Select a track to edit its notes");
        return false;
    }
    ui.horizontal_wrapped(|ui| {
        theme::label(ui, "GRID  1/16");
        ui.add(egui::Slider::new(&mut state.zoom, 0.25..=3.0).text("Zoom"));
        ui.label(
            egui::RichText::new("Click to draw · Drag to move · Drag edge to resize")
                .size(11.0)
                .color(theme::MUTED),
        );
    });
    ui.add_space(4.0);

    let ticks_per_quarter = project.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let snap = (beat / 4).max(1);
    let bar = bar_ticks(project);
    let timeline_ticks = timeline_ticks(&project.tracks[state.selected_track], ticks_per_quarter);
    let pixels_per_tick = state.pixels_per_tick(ticks_per_quarter);
    let size = Vec2::new(
        KEYBOARD_WIDTH + timeline_ticks as f32 * pixels_per_tick,
        RULER_HEIGHT + (u16::from(MAX_PITCH) + 1) as f32 * KEY_HEIGHT,
    );
    let mut scroll = egui::ScrollArea::both()
        .id_salt("piano-roll")
        .auto_shrink([false, false]);
    if state.scroll_to_middle {
        let notes = &project.tracks[state.selected_track].notes;
        let middle = match (
            notes.iter().map(|n| n.pitch).min(),
            notes.iter().map(|n| n.pitch).max(),
        ) {
            (Some(low), Some(high)) => ((u16::from(low) + u16::from(high)) / 2) as u8,
            _ => 60,
        };
        scroll = scroll.scroll_offset(Vec2::new(
            0.0,
            f32::from(MAX_PITCH - middle.saturating_add(10).min(MAX_PITCH)) * KEY_HEIGHT,
        ));
        state.scroll_to_middle = false;
    }

    scroll
        .show_viewport(ui, |ui, viewport| {
            let (response, full_painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
            let board = response.rect;
            let rect = Rect::from_min_max(
                board.min + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
                board.max,
            );
            let visible_origin = board.min + viewport.min.to_vec2();
            let edit_area = Rect::from_min_max(
                visible_origin + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
                full_painter.clip_rect().max,
            );
            let painter =
                full_painter.with_clip_rect(edit_area.intersect(full_painter.clip_rect()));
            painter.rect_filled(rect, 0.0, theme::BACKGROUND);
            draw_grid(&painter, rect, timeline_ticks, beat, bar, pixels_per_tick);

            for (index, note) in project.tracks[state.selected_track]
                .notes
                .iter()
                .enumerate()
            {
                let note_rect = note_rect(note, rect, pixels_per_tick);
                let selected = state.selected_note == Some(index);
                painter.rect_filled(
                    note_rect,
                    2.0,
                    if selected {
                        theme::IVORY
                    } else {
                        theme::track_color(state.selected_track)
                    },
                );
                painter.rect_stroke(
                    note_rect,
                    2.0,
                    Stroke::new(1.0, theme::TEXT.gamma_multiply(0.35)),
                    egui::StrokeKind::Inside,
                );
                if note_rect.width() >= 36.0 {
                    painter.text(
                        note_rect.left_center() + Vec2::new(5.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        note_name(note.pitch),
                        egui::FontId::monospace(10.0),
                        theme::BACKGROUND,
                    );
                }
            }
            draw_keyboard(&full_painter, rect, visible_origin);
            draw_ruler(
                &full_painter,
                rect,
                visible_origin,
                timeline_ticks,
                bar,
                pixels_per_tick,
            );

            let mut changed = false;
            if response.drag_started()
                && let Some(position) = ui.input(|input| input.pointer.press_origin())
                && edit_area.contains(position)
            {
                if let Some(index) = note_at(
                    &project.tracks[state.selected_track],
                    position,
                    rect,
                    pixels_per_tick,
                ) {
                    state.selected_note = Some(index);
                    let note = &project.tracks[state.selected_track].notes[index];
                    let right_edge = note_rect(note, rect, pixels_per_tick).max.x;
                    if (position.x - right_edge).abs() <= 4.0 {
                        state.resizing_note = Some(index);
                        state.dragging_note = None;
                    } else {
                        state.dragging_note = Some(index);
                        state.resizing_note = None;
                    }
                } else {
                    state.selected_note = None;
                    state.dragging_note = None;
                    state.resizing_note = None;
                }
            }
            if response.dragged()
                && let (Some(index), Some(position)) =
                    (state.dragging_note, response.interact_pointer_pos())
                && let Some(note) = project.tracks[state.selected_track].notes.get_mut(index)
            {
                let (pitch, tick) = position_to_note(position, rect, pixels_per_tick, snap);
                if note.pitch != pitch || note.start_tick != tick {
                    note.pitch = pitch;
                    note.start_tick = tick;
                    changed = true;
                }
            }
            if response.dragged()
                && let (Some(index), Some(position)) =
                    (state.resizing_note, response.interact_pointer_pos())
                && let Some(note) = project.tracks[state.selected_track].notes.get_mut(index)
            {
                let (_, end_tick) = position_to_note(position, rect, pixels_per_tick, snap);
                let duration = end_tick.saturating_sub(note.start_tick).max(snap);
                if note.duration_ticks != duration {
                    note.duration_ticks = duration;
                    changed = true;
                }
            }
            if response.drag_stopped() {
                state.dragging_note = None;
                state.resizing_note = None;
            }
            if response.clicked()
                && let Some(position) = response.interact_pointer_pos()
                && edit_area.contains(position)
            {
                if let Some(index) = note_at(
                    &project.tracks[state.selected_track],
                    position,
                    rect,
                    pixels_per_tick,
                ) {
                    state.selected_note = Some(index);
                } else {
                    let (pitch, start_tick) =
                        position_to_note(position, rect, pixels_per_tick, snap);
                    let index = project.tracks[state.selected_track].notes.len();
                    project.tracks[state.selected_track].notes.push(Note {
                        pitch,
                        start_tick,
                        duration_ticks: beat,
                        velocity: DEFAULT_NOTE_VELOCITY,
                    });
                    state.selected_note = Some(index);
                    changed = true;
                }
            }
            if response.hovered()
                && !ui.ctx().egui_wants_keyboard_input()
                && ui.input(|input| input.key_pressed(egui::Key::Delete))
                && let Some(index) = state.selected_note.take()
                && index < project.tracks[state.selected_track].notes.len()
            {
                project.tracks[state.selected_track].notes.remove(index);
                changed = true;
            }
            changed
        })
        .inner
}

fn draw_grid(
    painter: &egui::Painter,
    rect: Rect,
    timeline_ticks: u64,
    beat: u64,
    bar: u64,
    pixels_per_tick: f32,
) {
    for pitch in 0..=MAX_PITCH {
        let y = rect.top() + f32::from(MAX_PITCH - pitch) * KEY_HEIGHT;
        if is_black_key(pitch) {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left(), y),
                    Pos2::new(rect.right(), y + KEY_HEIGHT),
                ),
                0.0,
                Color32::from_rgb(26, 23, 33),
            );
        }
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, Color32::from_rgb(42, 36, 49)),
        );
    }
    let grid = (beat / 4).max(1);
    let mut tick = (((painter.clip_rect().left() - rect.left()).max(0.0) / pixels_per_tick) as u64
        / grid)
        * grid;
    let last_tick = (((painter.clip_rect().right() - rect.left()).max(0.0) / pixels_per_tick)
        as u64)
        .min(timeline_ticks);
    while tick <= last_tick {
        let x = rect.left() + tick as f32 * pixels_per_tick;
        let (width, color) = if tick.is_multiple_of(bar) {
            (1.5, theme::BORDER)
        } else if tick.is_multiple_of(beat) {
            (1.0, Color32::from_rgb(55, 46, 63))
        } else {
            (0.5, Color32::from_rgb(35, 30, 42))
        };
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(width, color),
        );
        tick += grid;
    }
}

fn draw_keyboard(painter: &egui::Painter, grid: Rect, visible: Pos2) {
    let keyboard = Rect::from_min_max(
        Pos2::new(visible.x, visible.y + RULER_HEIGHT),
        Pos2::new(visible.x + KEYBOARD_WIDTH, painter.clip_rect().bottom()),
    );
    let painter = painter.with_clip_rect(keyboard.intersect(painter.clip_rect()));
    painter.rect_filled(keyboard, 0.0, theme::PANEL);
    for pitch in 0..=MAX_PITCH {
        let y = grid.top() + f32::from(MAX_PITCH - pitch) * KEY_HEIGHT;
        let black = is_black_key(pitch);
        let key = Rect::from_min_size(
            Pos2::new(visible.x, y + 0.5),
            Vec2::new(
                if black { 42.0 } else { KEYBOARD_WIDTH - 2.0 },
                KEY_HEIGHT - 1.0,
            ),
        );
        painter.rect_filled(
            key,
            1.0,
            if black {
                theme::BACKGROUND
            } else {
                Color32::from_rgb(190, 180, 169)
            },
        );
        if pitch % 12 == 0 {
            painter.text(
                key.right_center() - Vec2::new(5.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                octave_label(pitch),
                egui::FontId::monospace(10.0),
                theme::BACKGROUND,
            );
        }
    }
}

fn draw_ruler(
    painter: &egui::Painter,
    grid: Rect,
    visible: Pos2,
    ticks: u64,
    bar: u64,
    pixels: f32,
) {
    let ruler = Rect::from_min_max(
        visible + Vec2::new(KEYBOARD_WIDTH, 0.0),
        Pos2::new(painter.clip_rect().right(), visible.y + RULER_HEIGHT),
    );
    let ruler_painter = painter.with_clip_rect(ruler.intersect(painter.clip_rect()));
    ruler_painter.rect_filled(ruler, 0.0, theme::RAISED);
    let first = (((ruler.left() - grid.left()).max(0.0) / pixels) as u64 / bar) * bar;
    let last = (((ruler.right() - grid.left()).max(0.0) / pixels) as u64).min(ticks);
    for tick in (first..=last).step_by(bar as usize) {
        let x = grid.left() + tick as f32 * pixels;
        ruler_painter.text(
            Pos2::new(x + 6.0, ruler.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{:02}", tick / bar + 1),
            egui::FontId::monospace(11.0),
            theme::IVORY,
        );
    }
    let corner = Rect::from_min_size(visible, Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT));
    painter.rect_filled(corner, 0.0, theme::PANEL);
    painter.text(
        corner.center(),
        egui::Align2::CENTER_CENTER,
        "KEY",
        egui::FontId::monospace(10.0),
        theme::MUTED,
    );
}

fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(pitch % 12)],
        i32::from(pitch) / 12 - 1
    )
}

fn note_rect(note: &Note, rect: Rect, pixels_per_tick: f32) -> Rect {
    let x = rect.left() + note.start_tick as f32 * pixels_per_tick;
    let y = rect.top() + f32::from(MAX_PITCH - note.pitch) * KEY_HEIGHT;
    Rect::from_min_size(
        Pos2::new(x, y + 1.0),
        Vec2::new(
            (note.duration_ticks as f32 * pixels_per_tick).max(3.0),
            KEY_HEIGHT - 2.0,
        ),
    )
}

fn note_at(track: &Track, position: Pos2, rect: Rect, pixels_per_tick: f32) -> Option<usize> {
    if !rect.contains(position) {
        return None;
    }
    let tick = ((position.x - rect.left()) / pixels_per_tick) as u64;
    let pitch = pitch_at(position, rect);
    track.notes.iter().position(|note| {
        note.pitch == pitch
            && tick >= note.start_tick
            && tick < note.start_tick + note.duration_ticks
    })
}

fn position_to_note(position: Pos2, rect: Rect, pixels_per_tick: f32, snap: u64) -> (u8, u64) {
    let raw_tick = ((position.x - rect.left()) / pixels_per_tick).max(0.0) as u64;
    (pitch_at(position, rect), snap_tick(raw_tick, snap))
}

fn pitch_at(position: Pos2, rect: Rect) -> u8 {
    let row = ((position.y - rect.top()) / KEY_HEIGHT).clamp(0.0, 127.0) as u8;
    MAX_PITCH - row
}

pub(crate) fn snap_tick(tick: u64, snap: u64) -> u64 {
    ((tick + snap / 2) / snap) * snap
}

fn octave_label(pitch: u8) -> String {
    format!("C{}", i32::from(pitch) / 12 - 1)
}

fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_lengths_follow_the_imported_time_signature() {
        let mut project = Project::new("Meters");
        assert_eq!(bar_ticks(&project), 1_920);
        project.beats_per_bar = 6;
        project.beat_unit = 8;
        assert_eq!(bar_ticks(&project), 1_440);
        project.ticks_per_quarter = 384;
        project.beats_per_bar = 3;
        project.beat_unit = 4;
        assert_eq!(bar_ticks(&project), 1_152);
    }

    #[test]
    fn sticky_keyboard_and_ruler_do_not_create_notes_but_the_grid_does() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut project = Project::new("Hit testing");
        project.tracks.push(Track::default());
        let mut state = RollState::default();
        let mut time = 0.0;
        let mut frame = |events| {
            time += 0.1;
            let _ = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    show(ui, &mut project, &mut state);
                },
            );
            project.tracks[0].notes.len()
        };
        frame(vec![]);
        frame(vec![]);
        for (position, expected) in [
            (Pos2::new(24.0, 140.0), 0), // fixed keyboard, after initial vertical scroll
            (Pos2::new(180.0, 48.0), 0), // fixed bar ruler
            (Pos2::new(180.0, 140.0), 1),
        ] {
            frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            let count = frame(vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert_eq!(count, expected, "unexpected note creation at {position:?}");
        }
        assert_eq!(project.tracks[0].notes[0].start_tick, 840);
    }

    #[test]
    fn snaps_to_the_nearest_grid_point_without_underflow() {
        assert_eq!(snap_tick(0, 32), 0);
        assert_eq!(snap_tick(15, 32), 0);
        assert_eq!(snap_tick(16, 32), 32);
        assert_eq!(snap_tick(47, 32), 32);
        assert_eq!(snap_tick(48, 32), 64);
    }

    #[test]
    fn octave_labels_cover_the_full_midi_range() {
        for pitch in 0..=MAX_PITCH {
            let _ = octave_label(pitch);
        }
        assert_eq!(octave_label(0), "C-1");
        assert_eq!(octave_label(60), "C4");
        assert_eq!(octave_label(120), "C9");
    }

    #[test]
    fn timeline_extends_to_cover_automation() {
        let track = Track {
            pitch_bends: vec![evil_project::PitchBend {
                tick: 28 * 480,
                value: 0,
            }],
            ..Track::default()
        };
        assert_eq!(timeline_ticks(&track, 480), 32 * 480);
    }

    #[test]
    fn black_keys_match_a_piano_keyboard() {
        assert!(!is_black_key(60));
        assert!(is_black_key(61));
        assert!(!is_black_key(62));
        assert!(is_black_key(63));
    }
}
