//! The sound library window: search, collections, and one button per sound, which assigns it to
//! the selected track.

use super::*;
use crate::accessibility::Accessible;
use crate::instruments::Collection;
use crate::speech::Unavailable;

impl StudioApp {
    pub(super) fn open_sound_library(&mut self) {
        self.sounds.open = true;
        self.report("The Sound library window is open; Tab reaches it.");
    }

    pub(super) fn sound_browser(&mut self, context: &egui::Context) {
        if !self.sounds.open {
            return;
        }
        let Some(current) = self.selected_sound() else {
            return;
        };
        let mut open = true;
        let mut selected = None;
        let mut forgotten = None;
        let mut load = false;
        egui::Window::new(format!("Sound library / {}", current.kind.label()))
            .id(egui::Id::new("sound-library"))
            .open(&mut open)
            .default_width(540.0)
            .default_height(400.0)
            .default_pos(context.content_rect().center() - egui::vec2(270.0, 250.0))
            .collapsible(false)
            .show(context, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.sounds.query)
                    .hint_text("Search sounds or file paths…").desired_width(f32::INFINITY))
                    .named("Search sounds or file paths");
                ui.horizontal_wrapped(|ui| {
                    for (collection, title) in [(Collection::All, "All sounds"), (Collection::Factory, "Factory"), (Collection::User, "User loaded")] {
                        ui.selectable_value(&mut self.sounds.collection, collection, title);
                    }
                    load = ui.add_enabled(self.recorder.is_none(), egui::Button::new("Load files…"))
                        .unavailable_because(Unavailable::WhileRecording).clicked();
                });
                ui.separator();
                let choices = self.sounds.choices(&self.project, self.patch_base(), current.kind);
                let list = egui::ScrollArea::vertical().id_salt("sound-list")
                    .max_height(320.0).auto_shrink([false, false])
                    .show(ui, |ui| {
                        if choices.is_empty() {
                            ui.label("No matching sounds. Clear the search or load an instrument file.");
                        }
                        for sound in &choices {
                            let origin = if sound.user { "USER" } else { "FACTORY" };
                            let detail = if sound.user { sound.reference.as_str() } else { sound.kind.label() };
                            if ui.add_enabled(self.recorder.is_none(), egui::Button::new(
                                format!("{}   ·   {origin}", sound.name))
                                .selected(sound.same_source(&current))
                                .min_size(egui::vec2(ui.available_width(), 36.0))
                                .truncate())
                                .on_hover_text(&sound.reference)
                                .unavailable_because(Unavailable::WhileRecording).clicked() {
                                selected = Some(sound.clone());
                            }
                            if sound.user {
                                ui.add(egui::Label::new(RichText::new(detail).small().color(theme::MUTED)).truncate()).on_hover_text(detail);
                                if self.sounds.is_remembered(sound)
                                    && ui.small_button("Forget library entry").on_hover_text("Keeps the file and project references; removes it from your remembered library").clicked() {
                                    forgotten = Some(sound.clone());
                                }
                            }
                        }
                    });
                crate::accessibility::name_scroll_bars(ui.ctx(), list.id, "Sound list");
                ui.separator();
                ui.label("Select a sound to assign it. Previous / Next follows this search and collection.");
                let instrument_index = self.project.tracks[self.roll.selected_track].instrument;
                let shared = self.project.tracks.iter().filter(|track| track.instrument == instrument_index).count();
                if shared > 1 {
                    ui.colored_label(theme::IVORY, format!("This instrument is shared by {shared} tracks; they change together."));
                }
                ui.small("JSON synth patches · EVILKIT drum banks · User files stay in their original locations.");
            });
        crate::accessibility::name_window_title_bar(
            context,
            egui::Id::new("sound-library"),
            "Sound library",
        );
        self.sounds.open = open;
        if let Some(sound) = forgotten {
            self.sounds.forget(&sound);
            self.report(format!(
                "Forgot {} from your library; the file is untouched.",
                sound.name
            ));
        }
        if load {
            self.load_sound_files();
        }
        if let Some(sound) = selected
            && self.assign_sound(sound)
        {
            let assigned = self.status.clone();
            match self.describe_selected_sound() {
                Some(place) => self.report(format!("{assigned}; {place}.")),
                None => self.report(assigned),
            }
        }
    }
}
