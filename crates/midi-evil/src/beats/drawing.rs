use super::KEYS;
use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, pos2, vec2};
use evil_beats::KitPad;

pub(super) fn paint_pad(
    painter: &egui::Painter,
    rect: Rect,
    slot: usize,
    pad: Option<&KitPad>,
    peaks: &[f32],
    selected: bool,
    lit: bool,
) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let color = if pad.is_some_and(|p| p.muted) {
        theme::MUTED
    } else {
        theme::track_color(slot / 4)
    };
    let fill = if lit {
        color.gamma_multiply(0.65)
    } else {
        Color32::from_rgb(color.r() / 5 + 16, color.g() / 5 + 12, color.b() / 5 + 20)
    };
    painter.rect_filled(rect.translate(vec2(0.0, 3.0)), 5.0, theme::BACKGROUND);
    painter.rect_filled(rect, 5.0, fill);
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(
            if selected || lit { 2.0 } else { 1.0 },
            if selected || lit {
                color
            } else {
                theme::BORDER
            },
        ),
        egui::StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(8.0, 7.0), vec2(rect.width() - 16.0, 3.0)),
        1.0,
        color.gamma_multiply(if lit { 1.0 } else { 0.5 }),
    );
    painter.text(
        rect.min + vec2(14.0, 17.0),
        Align2::LEFT_TOP,
        format!("{:02}", slot + 1),
        FontId::monospace(if rect.height() < 120.0 { 19.0 } else { 26.0 }),
        color,
    );
    painter.text(
        rect.right_top() + vec2(-14.0, 19.0),
        Align2::RIGHT_TOP,
        KEYS[slot].1,
        FontId::monospace(13.0),
        theme::TEXT,
    );
    let name = pad.map_or("Empty pad", |p| p.voice.name.as_str());
    let text = painter.layout(
        name.into(),
        FontId::proportional(if rect.width() < 170.0 { 13.0 } else { 16.0 }),
        theme::TEXT,
        rect.width() - 26.0,
    );
    painter.galley(
        pos2(rect.left() + 13.0, rect.bottom() - text.size().y - 18.0),
        text,
        theme::TEXT,
    );
    if rect.height() > 135.0 && pad.is_some() {
        let wave =
            Rect::from_min_size(rect.min + vec2(14.0, 62.0), vec2(rect.width() - 28.0, 22.0));
        paint_waveform(&painter, wave, peaks, color.gamma_multiply(0.5));
    }
    if pad.is_some_and(|p| p.muted) {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "MUTED",
            FontId::monospace(11.0),
            theme::MUTED,
        );
    }
}

pub(super) fn waveform(ui: &mut egui::Ui, samples: &[f32], height: f32, color: Color32) {
    let (response, painter) =
        ui.allocate_painter(vec2(ui.available_width(), height), egui::Sense::hover());
    painter.rect_filled(response.rect, 3.0, theme::BACKGROUND);
    paint_waveform(&painter, response.rect.shrink(6.0), samples, color);
}

fn paint_waveform(painter: &egui::Painter, rect: Rect, samples: &[f32], color: Color32) {
    let bars = rect.width().min(100.0) as usize;
    if bars == 0 || samples.is_empty() {
        return;
    }
    for bar in 0..bars {
        let start = bar * samples.len() / bars;
        let end = ((bar + 1) * samples.len() / bars)
            .max(start + 1)
            .min(samples.len());
        let peak = samples[start..end]
            .iter()
            .fold(0.0_f32, |p, s| p.max(s.abs()))
            .min(1.0);
        let x = rect.left() + bar as f32 / bars as f32 * rect.width();
        let height = (peak * rect.height() * 0.5).max(0.5);
        painter.line_segment(
            [
                pos2(x, rect.center().y - height),
                pos2(x, rect.center().y + height),
            ],
            Stroke::new(1.0, color),
        );
    }
}
