//! The side panel: the track rack and the selected track's settings.

use super::*;
use crate::accessibility::{self, Accessible};
use crate::regions::{self, Region};
use crate::speech::Unavailable;
use crate::workspace::View;

impl StudioApp {
    pub(super) fn track_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tracks")
            .default_size(252.0)
            .min_size(225.0)
            .max_size(340.0)
            .resizable(true)
            .frame(theme::panel())
            .show(ui, |ui| {
                regions::group(ui, Region::SidePanel, |ui| self.track_rack(ui));
            });
        accessibility::name_resize_handle(
            ui.ctx(),
            egui::Id::new("tracks"),
            "Side panel width, dragged with a pointer",
        );
    }

    fn track_rack(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::label(
                ui,
                format!("TRACK RACK  /  {:02}", self.project.tracks.len()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(self.structure_refused().is_none(), egui::Button::new("+"))
                    .on_hover_text("Add synth track")
                    .named("Add a synth track")
                    .unavailable_because(Unavailable::WhileRecording)
                    .clicked()
                {
                    self.add_track();
                }
                if ui
                    .add_enabled(
                        self.structure_refused().is_none(),
                        egui::Button::new("+KIT"),
                    )
                    .on_hover_text("Add a Maxwell drum-kit track")
                    .named("Add a Maxwell drum-kit track")
                    .unavailable_because(Unavailable::WhileRecording)
                    .clicked()
                {
                    self.add_drum_track();
                }
            });
        });
        ui.separator();
        let rack = egui::ScrollArea::vertical()
            .id_salt("track-rack")
            .max_height((ui.available_height() * 0.4).max(90.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for index in 0..self.project.tracks.len() {
                    let track = &self.project.tracks[index];
                    let instrument = &self.project.instruments[track.instrument];
                    let source = crate::instruments::Sound::from_source(
                        &instrument.source,
                        self.patch_base(),
                    )
                    .name;
                    let selected = self.roll.selected_track == index;
                    let response = ui.add_sized(
                        [ui.available_width(), 43.0],
                        egui::Button::new(
                            RichText::new(format!(
                                "{:02}   {}{}",
                                index + 1,
                                track.name,
                                if track.muted { "  [M]" } else { "" }
                            ))
                            .color(if selected {
                                theme::IVORY
                            } else {
                                theme::TEXT
                            }),
                        )
                        .selected(selected)
                        .stroke(egui::Stroke::new(
                            1.0,
                            if selected {
                                theme::track_color(index)
                            } else {
                                theme::BORDER
                            },
                        )),
                    );
                    if selected {
                        regions::prefer_stop(ui.ctx(), Region::SidePanel, response.id);
                    }
                    if response.clicked() {
                        self.select_sound_track(index);
                    }
                    if response.double_clicked() {
                        self.select_sound_track(index);
                        self.view = View::Notes;
                    }
                    response.on_hover_text(format!(
                        "{} · {} notes",
                        source,
                        self.project.tracks[index].notes.len()
                    ));
                }
            });
        accessibility::name_scroll_bars(ui.ctx(), rack.id, "Track rack");
        if !self.project.tracks.is_empty() {
            let actions = ui.horizontal(|ui| {
                ui.add_enabled_ui(self.structure_refused().is_none(), |ui| {
                    ui.menu_button("Track actions", |ui| {
                        if ui.button("Duplicate track").clicked() {
                            ui.close();
                            self.duplicate_selected_track();
                        }
                        if ui.button("Move up").clicked() {
                            ui.close();
                            self.move_selected_track(-1);
                        }
                        if ui.button("Move down").clicked() {
                            ui.close();
                            self.move_selected_track(1);
                        }
                        ui.separator();
                        if ui.button("Delete track").clicked() {
                            ui.close();
                            self.remove_selected_track();
                        }
                    })
                    .response
                    .unavailable_because(Unavailable::WhileRecording)
                })
                .inner
            });
            accessibility::name_menu(ui.ctx(), &actions.inner, "Track actions menu");
        }
        ui.add_space(8.0);
        ui.separator();
        let mut open_patch = false;
        let mut open_pads = false;
        let mut synth_page = None;
        let mut name_field = None;
        let mut mute_toggled = false;
        let mut browse = false;
        let settings = egui::ScrollArea::vertical()
            .id_salt("track-settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(track) = self.project.tracks.get_mut(self.roll.selected_track) {
                    let instrument = &mut self.project.instruments[track.instrument];
                    theme::label(ui, "SELECTED TRACK");
                    name_field = Some(
                        ui.add(
                            egui::TextEdit::singleline(&mut track.name)
                                .desired_width(f32::INFINITY),
                        )
                        .named("Track name"),
                    );
                    ui.add_space(4.0);
                    theme::label(ui, "INSTRUMENT");
                    if ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new("Browse sounds…"),
                        )
                        .clicked()
                    {
                        browse = true;
                    }
                    open_pads = matches!(instrument.source, InstrumentSource::DrumKit { .. });
                    open_patch = ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::image_and_text(
                                art::control(&self.controls, 3),
                                if open_pads {
                                    "Open Pads"
                                } else {
                                    "Open Evil Synth"
                                },
                            ),
                        )
                        .clicked();
                    ui.add_space(6.0);
                    if !open_pads {
                        ui.horizontal_wrapped(|ui| {
                            for (title, page) in [
                                ("Samples", patch_lab::PatchPage::Samples),
                                ("Modulation", patch_lab::PatchPage::Modulation),
                            ] {
                                if ui.button(title).clicked() {
                                    synth_page = Some(page);
                                }
                            }
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Mixer & FX").clicked() {
                            self.view = View::Mixer;
                        }
                        if ui.button("Automation").clicked() {
                            self.view = View::Notes;
                            self.show_automation = true;
                        }
                    });
                    ui.add_space(8.0);
                    theme::label(ui, "CHANNEL");
                    ui.add(egui::Slider::new(&mut instrument.gain, 0.0..=1.0).text("Gain"));
                    ui.add(egui::Slider::new(&mut instrument.pan, -1.0..=1.0).text("Pan"));
                    mute_toggled = ui.checkbox(&mut track.muted, "Mute track").changed();
                    ui.add_space(6.0);
                    ui.collapsing("Performance", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Bend range");
                            ui.add(
                                egui::DragValue::new(&mut instrument.pitch_bend_range_semitones)
                                    .range(0.01..=24.0)
                                    .suffix(" st"),
                            )
                            .named("Bend range in semitones");
                        });
                        ui.label(format!(
                            "{} bends · {} pressure events",
                            track.pitch_bends.len(),
                            track.aftertouch.len()
                        ));
                        ui.label(format!("{} controller events", track.controllers.len()));
                    });
                } else {
                    ui.label(
                        RichText::new("Add a track to start your next experiment.")
                            .color(theme::MUTED),
                    );
                }
            });
        accessibility::name_scroll_bars(ui.ctx(), settings.id, "Selected track settings");
        let selected = self.roll.selected_track;
        if let Some(field) = name_field {
            self.track_name_edited(&field, selected);
        }
        if mute_toggled {
            self.track_mute_toggled(selected);
        }
        if browse {
            self.open_sound_library();
        }
        if let Some(page) = synth_page {
            self.view = View::Synth;
            if self
                .patch_lab
                .as_ref()
                .is_none_or(|lab| lab.track_index != self.roll.selected_track)
            {
                self.open_patch_lab();
            }
            if let Some(lab) = &mut self.patch_lab {
                lab.page = page;
            }
        }
        if open_patch {
            if open_pads {
                self.view = View::Pads;
            } else {
                self.view = View::Synth;
                if self
                    .patch_lab
                    .as_ref()
                    .is_none_or(|lab| lab.track_index != self.roll.selected_track)
                {
                    self.open_patch_lab();
                }
            }
        }
    }
}
