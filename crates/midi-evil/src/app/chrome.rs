use super::*;
use crate::accessibility::{Accessible, Refusable};
use crate::key_help::{self, AppCommand};
use crate::platform::{self, Shown};
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
                            self.open_about();
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
                        let platform = crate::platform::of(ui.ctx());
                        let shows = |thing| platform::shows(ui.ctx(), thing);
                        let shows_import = shows(Shown::ImportMidi);
                        let shows_recovery = shows(Shown::RecoverRhythmFromWav);
                        let shows_relink = shows(Shown::RelinkMidi);
                        let shows_export = shows(Shown::ExportMidi);
                        let shows_renders = shows(Shown::WavRenders);
                        let project_menu = ui.menu_button("Project", |ui| {
                            let label = |title, gap, command| {
                                key_help::menu_label(title, gap, command, platform)
                            };
                            let new = label("New", "                 ", AppCommand::NewProject);
                            if ui.button(new).clicked() {
                                ui.close();
                                self.new_project();
                            }
                            let open = label("Open…", "             ", AppCommand::OpenProject);
                            if ui.button(open).clicked() {
                                ui.close();
                                self.open_project();
                            }
                            if shows_import && ui.button("Import MIDI…").clicked() {
                                ui.close();
                                self.import_midi();
                            }
                            if shows_recovery && ui.button("Recover rhythm from WAV…").clicked() {
                                ui.close();
                                self.stop_keyboard();
                                self.beats.stop();
                                self.recovery.open = true;
                                self.report(
                                    "The Recover rhythm from WAV window is open; Tab reaches it.",
                                );
                            }
                            ui.separator();
                            let save = label("Save", "                ", AppCommand::SaveProject);
                            if ui.button(save).clicked() {
                                ui.close();
                                self.save_project();
                            }
                            if ui.button("Save as…").clicked() {
                                ui.close();
                                self.save_project_as();
                            }
                            if shows_relink
                                && self.source_midi.is_some()
                                && ui.button("Relink MIDI…").clicked()
                            {
                                ui.close();
                                self.relink_midi();
                            }
                            if shows_export || shows_renders {
                                ui.separator();
                            }
                            if shows_export && ui.button("Export MIDI…").clicked() {
                                ui.close();
                                self.export_midi();
                            }
                            if shows_renders {
                                self.render_menu_items(ui);
                            }
                            ui.separator();
                            if ui.button("Keyboard notes…").clicked() {
                                ui.close();
                                self.show_key_help = true;
                                self.report("The Keyboard notes window is open; Tab reaches it.");
                            }
                            if ui.button("About MIDI Evil").clicked() {
                                ui.close();
                                self.open_about();
                            }
                        });
                        crate::accessibility::name_menu(
                            ui.ctx(),
                            &project_menu.response,
                            "Project menu",
                        );
                        if ui
                            .add(egui::Button::image(art::control(&self.controls, 6)))
                            .on_hover_text(key_help::hover_text(
                                "Save project",
                                AppCommand::SaveProject,
                                egui::Key::S,
                                platform,
                            ))
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
                        if platform::shows(ui.ctx(), Shown::MidiInput) {
                            self.midi_input.show(ui);
                        }
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
                            if platform::shows(ui.ctx(), Shown::View(view))
                                && ui
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
                                        .selected(self.view() == view)
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
                                self.select_view(view);
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // A toggle, so a screen reader hears whether the export row is shown.
                            if platform::shows(ui.ctx(), Shown::ExportRow)
                                && ui
                                    .add(egui::Button::new("Export").selected(self.show_export))
                                    .clicked()
                            {
                                self.show_export = !self.show_export;
                            }
                            let platform = crate::platform::of(ui.ctx());
                            let redo = Refusable::add(
                                ui,
                                (!self.history.can_redo()).then_some(Unavailable::NothingToRedo),
                                egui::Button::new("Redo"),
                            )
                            .on_hover_text(key_help::hover_text(
                                "",
                                AppCommand::Redo,
                                egui::Key::Y,
                                platform,
                            ));
                            if redo.clicked() {
                                self.redo();
                            }
                            self.report_refusal(&redo);
                            let undo = Refusable::add(
                                ui,
                                (!self.history.can_undo()).then_some(Unavailable::NothingToUndo),
                                egui::Button::new("Undo"),
                            )
                            .on_hover_text(key_help::hover_text(
                                "",
                                AppCommand::Undo,
                                egui::Key::Z,
                                platform,
                            ));
                            if undo.clicked() {
                                self.undo();
                            }
                            self.report_refusal(&undo);
                        });
                    });
                    if self.show_export && platform::shows(ui.ctx(), Shown::ExportRow) {
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

    /// The Project menu's renders, which write a WAV file each.
    fn render_menu_items(&mut self, ui: &mut egui::Ui) {
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
    }

    fn open_about(&mut self) {
        self.show_about = true;
        self.report("The About window is open; Tab reaches it.");
    }

    pub(super) fn about_window(&mut self, context: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut start = false;
        let mut open = false;
        let mut import = false;
        egui::Window::new("Dr. Maxwell's studio")
            .id(egui::Id::new("about"))
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
                    import = platform::shows(ui.ctx(), Shown::ImportMidi)
                        && ui
                            .add_sized([280.0, 32.0], egui::Button::new("Import MIDI…"))
                            .clicked();
                    ui.add_space(8.0);
                    theme::label(ui, "MIDI EVIL   /   ORIGINAL SYNTHESIS   /   STEREO SOUND");
                });
            });
        // Anchored, the window draws no title bar to drag; named all the same, should it ever.
        crate::accessibility::name_window_title_bar(context, egui::Id::new("about"), "About");
        if start {
            self.show_about = false;
            self.select_view(View::Notes);
            self.report(format!(
                "The piano roll is open. {} moves between the toolbar, the side panel and the \
                 editor.",
                key_help::next_region_chord(crate::platform::of(context))
            ));
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

    /// The keyboard notes, written from each surface's table of chords. One read-only text, so
    /// a screen reader reads it line by line as it would any text.
    pub(super) fn key_help_window(&mut self, context: &egui::Context) {
        if !self.show_key_help {
            return;
        }
        let notes = key_help::help_text_on(
            crate::speech::platform_command_key(),
            crate::platform::of(context),
            |_| true,
        );
        egui::Window::new("Keyboard notes")
            .id(egui::Id::new("keyboard-notes"))
            .movable(platform::shows(context, Shown::MovingWindows))
            .open(&mut self.show_key_help)
            .default_width(520.0)
            .default_height(420.0)
            .collapsible(false)
            .show(context, |ui| {
                let scroll =
                    egui::ScrollArea::vertical()
                        .id_salt("keyboard-notes")
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut notes.as_str())
                                    .desired_width(f32::INFINITY),
                            )
                            .named("Keyboard notes, one chord a line");
                        });
                crate::accessibility::name_scroll_bars(ui.ctx(), scroll.id, "Keyboard notes");
            });
        crate::accessibility::name_window_title_bar(
            context,
            egui::Id::new("keyboard-notes"),
            "Keyboard notes",
        );
    }

    /// What a command did, said once: on the status line a sighted person reads, and to the
    /// screen reader through the announcer. Every command reports through here.
    pub(super) fn report(&mut self, outcome: impl Into<String>) {
        let text = outcome.into();
        self.announcer.announce(text.clone());
        self.status = text;
    }

    /// Says again why a control that was activated while unavailable did nothing.
    pub(super) fn report_refusal(&mut self, control: &Refusable) {
        if let Some(reason) = control.refusal() {
            self.report(reason);
        }
    }

    /// The same for a command that changes what the focused editor says: the outcome is heard
    /// a frame later, after the name.
    pub(super) fn report_after_the_name(&mut self, outcome: impl Into<String>) {
        let text = outcome.into();
        self.announcer.announce_next_frame(text.clone());
        self.status = text;
    }

    /// The same for a command that moves focus to another widget, perhaps not drawn yet: the
    /// outcome is heard once that widget's name has been.
    pub(super) fn report_after_focus_moves(&mut self, outcome: impl Into<String>) {
        let text = outcome.into();
        self.announcer.announce_after_focus_moves(text.clone());
        self.status = text;
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
