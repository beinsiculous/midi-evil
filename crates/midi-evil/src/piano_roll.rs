use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::{Note, Project, Track};

const KEY_HEIGHT: f32 = 14.0;
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

pub fn show(ui: &mut egui::Ui, project: &mut Project, state: &mut RollState) -> bool {
    if project.tracks.get(state.selected_track).is_none() {
        ui.label("Select a track to edit its notes");
        return false;
    }
    ui.horizontal(|ui| {
        ui.label("Piano roll");
        ui.add(egui::Slider::new(&mut state.zoom, 0.25..=3.0).text("Zoom"));
        ui.label("Click: add/select · Drag: move · Drag edge: resize · Delete: remove");
    });

    let ticks_per_quarter = project.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let snap = (beat / 4).max(1);
    let bar = beat * u64::from(project.beats_per_bar.max(1));
    let timeline_ticks = timeline_ticks(&project.tracks[state.selected_track], ticks_per_quarter);
    let pixels_per_tick = state.pixels_per_tick(ticks_per_quarter);
    let size = Vec2::new(
        timeline_ticks as f32 * pixels_per_tick,
        (u16::from(MAX_PITCH) + 1) as f32 * KEY_HEIGHT,
    );
    let mut scroll = egui::ScrollArea::both().auto_shrink([false, false]);
    if state.scroll_to_middle {
        scroll = scroll.scroll_offset(Vec2::new(0.0, (MAX_PITCH - 60) as f32 * KEY_HEIGHT));
        state.scroll_to_middle = false;
    }

    scroll
        .show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
            let rect = response.rect;
            painter.rect_filled(rect, 0.0, Color32::from_rgb(12, 18, 24));
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
                        Color32::from_rgb(255, 170, 76)
                    } else {
                        Color32::from_rgb(54, 190, 180)
                    },
                );
                painter.rect_stroke(
                    note_rect,
                    2.0,
                    Stroke::new(1.0, Color32::from_rgb(230, 240, 235)),
                    egui::StrokeKind::Inside,
                );
            }

            let mut changed = false;
            if response.drag_started()
                && let Some(position) = ui.input(|input| input.pointer.press_origin())
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
                Color32::from_rgb(18, 27, 35),
            );
        }
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, Color32::from_rgb(38, 51, 60)),
        );
        if pitch % 12 == 0 {
            painter.text(
                Pos2::new(rect.left() + 4.0, y + 1.0),
                egui::Align2::LEFT_TOP,
                octave_label(pitch),
                egui::FontId::monospace(10.0),
                Color32::from_rgb(160, 175, 180),
            );
        }
    }
    let grid = (beat / 4).max(1);
    let mut tick = 0;
    while tick <= timeline_ticks {
        let x = rect.left() + tick as f32 * pixels_per_tick;
        let (width, color) = if tick % bar == 0 {
            (1.5, Color32::from_rgb(90, 115, 125))
        } else if tick % beat == 0 {
            (1.0, Color32::from_rgb(55, 76, 86))
        } else {
            (0.5, Color32::from_rgb(32, 47, 56))
        };
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(width, color),
        );
        tick += grid;
    }
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
