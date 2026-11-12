use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, vec2};
use evil_project::{InstrumentSource, Project};
use evil_synth::{ChannelMeter, INSERT_LIMIT, MixAnalysis, SEND_COUNT};

use crate::{
    effect_ui::{self, EffectType},
    preview::MeterSnapshot,
    theme,
};

#[derive(Default)]
pub struct MixerState {
    pub last_analysis: Option<MixAnalysis>,
    pub last_channel_meters: Vec<(usize, ChannelMeter)>,
}

pub fn mixer(
    ui: &mut egui::Ui,
    project: &mut Project,
    selected: &mut usize,
    state: &mut MixerState,
    meter: MeterSnapshot,
    channel_meters: &[(usize, MeterSnapshot)],
) {
    egui::ScrollArea::vertical()
        .id_salt("mixer-editor")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            mixer_content(ui, project, selected, state, meter, channel_meters);
        });
}

fn mixer_content(
    ui: &mut egui::Ui,
    project: &mut Project,
    selected: &mut usize,
    state: &mut MixerState,
    meter: MeterSnapshot,
    channel_meters: &[(usize, MeterSnapshot)],
) {
    ui.heading("Mixing desk");
    ui.label(
        RichText::new("Channel gain/pan/mute, ordered inserts, project sends and master output")
            .color(theme::MUTED),
    );
    ui.add_space(12.0);
    egui::ScrollArea::horizontal()
        .id_salt("mixer-channels")
        .max_height(560.0)
        .auto_shrink([false, true])
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
                            for bus in 0..SEND_COUNT {
                                ui.add(
                                    egui::Slider::new(&mut instrument.send_gains[bus], 0.0..=1.0)
                                        .text(if bus == 0 { "Send A" } else { "Send B" }),
                                );
                            }
                            ui.separator();
                            fader(ui, &mut instrument.gain);
                            let live = channel_meters
                                .iter()
                                .find(|(track, _)| *track == index)
                                .map(|(_, meter)| *meter);
                            let last = state
                                .last_channel_meters
                                .iter()
                                .find(|(track, _)| *track == index)
                                .map(|(_, meter)| meter);
                            channel_level_meters(ui, live, last);
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
                            if ui
                                .add_sized([112.0, 28.0], egui::Button::new("EDIT INSERTS"))
                                .clicked()
                            {
                                *selected = index;
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
                    ui.separator();
                    fader(ui, &mut project.master_gain);
                    theme::label(ui, "MASTER GAIN");
                    master_meters(ui, meter, state.last_analysis.as_ref());
                });
            });
        });

    if project.tracks.is_empty() {
        return;
    }
    *selected = (*selected).min(project.tracks.len() - 1);
    let track_index = *selected;
    let instrument_index = project.tracks[track_index].instrument;
    let shared_tracks = project
        .tracks
        .iter()
        .filter(|track| track.instrument == instrument_index)
        .count();
    ui.add_space(14.0);
    if ui.available_width() < 760.0 {
        insert_editor(ui, project, track_index, instrument_index, shared_tracks);
        ui.add_space(14.0);
        send_editor(ui, project);
    } else {
        ui.columns(2, |columns| {
            columns[0].set_min_width(360.0);
            columns[1].set_min_width(320.0);
            insert_editor(
                &mut columns[0],
                project,
                track_index,
                instrument_index,
                shared_tracks,
            );
            send_editor(&mut columns[1], project);
        });
    }
}

fn insert_editor(
    ui: &mut egui::Ui,
    project: &mut Project,
    track_index: usize,
    instrument_index: usize,
    shared_tracks: usize,
) {
    theme::section(ui, "CHANNEL INSERTS", theme::PURPLE, |ui| {
        ui.label(
            RichText::new(format!(
                "{} / {}{}",
                project.tracks[track_index].name,
                project.instruments[instrument_index].name,
                if shared_tracks > 1 {
                    format!(" · shared by {shared_tracks} tracks")
                } else {
                    String::new()
                }
            ))
            .color(theme::MUTED),
        );
        let inserts = &mut project.instruments[instrument_index].inserts;
        let mut remove = None;
        let mut swap = None;
        for slot in 0..inserts.len() {
            ui.push_id(("channel-insert", slot), |ui| {
                let title = format!("{} · {}", slot + 1, EffectType::of(&inserts[slot]).label());
                egui::CollapsingHeader::new(title)
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(slot > 0, egui::Button::new("↑ Move earlier"))
                                .clicked()
                            {
                                swap = Some((slot, slot - 1));
                            }
                            if ui
                                .add_enabled(
                                    slot + 1 < inserts.len(),
                                    egui::Button::new("↓ Move later"),
                                )
                                .clicked()
                            {
                                swap = Some((slot, slot + 1));
                            }
                            if ui.button("Remove").clicked() {
                                remove = Some(slot);
                            }
                        });
                        effect_ui::effect_controls(ui, &mut inserts[slot]);
                    });
            });
        }
        if let Some(slot) = remove {
            inserts.remove(slot);
        }
        if let Some((from, to)) = swap {
            inserts.swap(from, to);
        }
        if inserts.len() < INSERT_LIMIT {
            egui::ComboBox::from_id_salt("add-channel-insert")
                .selected_text(format!("+ Add insert ({}/{INSERT_LIMIT})", inserts.len()))
                .show_ui(ui, |ui| {
                    for kind in EffectType::ALL {
                        if ui.selectable_label(false, kind.label()).clicked() {
                            inserts.push(kind.default_effect());
                            ui.close();
                        }
                    }
                });
        } else {
            ui.label(RichText::new("All four insert slots are in use").color(theme::MUTED));
        }
    });
}

fn send_editor(ui: &mut egui::Ui, project: &mut Project) {
    theme::section(ui, "PROJECT SENDS", theme::GREEN, |ui| {
        ui.label(
            RichText::new("Post-channel audio feeds each effect bus; bus gain sets its return.")
                .color(theme::MUTED),
        );
        for (index, bus) in project.send_buses.iter_mut().enumerate() {
            ui.push_id(("project-send", index), |ui| {
                ui.separator();
                theme::label(ui, if index == 0 { "SEND A" } else { "SEND B" });
                effect_ui::optional_effect_selector(ui, "send-effect", &mut bus.effect);
                ui.add(egui::Slider::new(&mut bus.gain, 0.0..=1.0).text("Return gain"));
                if let Some(effect) = &mut bus.effect {
                    effect_ui::effect_controls(ui, effect);
                }
            });
        }
    });
}

fn channel_level_meters(
    ui: &mut egui::Ui,
    live: Option<MeterSnapshot>,
    last: Option<&ChannelMeter>,
) {
    let levels = live.map_or([0.0; 2], |meter| meter.level);
    let peaks = live.map_or_else(
        || last.map_or([0.0; 2], |meter| meter.pre_sum_peak),
        |meter| meter.peak,
    );
    for (channel, level) in levels.into_iter().enumerate() {
        meter_bar(ui, level, peaks[channel]);
    }
    let peak = peaks[0].max(peaks[1]);
    ui.monospace(db_text(peak));
    if last.is_some_and(|meter| meter.clipped_frames > 0) {
        ui.label(RichText::new("CLIP").small().strong().color(theme::RED));
    }
}

fn master_meters(ui: &mut egui::Ui, meter: MeterSnapshot, analysis: Option<&MixAnalysis>) {
    ui.separator();
    theme::label(ui, "OUTPUT METERS");
    for channel in 0..2 {
        meter_bar(ui, meter.level[channel], meter.peak[channel]);
        ui.monospace(format!(
            "{} {}",
            if channel == 0 { "L" } else { "R" },
            db_text(meter.peak[channel])
        ));
    }
    if let Some(analysis) = analysis {
        ui.separator();
        theme::label(ui, "LAST RENDER · PRE-SATURATOR");
        ui.monospace(format!(
            "Peak {} / {}",
            db_text(analysis.pre_saturator_peak[0]),
            db_text(analysis.pre_saturator_peak[1])
        ));
        if analysis.is_clipped() {
            ui.label(
                RichText::new(format!("CLIP · {} hot frames", analysis.clipped_frames))
                    .strong()
                    .color(theme::RED),
            );
        } else {
            ui.label(RichText::new("NO CLIPPING").color(theme::GREEN));
        }
    } else {
        ui.label(
            RichText::new("Play to measure the mix")
                .small()
                .color(theme::MUTED),
        );
    }
}

fn meter_bar(ui: &mut egui::Ui, level: f32, peak: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(112.0, 9.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, theme::BACKGROUND);
    let level_width = meter_fraction(level) * rect.width();
    if level_width > 0.0 {
        let fill = Rect::from_min_size(rect.min, vec2(level_width, rect.height()));
        ui.painter().rect_filled(fill, 2.0, theme::GREEN);
    }
    if peak > 0.0 {
        let x = rect.left() + meter_fraction(peak) * rect.width();
        ui.painter().line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, theme::IVORY),
        );
    }
}

fn meter_fraction(level: f32) -> f32 {
    if !level.is_finite() || level <= 0.000_001 {
        0.0
    } else {
        ((60.0 + 20.0 * level.log10()) / 60.0).clamp(0.0, 1.0)
    }
}

fn db_text(level: f32) -> String {
    if level <= 0.000_001 {
        "-∞ dB".to_owned()
    } else {
        format!("{:+.1} dB", 20.0 * level.log10())
    }
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
    let (area, _) = ui.allocate_exact_size(vec2(112.0, 160.0), egui::Sense::hover());
    let slider_rect = Rect::from_min_size(
        Pos2::new(area.center().x - 14.0, area.top()),
        vec2(28.0, 160.0),
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
        ui.spacing_mut().slider_width = 160.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_scale_handles_silence_headroom_and_clipping() {
        assert_eq!(meter_fraction(0.0), 0.0);
        assert_eq!(meter_fraction(0.001), 0.0);
        assert!((meter_fraction(1.0) - 1.0).abs() < f32::EPSILON);
        assert_eq!(meter_fraction(2.0), 1.0);
        assert_eq!(meter_fraction(f32::NAN), 0.0);
        assert_eq!(db_text(0.0), "-∞ dB");
        assert_eq!(db_text(1.0), "+0.0 dB");
        assert_eq!(db_text(2.0), "+6.0 dB");
    }
}
