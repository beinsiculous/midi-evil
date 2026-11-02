use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, vec2};
use evil_project::{InstrumentSource, Project};

use crate::{theme, transport::Transport};

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

pub use crate::arranger::SongResult;

/// Arrange shared patterns and unique performances on the song timeline.
pub fn song(
    ui: &mut egui::Ui,
    project: &mut Project,
    roll: &mut crate::piano_roll::RollState,
    transport: &mut Transport,
    playhead_tick: u64,
) -> SongResult {
    crate::arranger::show(ui, project, roll, transport, playhead_tick)
}

pub fn mixer(ui: &mut egui::Ui, project: &mut Project, selected: &mut usize) {
    ui.heading("Mixing desk");
    ui.label(RichText::new("Track levels, stereo placement and master output").color(theme::MUTED));
    ui.add_space(12.0);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for index in 0..project.tracks.len() {
                    let instrument_index = project.tracks[index].instrument;
                    let track = &mut project.tracks[index];
                    let instrument = &mut project.instruments[instrument_index];
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
                                    RichText::new(instrument_source(&instrument.source))
                                        .small()
                                        .color(theme::MUTED),
                                )
                                .truncate(),
                            );
                            ui.separator();
                            theme::label(ui, "PAN");
                            ui.add(
                                egui::Slider::new(&mut instrument.pan, -1.0..=1.0)
                                    .show_value(false),
                            )
                            .on_hover_text("Stereo position: left −1 / center 0 / right +1");
                            ui.monospace(format!("{:+.2}", instrument.pan));
                            ui.separator();
                            fader(ui, &mut instrument.gain);
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

fn instrument_source(source: &InstrumentSource) -> &str {
    match source {
        InstrumentSource::Synth { patch } => patch,
        InstrumentSource::DrumKit { kit, .. } => kit,
    }
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
