use super::*;
use crate::accessibility::Accessible;
use crate::regions::{self, Region};
use crate::speech::Unavailable;
use crate::{theme, workspace::View};

impl StudioApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("transport")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 10)))
            .show(ui, |ui| {
                regions::group(ui, Region::Toolbar, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add(
                                egui::Button::image(egui::Image::new((
                                    self.mascot.id(),
                                    egui::vec2(36.0, 36.0),
                                )))
                                .frame(false),
                            )
                            .on_hover_text("About MIDI Evil")
                            .named("About MIDI Evil")
                            .clicked()
                        {
                            self.show_about = true;
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("MIDI EVIL")
                                    .size(20.0)
                                    .strong()
                                    .color(theme::IVORY),
                            );
                            ui.label(
                                RichText::new("DR. MAXWELL'S STUDIO")
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                        });
                        ui.add_space(8.0);
                        ui.menu_button("Project", |ui| {
                            if ui.button("New                 Ctrl+N").clicked() {
                                ui.close();
                                self.new_project();
                            }
                            if ui.button("Open…             Ctrl+O").clicked() {
                                ui.close();
                                self.open_project();
                            }
                            if ui.button("Import MIDI…").clicked() {
                                ui.close();
                                self.import_midi();
                            }
                            if ui.button("Recover rhythm from WAV…").clicked() {
                                ui.close();
                                self.stop_keyboard();
                                self.beats.stop();
                                self.recovery.open = true;
                            }
                            ui.separator();
                            if ui.button("Save                Ctrl+S").clicked() {
                                ui.close();
                                self.save_project();
                            }
                            if ui.button("Save as…").clicked() {
                                ui.close();
                                self.save_project_as();
                            }
                            if self.source_midi.is_some() && ui.button("Relink MIDI…").clicked() {
                                ui.close();
                                self.relink_midi();
                            }
                            ui.separator();
                            if ui.button("Export MIDI…").clicked() {
                                ui.close();
                                self.export_midi();
                            }
                            if ui.button("Render WAV…").clicked() {
                                ui.close();
                                self.render_audio();
                            }
                            if ui.button("Render section WAV…").clicked() {
                                ui.close();
                                self.render_section_audio();
                            }
                            if ui.button("Track stems…").clicked() {
                                ui.close();
                                self.render_track_stems();
                            }
                            if ui.button("Arrangement parts…").clicked() {
                                ui.close();
                                self.render_arrangement_parts();
                            }
                            ui.separator();
                            if ui.button("About MIDI Evil").clicked() {
                                ui.close();
                                self.show_about = true;
                            }
                        });
                        if ui
                            .add(egui::Button::image(art::control(&self.controls, 6)))
                            .on_hover_text("Save project · Ctrl/Cmd+S")
                            .named("Save project")
                            .clicked()
                        {
                            self.save_project();
                        }
                        ui.separator();
                        self.transport_block(ui);
                    });
                });
            });
        egui::Panel::top("section-transport")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 6)))
            .show(ui, |ui| {
                regions::group(ui, Region::Toolbar, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let bar_ticks = piano_roll::bar_ticks(&self.project);
                        let mut start = self.transport.start_bar(bar_ticks);
                        let mut end = self.transport.end_bar(bar_ticks);
                        theme::label(ui, "START BAR");
                        let start_changed = ui
                            .add(egui::DragValue::new(&mut start).range(1..=65_535))
                            .on_hover_text("Start playback at the beginning of this bar")
                            .named("Start bar")
                            .changed();
                        theme::label(ui, "LOOP END");
                        let end_changed = ui
                            .add(egui::DragValue::new(&mut end).range(2..=65_536))
                            .on_hover_text("Repeat stops at the beginning of this bar")
                            .named("Loop end bar")
                            .changed();
                        let repeat_changed = ui
                            .checkbox(&mut self.transport.repeat, "Repeat section")
                            .changed();
                        if start_changed || end_changed || repeat_changed {
                            self.transport.set_bars(start, end, bar_ticks);
                            self.stop_preview();
                        }
                        if self.transport.repeat {
                            theme::label(
                                ui,
                                format!(
                                    "BARS {}–{}",
                                    self.transport.start_bar(bar_ticks),
                                    self.transport.end_bar(bar_ticks) - 1
                                ),
                            );
                        } else {
                            theme::label(ui, "PLAY THROUGH TAIL");
                        }
                        ui.separator();
                        ui.checkbox(&mut self.record_quantize, "Quantize recording to 1/16");
                        if let Some(recorder) = &self.recorder {
                            theme::label(
                                ui,
                                format!("{} EVENTS IN TAKE", recorder.pending_events()),
                            );
                        }
                        self.midi_input.show(ui);
                    });
                });
            });
        egui::Panel::top("workspace-tabs")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 6)))
            .show(ui, |ui| {
                regions::group(ui, Region::Toolbar, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let tab_width = if ui.available_width() < 1100.0 {
                            88.0
                        } else {
                            110.0
                        };
                        for (index, (view, label)) in View::ALL.into_iter().enumerate() {
                            if ui
                                .add_enabled(
                                    !matches!(view, View::Synth | View::Keyboard)
                                        || !self.project.tracks.is_empty(),
                                    egui::Button::image_and_text(
                                        art::control(
                                            &self.controls,
                                            match view {
                                                View::Keyboard => 7,
                                                View::Pads => 8,
                                                _ => index,
                                            },
                                        ),
                                        label,
                                    )
                                    .selected(self.view == view)
                                    .min_size(egui::vec2(tab_width, 30.0)),
                                )
                                .unavailable_because(Unavailable::NoTrack)
                                .clicked()
                            {
                                if view == View::Synth
                                    && self.patch_lab.as_ref().is_none_or(|lab| {
                                        lab.track_index != self.roll.selected_track
                                    })
                                {
                                    self.open_patch_lab();
                                }
                                self.view = view;
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Export").clicked() {
                                self.show_export = !self.show_export;
                            }
                            if ui
                                .add_enabled(self.history.can_redo(), egui::Button::new("Redo"))
                                .on_hover_text("Ctrl/Cmd+Y")
                                .unavailable_because(Unavailable::NothingToRedo)
                                .clicked()
                            {
                                self.redo();
                            }
                            if ui
                                .add_enabled(self.history.can_undo(), egui::Button::new("Undo"))
                                .on_hover_text("Ctrl/Cmd+Z")
                                .unavailable_because(Unavailable::NothingToUndo)
                                .clicked()
                            {
                                self.undo();
                            }
                        });
                    });
                    if self.show_export {
                        ui.horizontal_wrapped(|ui| {
                            theme::label(ui, "EXPORT SONG");
                            if ui.button("Standard MIDI…").clicked() {
                                self.export_midi();
                                self.show_export = false;
                            }
                            if ui.button("Stereo WAV…").clicked() {
                                self.render_audio();
                                self.show_export = false;
                            }
                            if ui.button("Section WAV…").clicked() {
                                self.render_section_audio();
                                self.show_export = false;
                            }
                            if ui.button("Track stems…").clicked() {
                                self.render_track_stems();
                                self.show_export = false;
                            }
                            if ui.button("Arrangement parts…").clicked() {
                                self.render_arrangement_parts();
                                self.show_export = false;
                            }
                        });
                    }
                });
            });
    }

    pub(super) fn about_window(&mut self, context: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut start = false;
        let mut open = false;
        let mut import = false;
        egui::Window::new("Dr. Maxwell's studio")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut self.show_about)
            .show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.image((self.splash.id(), egui::vec2(480.0, 288.0)));
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("A little science. A little sonic mischief.")
                            .size(18.0)
                            .color(theme::IVORY),
                    );
                    ui.label(
                        RichText::new("Compose MIDI, design a sound, make it evil.")
                            .color(theme::MUTED),
                    );
                    ui.add_space(10.0);
                    start = ui
                        .add_sized([280.0, 36.0], egui::Button::new("Start composing"))
                        .clicked();
                    open = ui
                        .add_sized([280.0, 32.0], egui::Button::new("Open project…"))
                        .clicked();
                    import = ui
                        .add_sized([280.0, 32.0], egui::Button::new("Import MIDI…"))
                        .clicked();
                    ui.add_space(8.0);
                    theme::label(ui, "MIDI EVIL   /   ORIGINAL SYNTHESIS   /   STEREO SOUND");
                });
            });
        if start {
            self.show_about = false;
            self.view = View::Notes;
        }
        if open {
            self.show_about = false;
            self.open_project();
        }
        if import {
            self.show_about = false;
            self.import_midi();
        }
    }

    pub(super) fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status")
            .exact_size(29.0)
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(12, 5)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (indicator, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        indicator.center(),
                        3.0,
                        if self.dirty {
                            theme::IVORY
                        } else {
                            theme::GREEN
                        },
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(&self.status).size(11.0).color(theme::MUTED),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&self.status);
                });
            });
    }
}
