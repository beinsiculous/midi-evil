//! Painting the roll and turning pointer positions into pitches and ticks.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::{EventSequence, Note};

use super::edits::MAX_PITCH;
use crate::theme;

pub const KEY_HEIGHT: f32 = 18.0;
pub const KEYBOARD_WIDTH: f32 = 62.0;
pub const RULER_HEIGHT: f32 = 26.0;

pub fn draw_grid(
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

pub fn draw_notes(
    painter: &egui::Painter,
    notes: &[Note],
    rect: Rect,
    pixels_per_tick: f32,
    cursor: Option<usize>,
    track_index: usize,
) {
    for (index, note) in notes.iter().enumerate() {
        let note_rect = note_rect(note, rect, pixels_per_tick);
        painter.rect_filled(
            note_rect,
            2.0,
            if cursor == Some(index) {
                theme::IVORY
            } else {
                theme::track_color(track_index)
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
}

pub fn draw_keyboard(painter: &egui::Painter, grid: Rect, visible: Pos2) {
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

pub fn draw_ruler(
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

pub fn note_rect(note: &Note, rect: Rect, pixels_per_tick: f32) -> Rect {
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

/// What to bring into view for a note: the note and the strip the fixed ruler and keyboard draw
/// over, above and to the left of it. Scrolling to the note alone can stop with it under them.
pub fn scroll_target(note_rect: Rect) -> Rect {
    Rect::from_min_max(
        note_rect.min - Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
        note_rect.max,
    )
}

pub fn note_at(
    sequence: &(impl EventSequence + ?Sized),
    position: Pos2,
    rect: Rect,
    pixels_per_tick: f32,
) -> Option<usize> {
    if !rect.contains(position) {
        return None;
    }
    let tick = ((position.x - rect.left()) / pixels_per_tick) as u64;
    let pitch = pitch_at(position, rect);
    sequence.notes().iter().position(|note| {
        note.pitch == pitch
            && tick >= note.start_tick
            && tick < note.start_tick + note.duration_ticks
    })
}

pub fn position_to_note(position: Pos2, rect: Rect, pixels_per_tick: f32, snap: u64) -> (u8, u64) {
    let raw_tick = ((position.x - rect.left()) / pixels_per_tick).max(0.0) as u64;
    (pitch_at(position, rect), snap_tick(raw_tick, snap))
}

fn pitch_at(position: Pos2, rect: Rect) -> u8 {
    let row = ((position.y - rect.top()) / KEY_HEIGHT).clamp(0.0, 127.0) as u8;
    MAX_PITCH - row
}

pub fn snap_tick(tick: u64, snap: u64) -> u64 {
    ((tick + snap / 2) / snap) * snap
}

pub fn octave_label(pitch: u8) -> String {
    format!("C{}", i32::from(pitch) / 12 - 1)
}

pub fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}
