//! What the arranger paints: the bar ruler, each track's lane with its parts, and the playhead.
//! Where things are is computed once, in `Layout`, so the painting and the pointer's hit test
//! agree on every rectangle.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, vec2};
use evil_project::{Part, Pattern, Project};

use super::cursor::SongCursor;
use crate::theme;

pub const LABEL_WIDTH: f32 = 148.0;
pub const LANE_HEIGHT: f32 = 94.0;
const LANE_GAP: f32 = 6.0;

/// Where the lanes and the parts are on the canvas.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub board: Rect,
    pub pixels_per_tick: f32,
}

impl Layout {
    /// The whole row of a track, its label included.
    pub fn track_rect(self, track: usize) -> Rect {
        Rect::from_min_size(
            self.board.min + vec2(0.0, track as f32 * LANE_HEIGHT),
            vec2(self.board.width(), LANE_HEIGHT - LANE_GAP),
        )
    }

    /// The part of a track's row the song's time runs along.
    pub fn lane_rect(self, track: usize) -> Rect {
        let row = self.track_rect(track);
        Rect::from_min_max(row.min + vec2(LABEL_WIDTH, 0.0), row.max)
    }

    pub fn part_rect(self, track: usize, part: Part) -> Rect {
        let lane = self.lane_rect(track);
        Rect::from_min_max(
            lane.min + vec2(part.start_tick as f32 * self.pixels_per_tick, 8.0),
            Pos2::new(
                lane.left() + (part.start_tick + part.length_ticks) as f32 * self.pixels_per_tick,
                lane.bottom() - 8.0,
            ),
        )
    }

    /// The track and, when the point is on one, the part under it. Of overlapping parts, the
    /// one drawn last — on top — is hit.
    pub fn hit(self, project: &Project, position: Pos2) -> Option<SongCursor> {
        if !self.board.contains(position) {
            return None;
        }
        let track = ((position.y - self.board.top()) / LANE_HEIGHT) as usize;
        let row = project.tracks.get(track)?;
        let part = (0..row.parts.len())
            .rev()
            .find(|&index| self.part_rect(track, row.parts[index]).contains(position));
        Some(SongCursor { track, part })
    }
}

pub fn draw_ruler(
    painter: &egui::Painter,
    ruler: Rect,
    section: (u64, u64),
    bars: u64,
    bar_ticks: u64,
    pixels_per_tick: f32,
    playhead_tick: u64,
) {
    painter.rect_filled(ruler, 2.0, theme::RAISED);
    let x = |tick: u64| ruler.left() + LABEL_WIDTH + tick as f32 * pixels_per_tick;
    let looped = Rect::from_min_max(
        Pos2::new(x(section.0), ruler.top()),
        Pos2::new(x(section.1), ruler.bottom()),
    );
    painter.rect_filled(looped, 2.0, theme::GREEN.gamma_multiply(0.18));
    draw_playhead(painter, ruler, LABEL_WIDTH, playhead_tick, pixels_per_tick);
    painter.text(
        ruler.left_center() + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        "BARS / PARTS",
        FontId::monospace(10.0),
        theme::MUTED,
    );
    let lane_width = ruler.width() - LABEL_WIDTH;
    for index in (0..bars).step_by(label_stride(bars, lane_width)) {
        painter.text(
            Pos2::new(x(index * bar_ticks) + 6.0, ruler.center().y),
            Align2::LEFT_CENTER,
            format!("{:02}", index + 1),
            FontId::monospace(11.0),
            theme::IVORY,
        );
    }
}

/// Every how many bars a bar line and number are drawn, so the numbers never overlap.
pub fn label_stride(bars: u64, lane_width: f32) -> usize {
    ((bars as f32 * 48.0 / lane_width.max(1.0)).ceil() as usize).max(1)
}

/// One track's row: its label, its lane, the bar lines and every part on it.
pub fn draw_track(
    painter: &egui::Painter,
    project: &Project,
    layout: Layout,
    track_index: usize,
    cursor: Option<SongCursor>,
    bars: (u64, u64),
    playhead_tick: u64,
) {
    let track = &project.tracks[track_index];
    let color = if track.muted {
        theme::MUTED
    } else {
        theme::track_color(track_index)
    };
    let rect = layout.track_rect(track_index);
    let on_track = cursor.is_some_and(|cursor| cursor.track == track_index);
    painter.rect_filled(
        rect,
        3.0,
        if on_track {
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
    let lane = layout.lane_rect(track_index);
    painter.rect_filled(lane, 2.0, theme::BACKGROUND);
    if on_track && cursor.is_some_and(|cursor| cursor.part.is_none()) {
        // The cursor on the track's own timeline: the lane itself is what it is on.
        painter.rect_stroke(lane, 2.0, Stroke::new(2.0, color), egui::StrokeKind::Inside);
    }
    draw_playhead(painter, lane, 0.0, playhead_tick, layout.pixels_per_tick);
    let (bar_count, bar_ticks) = bars;
    for index in (0..bar_count).step_by(label_stride(bar_count, lane.width())) {
        let x = lane.left() + (index * bar_ticks) as f32 * layout.pixels_per_tick;
        painter.line_segment(
            [Pos2::new(x, lane.top()), Pos2::new(x, lane.bottom())],
            Stroke::new(1.0, theme::BORDER),
        );
    }
    for (part_index, part) in track.parts.iter().enumerate() {
        let pattern = &project.patterns[part.pattern];
        let selected = on_track && cursor.is_some_and(|cursor| cursor.part == Some(part_index));
        let part_rect = layout.part_rect(track_index, *part);
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
        draw_pattern_notes(
            painter,
            part_rect,
            pattern,
            *part,
            layout.pixels_per_tick,
            color,
        );
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
