use super::*;
use crate::{theme, workspace::View};

impl StudioApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("transport")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::image(egui::Image::new((
                                self.mascot.id(),
                                egui::vec2(48.0, 48.0),
                            )))
                            .frame(false),
                        )
                        .on_hover_text("About MIDI Evil")
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
                                .size(9.0)
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
                        ui.separator();
                        if ui.button("About MIDI Evil").clicked() {
                            ui.close();
                            self.show_about = true;
                        }
                    });
                    if ui
                        .add(egui::Button::image(art::control(&self.controls, 6)))
                        .on_hover_text("Save project · Ctrl/Cmd+S")
                        .clicked()
                    {
                        self.save_project();
                    }
                    ui.separator();
                    let active = self.render_job.is_some() || self.preview.is_some();
                    let label = if self.render_job.is_some() {
                        "CANCEL"
                    } else if active {
                        "STOP"
                    } else {
                        "PLAY"
                    };
                    if ui
                        .add_sized(
                            [88.0, 40.0],
                            egui::Button::image_and_text(
                                art::control(&self.controls, if active { 5 } else { 4 }),
                                label,
                            ),
                        )
                        .on_hover_text("Render and play the song · Space to play/stop")
                        .clicked()
                    {
                        if active {
                            self.stop_preview();
                        } else {
                            self.preview_project();
                        }
                    }
                    if ui
                        .add_sized(
                            [88.0, 40.0],
                            egui::Button::new(if self.recorder.is_some() {
                                "■ TAKE"
                            } else {
                                "● REC"
                            })
                            .selected(self.recorder.is_some()),
                        )
                        .on_hover_text("Record keyboard, pads or MIDI into the selected track/part")
                        .clicked()
                    {
                        self.toggle_recording();
                    }
                    egui::Frame::new()
                        .fill(theme::BACKGROUND)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                        .corner_radius(3)
                        .inner_margin(egui::Margin::symmetric(10, 5))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if self.recorder.is_some() {
                                            "RECORDING"
                                        } else if self.render_job.is_some() {
                                            "RENDERING"
                                        } else if active {
                                            "PLAYING"
                                        } else {
                                            "READY"
                                        })
                                        .size(9.0)
                                        .color(if active { theme::GREEN } else { theme::MUTED }),
                                    );
                                    ui.label(
                                        RichText::new(if self.dirty {
                                            "• MODIFIED"
                                        } else {
                                            "• SAVED"
                                        })
                                        .size(9.0)
                                        .color(theme::MUTED),
                                    );
                                });
                                let width =
                                    (ui.ctx().content_rect().width() - 700.0).clamp(100.0, 280.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut self.project.name)
                                            .desired_width(width)
                                            .font(egui::FontId::monospace(16.0))
                                            .text_color(theme::IVORY)
                                            .frame(egui::Frame::NONE),
                                    )
                                    .changed()
                                {
                                    self.dirty = true;
                                }
                            });
                        });
                    ui.vertical(|ui| {
                        theme::label(ui, "TEMPO");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.project.tempo_bpm)
                                    .range(1.0..=999.0)
                                    .speed(0.1)
                                    .suffix(" BPM"),
                            )
                            .changed()
                        {
                            self.dirty = true;
                        }
                    });
                    ui.vertical(|ui| {
                        theme::label(ui, "METER");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.project.beats_per_bar)
                                    .range(1..=32)
                                    .suffix(format!(" / {}", self.project.beat_unit)),
                            )
                            .changed()
                        {
                            self.dirty = true;
                        }
                    });
                    if ui.available_width() > 150.0 {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                let position = self
                                    .preview
                                    .as_ref()
                                    .map_or(0, |player| player.position().as_millis());
                                ui.label(
                                    RichText::new(format!(
                                        "{:02}:{:02}.{:02}",
                                        position / 60_000,
                                        position / 1_000 % 60,
                                        position / 10 % 100
                                    ))
                                    .monospace()
                                    .size(22.0)
                                    .color(theme::IVORY),
                                );
                                theme::label(
                                    ui,
                                    format!("STEREO  /  {} Hz", self.project.sample_rate),
                                );
                            });
                        });
                    }
                });
            });
        egui::Panel::top("section-transport")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 6)))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let bar_ticks = piano_roll::bar_ticks(&self.project);
                    let mut start = self.transport.start_bar(bar_ticks);
                    let mut end = self.transport.end_bar(bar_ticks);
                    theme::label(ui, "START BAR");
                    let start_changed = ui
                        .add(egui::DragValue::new(&mut start).range(1..=65_535))
                        .on_hover_text("Start playback at the beginning of this bar")
                        .changed();
                    theme::label(ui, "LOOP END");
                    let end_changed = ui
                        .add(egui::DragValue::new(&mut end).range(2..=65_536))
                        .on_hover_text("Repeat stops at the beginning of this bar")
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
                        theme::label(ui, format!("{} EVENTS IN TAKE", recorder.pending_events()));
                    }
                    self.midi_input.show(ui);
                });
            });
        egui::Panel::top("workspace-tabs")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
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
                            .clicked()
                        {
                            if view == View::Synth
                                && self
                                    .patch_lab
                                    .as_ref()
                                    .is_none_or(|lab| lab.track_index != self.roll.selected_track)
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
                            .clicked()
                        {
                            self.redo();
                        }
                        if ui
                            .add_enabled(self.history.can_undo(), egui::Button::new("Undo"))
                            .on_hover_text("Ctrl/Cmd+Z")
                            .clicked()
                        {
                            self.undo();
                        }
                    });
                });
                if self.show_export {
                    ui.horizontal(|ui| {
                        theme::label(ui, "EXPORT SONG");
                        if ui.button("Standard MIDI…").clicked() {
                            self.export_midi();
                            self.show_export = false;
                        }
                        if ui.button("Stereo WAV…").clicked() {
                            self.render_audio();
                            self.show_export = false;
                        }
                    });
                }
            });
    }

    pub(super) fn track_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tracks")
            .default_size(252.0)
            .min_size(225.0)
            .max_size(340.0)
            .resizable(true)
            .frame(theme::panel())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::label(
                        ui,
                        format!("TRACK RACK  /  {:02}", self.project.tracks.len()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+").on_hover_text("Add synth track").clicked() {
                            self.add_track();
                        }
                        if ui
                            .button("+KIT")
                            .on_hover_text("Add a Maxwell drum-kit track")
                            .clicked()
                        {
                            self.add_drum_track();
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("track-rack")
                    .max_height((ui.available_height() * 0.4).max(90.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for index in 0..self.project.tracks.len() {
                            let track = &self.project.tracks[index];
                            let instrument = &self.project.instruments[track.instrument];
                            let source = match &instrument.source {
                                InstrumentSource::Synth { patch } => patch,
                                InstrumentSource::DrumKit { kit, .. } => kit,
                            };
                            let selected = self.roll.selected_track == index;
                            let response =
                                ui.add_sized(
                                    [ui.available_width(), 43.0],
                                    egui::Button::new(
                                        RichText::new(format!(
                                            "{:02}   {}{}",
                                            index + 1,
                                            track.name,
                                            if track.muted { "  [M]" } else { "" }
                                        ))
                                        .color(if selected { theme::IVORY } else { theme::TEXT }),
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
                            if response.clicked() {
                                self.roll.select_track(index);
                            }
                            if response.double_clicked() {
                                self.roll.select_track(index);
                                self.view = View::Notes;
                            }
                            response.on_hover_text(format!(
                                "{} · {} notes",
                                source,
                                track.notes.len()
                            ));
                        }
                    });
                if !self.project.tracks.is_empty() {
                    ui.horizontal(|ui| {
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
                        });
                    });
                }
                ui.add_space(8.0);
                ui.separator();
                let mut open_patch = false;
                egui::ScrollArea::vertical()
                    .id_salt("track-settings")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(track) = self.project.tracks.get_mut(self.roll.selected_track) {
                            let instrument = &mut self.project.instruments[track.instrument];
                            theme::label(ui, "SELECTED TRACK");
                            ui.add(
                                egui::TextEdit::singleline(&mut track.name)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.add_space(4.0);
                            theme::label(ui, "INSTRUMENT");
                            match &mut instrument.source {
                                InstrumentSource::Synth { patch } => {
                                    ui.add(
                                        egui::TextEdit::singleline(patch)
                                            .desired_width(f32::INFINITY),
                                    )
                                    .on_hover_text(
                                        "Synth preset name or a JSON patch path relative to this project",
                                    );
                                }
                                InstrumentSource::DrumKit { kit, .. } => {
                                    ui.add(
                                        egui::TextEdit::singleline(kit)
                                            .desired_width(f32::INFINITY),
                                    )
                                    .on_hover_text(
                                        "Drum-kit path relative to this project, or the maxwell kit",
                                    );
                                }
                            }
                            open_patch = ui
                                .add_sized(
                                    [ui.available_width(), 32.0],
                                    egui::Button::image_and_text(
                                        art::control(&self.controls, 3),
                                        "Open Evil Synth",
                                    ),
                                )
                                .clicked();
                            ui.add_space(8.0);
                            theme::label(ui, "CHANNEL");
                            ui.add(egui::Slider::new(&mut instrument.gain, 0.0..=1.0).text("Gain"));
                            ui.add(egui::Slider::new(&mut instrument.pan, -1.0..=1.0).text("Pan"));
                            ui.checkbox(&mut track.muted, "Mute track");
                            ui.add_space(6.0);
                            ui.collapsing("Performance", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Bend range");
                                    ui.add(
                                        egui::DragValue::new(
                                            &mut instrument.pitch_bend_range_semitones,
                                        )
                                            .range(0.01..=24.0)
                                            .suffix(" st"),
                                    );
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
                if open_patch {
                    self.open_patch_lab();
                }
            });
    }

    pub(super) fn note_inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("note-inspector")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(12, 6)))
            .show(ui, |ui| {
                let Some(note_index) = self.roll.selected_note else {
                    ui.horizontal(|ui| {
                        theme::label(ui, "NOTE INSPECTOR");
                        ui.label(
                            RichText::new("Select a note to edit pitch, velocity and timing")
                                .color(theme::MUTED),
                        );
                    });
                    return;
                };
                let track_index = self.roll.selected_track;
                let part_index = self.roll.selected_part;
                let pattern_index = part_index.and_then(|part| {
                    self.project.tracks[track_index]
                        .parts
                        .get(part)
                        .map(|part| part.pattern)
                });
                let pattern_length =
                    pattern_index.map(|index| self.project.patterns[index].length_ticks);
                let sequence: &mut dyn evil_project::EventSequence = match pattern_index {
                    Some(index) => &mut self.project.patterns[index],
                    None => &mut self.project.tracks[track_index],
                };
                let Some(note) = sequence.notes_mut().get_mut(note_index) else {
                    self.roll.selected_note = None;
                    return;
                };
                let mut remove = false;
                ui.horizontal_wrapped(|ui| {
                    theme::label(ui, "NOTE");
                    ui.label("Pitch");
                    ui.add(egui::DragValue::new(&mut note.pitch).range(0..=127));
                    ui.label("Velocity");
                    ui.add(egui::DragValue::new(&mut note.velocity).range(1..=127));
                    ui.label("Start");
                    ui.add(
                        egui::DragValue::new(&mut note.start_tick)
                            .range(0..=pattern_length.unwrap_or(u64::MAX).saturating_sub(1)),
                    );
                    ui.label("Length");
                    ui.add(egui::DragValue::new(&mut note.duration_ticks).range(1..=1_000_000));
                    remove = ui.button("Delete note").clicked();
                });
                if remove {
                    sequence.notes_mut().remove(note_index);
                    self.roll.selected_note = None;
                }
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
