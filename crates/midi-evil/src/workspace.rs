use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, vec2};
use evil_project::Project;

use crate::{piano_roll, theme, transport::Transport};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Song,
    Notes,
    Mixer,
    Synth,
    Keyboard,
    Pads,
}

impl View {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::Song, "SONG"),
        (Self::Notes, "PIANO ROLL"),
        (Self::Mixer, "MIXER"),
        (Self::Synth, "SYNTH"),
        (Self::Keyboard, "KEYBOARD"),
        (Self::Pads, "PADS"),
    ];
}

/// A fitted overview of the actual notes, not a separate pattern/clip model.
pub fn song(
    ui: &mut egui::Ui,
    project: &Project,
    selected: &mut usize,
    transport: &mut Transport,
    playhead_tick: u64,
) -> bool {
    ui.horizontal(|ui| {
        ui.heading("Song overview");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme::label(ui, "DOUBLE-CLICK A TRACK TO EDIT NOTES");
        });
    });
    ui.add_space(8.0);
    let bar = piano_roll::bar_ticks(project);
    let ticks = project
        .tracks
        .iter()
        .map(|track| piano_roll::timeline_ticks(track, project.ticks_per_quarter))
        .max()
        .unwrap_or(bar * 4);
    let bars = ticks.div_ceil(bar).max(4);
    let mut edit = false;
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
            let playhead_x =
                ruler.rect.left() + label_width + playhead_tick as f32 * pixels_per_tick;
            painter.line_segment(
                [
                    Pos2::new(playhead_x, ruler.rect.top()),
                    Pos2::new(playhead_x, ruler.rect.bottom()),
                ],
                Stroke::new(1.5, theme::GREEN),
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
                "BARS / BEATS",
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
            for (index, track) in project.tracks.iter().enumerate() {
                let (response, painter) =
                    ui.allocate_painter(vec2(width, 94.0), egui::Sense::click());
                let rect = response.rect;
                let color = if track.muted {
                    theme::MUTED
                } else {
                    theme::track_color(index)
                };
                painter.rect_filled(
                    rect,
                    3.0,
                    if *selected == index {
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
                let label_painter = painter.with_clip_rect(Rect::from_min_size(
                    rect.min,
                    vec2(label_width - 6.0, rect.height()),
                ));
                label_painter.text(
                    rect.min + vec2(14.0, 15.0),
                    Align2::LEFT_TOP,
                    format!("{:02}  {}", index + 1, track.name),
                    FontId::proportional(14.0),
                    theme::TEXT,
                );
                label_painter.text(
                    rect.min + vec2(14.0, 39.0),
                    Align2::LEFT_TOP,
                    &track.patch,
                    FontId::monospace(11.0),
                    color,
                );
                label_painter.text(
                    rect.min + vec2(14.0, 64.0),
                    Align2::LEFT_TOP,
                    if track.muted {
                        "MUTED".to_owned()
                    } else {
                        format!("{} notes", track.notes.len())
                    },
                    FontId::proportional(11.0),
                    theme::MUTED,
                );
                let lane = Rect::from_min_max(rect.min + vec2(label_width, 0.0), rect.max);
                painter.rect_filled(lane, 2.0, theme::BACKGROUND);
                let playhead_x = lane.left() + playhead_tick as f32 * pixels_per_tick;
                painter.line_segment(
                    [
                        Pos2::new(playhead_x, lane.top()),
                        Pos2::new(playhead_x, lane.bottom()),
                    ],
                    Stroke::new(1.0, theme::GREEN.gamma_multiply(0.8)),
                );
                for index in (0..bars).step_by(stride as usize) {
                    let x = lane.left() + (index * bar) as f32 * pixels_per_tick;
                    painter.line_segment(
                        [Pos2::new(x, lane.top()), Pos2::new(x, lane.bottom())],
                        Stroke::new(1.0, theme::BORDER),
                    );
                }
                if let (Some(start), Some(end)) = (
                    track.notes.iter().map(|n| n.start_tick).min(),
                    track
                        .notes
                        .iter()
                        .map(|n| n.start_tick.saturating_add(n.duration_ticks))
                        .max(),
                ) {
                    let region = Rect::from_min_max(
                        lane.min + vec2(start as f32 * pixels_per_tick, 8.0),
                        Pos2::new(
                            lane.left() + end as f32 * pixels_per_tick,
                            lane.bottom() - 8.0,
                        ),
                    );
                    painter.rect_filled(
                        region,
                        3.0,
                        color.gamma_multiply(if track.muted { 0.08 } else { 0.15 }),
                    );
                    painter.rect_stroke(
                        region,
                        3.0,
                        Stroke::new(1.0, color.gamma_multiply(0.45)),
                        egui::StrokeKind::Inside,
                    );
                    let low = track
                        .notes
                        .iter()
                        .map(|n| n.pitch)
                        .min()
                        .unwrap_or(48)
                        .saturating_sub(3);
                    let high = track
                        .notes
                        .iter()
                        .map(|n| n.pitch)
                        .max()
                        .unwrap_or(72)
                        .saturating_add(3);
                    for note in &track.notes {
                        let x = lane.left() + note.start_tick as f32 * pixels_per_tick;
                        let y = region.bottom()
                            - 6.0
                            - f32::from(note.pitch - low) / f32::from((high - low).max(1))
                                * (region.height() - 12.0);
                        let w = (note.duration_ticks as f32 * pixels_per_tick).max(2.0);
                        painter.rect_filled(
                            Rect::from_min_size(Pos2::new(x, y), vec2(w, 2.5)),
                            0.5,
                            color,
                        );
                    }
                } else {
                    painter.text(
                        lane.center(),
                        Align2::CENTER_CENTER,
                        "Empty track · double-click to compose",
                        FontId::proportional(12.0),
                        theme::MUTED,
                    );
                }
                if response.clicked() || response.double_clicked() {
                    *selected = index;
                }
                edit |= response.double_clicked();
                response.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            ui.add_space(16.0);
            theme::label(
                ui,
                format!(
                    "{} TRACKS    /    {} BARS    /    {} TICKS PER QUARTER",
                    project.tracks.len(),
                    bars,
                    project.ticks_per_quarter
                ),
            );
        });
    edit
}

pub fn mixer(ui: &mut egui::Ui, project: &mut Project, selected: &mut usize) {
    ui.heading("Mixing desk");
    ui.label(RichText::new("Track levels, stereo placement and master output").color(theme::MUTED));
    ui.add_space(12.0);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (index, track) in project.tracks.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        channel(ui, theme::track_color(index), |ui| {
                            theme::label(ui, format!("CHANNEL {:02}", index + 1));
                            if ui
                                .add_sized(
                                    [112.0, 30.0],
                                    egui::Button::new(&track.name).selected(*selected == index),
                                )
                                .clicked()
                            {
                                *selected = index;
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&track.patch).small().color(theme::MUTED),
                                )
                                .truncate(),
                            );
                            ui.separator();
                            theme::label(ui, "PAN");
                            ui.add(egui::Slider::new(&mut track.pan, -1.0..=1.0).show_value(false))
                                .on_hover_text("Stereo position: left −1 / center 0 / right +1");
                            ui.monospace(format!("{:+.2}", track.pan));
                            ui.separator();
                            fader(ui, &mut track.gain);
                            let mute = if track.muted { "MUTED" } else { "MUTE" };
                            if ui
                                .add_sized(
                                    [112.0, 30.0],
                                    egui::Button::new(mute).selected(track.muted),
                                )
                                .clicked()
                            {
                                track.muted = !track.muted;
                            }
                        });
                    });
                }
                channel(ui, theme::IVORY, |ui| {
                    theme::label(ui, "OUTPUT");
                    ui.add_sized(
                        [112.0, 30.0],
                        egui::Label::new(RichText::new("MASTER").strong().color(theme::IVORY)),
                    );
                    ui.label(RichText::new("Stereo mix").small().color(theme::MUTED));
                    ui.separator();
                    theme::label(ui, "SAMPLE RATE");
                    egui::ComboBox::from_id_salt("mix-sample-rate")
                        .width(105.0)
                        .selected_text(format!("{} Hz", project.sample_rate))
                        .show_ui(ui, |ui| {
                            for rate in [22_050, 44_100, 48_000, 96_000] {
                                ui.selectable_value(
                                    &mut project.sample_rate,
                                    rate,
                                    format!("{rate} Hz"),
                                );
                            }
                        });
                    ui.monospace("L + R");
                    ui.separator();
                    fader(ui, &mut project.master_gain);
                    theme::label(ui, "MASTER GAIN");
                });
            });
        });
}

fn channel(ui: &mut egui::Ui, accent: Color32, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(4)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(112.0);
                let (rect, _) = ui.allocate_exact_size(vec2(112.0, 3.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, accent);
                ui.vertical_centered(body);
            });
        });
}

fn fader(ui: &mut egui::Ui, gain: &mut f32) {
    theme::label(ui, "LEVEL");
    let (area, _) = ui.allocate_exact_size(vec2(112.0, 200.0), egui::Sense::hover());
    let slider_rect = Rect::from_min_size(
        Pos2::new(area.center().x - 14.0, area.top()),
        vec2(28.0, 200.0),
    );
    ui.painter()
        .rect_filled(slider_rect.expand(2.0), 3.0, theme::BACKGROUND);
    for step in 0..=4 {
        let y = area.bottom() - 14.0 - step as f32 / 4.0 * (area.height() - 28.0);
        ui.painter().line_segment(
            [
                Pos2::new(area.center().x + 19.0, y),
                Pos2::new(area.center().x + 25.0, y),
            ],
            Stroke::new(1.0, theme::BORDER),
        );
        ui.painter().text(
            Pos2::new(area.center().x + 29.0, y),
            Align2::LEFT_CENTER,
            format!("{}", step * 25),
            FontId::monospace(9.0),
            theme::MUTED,
        );
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(slider_rect), |ui| {
        ui.spacing_mut().slider_width = 200.0;
        ui.add(
            egui::Slider::new(gain, 0.0..=1.0)
                .vertical()
                .show_value(false),
        )
        .on_hover_text("Output gain");
    });
    ui.label(
        RichText::new(format!("{:03.0}%", *gain * 100.0))
            .monospace()
            .size(20.0)
            .color(theme::IVORY),
    );
}
