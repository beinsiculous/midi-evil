use super::*;
use crate::instruments::{Collection, LoadedSound, Sound, cycle};

impl StudioApp {
    fn selected_sound(&self) -> Option<Sound> {
        self.project
            .instrument_for_track(self.roll.selected_track)
            .ok()
            .map(|instrument| Sound::from_source(&instrument.source, self.patch_base()))
    }

    pub(super) fn can_replace_sound(&mut self) -> bool {
        if !self.structural_edit_allowed() {
            return false;
        }
        let synth_dirty = self
            .patch_lab
            .as_ref()
            .is_some_and(PatchLab::has_unsaved_changes);
        self.confirm_sound_discard(synth_dirty)
    }

    pub(super) fn confirm_sound_discard(&mut self, synth_dirty: bool) -> bool {
        if !synth_dirty && !self.beats.has_unsaved_changes() {
            return true;
        }
        self.stop_keyboard();
        self.beats.stop();
        rfd::MessageDialog::new()
            .set_title("Unsaved instrument edits")
            .set_description("Replace the current sound draft? Save it in Synth or Pads first to keep your edits.")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show() == rfd::MessageDialogResult::Yes
    }

    pub(super) fn select_sound_track(&mut self, index: usize) {
        if index != self.roll.selected_track && self.structural_edit_allowed() {
            self.stop_keyboard();
            self.beats.stop();
            self.roll.select_track(index);
            self.sync_sound_drafts();
            if let Some(sound) = self.selected_sound() {
                match sound.kind {
                    crate::instruments::SoundKind::Synth if self.view == View::Pads => {
                        self.view = View::Synth
                    }
                    crate::instruments::SoundKind::Kit
                        if matches!(self.view, View::Synth | View::Keyboard) =>
                    {
                        self.view = View::Pads
                    }
                    _ => {}
                }
            }
        }
    }

    pub(super) fn discard_sound_drafts(&mut self) {
        self.stop_keyboard();
        self.patch_lab = None;
        self.beats.discard_draft();
        self.invalidate_beats_link();
    }

    pub(super) fn load_sound_files(&mut self) {
        let Some(current) = self.selected_sound() else {
            return;
        };
        if !self.structural_edit_allowed() {
            return;
        }
        let Some(paths) = self
            .file_dialog()
            .add_filter(current.kind.label(), &[current.kind.extension()])
            .pick_files()
        else {
            return;
        };
        let mut first = None;
        let mut errors = Vec::new();
        let mut count = 0;
        for path in paths {
            let mut sound = Sound::file(&path, current.kind);
            match sound.load() {
                Ok(loaded) => {
                    sound.name = match loaded {
                        LoadedSound::Synth(patch) => patch.name,
                        LoadedSound::Kit(kit) => kit.name,
                    };
                    self.sounds.remember(sound.clone());
                    first.get_or_insert(sound);
                    count += 1;
                }
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
        if let Some(sound) = first {
            self.sounds.query.clear();
            self.sounds.collection = Collection::User;
            if self.assign_sound(sound) {
                self.status = format!(
                    "Loaded {count} user instrument(s); Previous / Next cycles your library"
                );
            }
        }
        if !errors.is_empty() {
            self.status = format!("Loaded {count}; could not load {}", errors.join("; "));
        }
    }

    pub(super) fn load_patch_draft(&mut self) {
        if !self.structural_edit_allowed() {
            return;
        }
        let Some(path) = self
            .file_dialog()
            .add_filter("Synth patch", &["json"])
            .pick_file()
        else {
            return;
        };
        let patch = match evil_render::load_patch_file(&path) {
            Ok(patch) => patch,
            Err(error) => {
                self.status = format!("Could not load patch: {error}");
                return;
            }
        };
        if !self.can_replace_sound() {
            return;
        }
        self.sounds.remember(Sound {
            name: patch.name.clone(),
            ..Sound::file(&path, crate::instruments::SoundKind::Synth)
        });
        self.patch_lab = Some(PatchLab::draft(
            patch,
            path.to_string_lossy().into_owned(),
            self.roll.selected_track,
        ));
        self.status = "Patch opened as a draft; Save & assign updates the song".into();
    }

    fn assign_sound(&mut self, sound: Sound) -> bool {
        if self
            .selected_sound()
            .is_none_or(|current| current.kind != sound.kind)
            || !self.structural_edit_allowed()
        {
            return false;
        }
        // Decode before touching the track or its draft: a missing file is recoverable.
        let loaded = match sound.load() {
            Ok(loaded) => loaded,
            Err(error) => {
                self.status = format!("Could not load {}: {error}", sound.name);
                return false;
            }
        };
        self.assign_loaded_sound(sound, loaded)
    }

    fn assign_loaded_sound(&mut self, sound: Sound, loaded: LoadedSound) -> bool {
        if !self.can_replace_sound() {
            return false;
        }
        self.stop_preview();
        self.discard_sound_drafts();
        let track_index = self.roll.selected_track;
        let reference = sound.path().map_or_else(
            || sound.reference.clone(),
            |path| portable_file_reference(&path, self.patch_base()),
        );
        let instrument = self.project.instrument_for_track_mut(track_index).unwrap();
        match loaded {
            LoadedSound::Synth(patch) => {
                instrument.source = InstrumentSource::Synth {
                    patch: reference.clone(),
                };
                instrument.midi_program = patch.midi_program;
                let file_name = if sound.user {
                    reference.clone()
                } else {
                    format!("{reference}-custom.json")
                };
                self.patch_lab = Some(PatchLab::new(*patch, file_name, track_index));
                self.keyboard_binding = Some((track_index, reference));
            }
            LoadedSound::Kit(kit) => {
                // A different kit does not change the notes that address its pads.
                if let InstrumentSource::DrumKit { kit: source, .. } = &mut instrument.source {
                    *source = reference.clone();
                }
                self.beats.load_instrument(*kit, sound.path(), track_index);
                self.beats_binding = Some((track_index, reference));
            }
        }
        self.status = format!(
            "{} assigned to {}",
            sound.name, self.project.tracks[track_index].name
        );
        self.sounds.remember(sound);
        self.dirty = true;
        true
    }

    pub(super) fn cycle_sound(&mut self, backwards: bool) {
        let Some(current) = self.selected_sound() else {
            return;
        };
        let choices = self
            .sounds
            .choices(&self.project, self.patch_base(), current.kind);
        let mut cursor = current.clone();
        let mut skipped = 0;
        for _ in 0..choices.len() {
            let Some(sound) = cycle(&choices, &cursor, backwards) else {
                break;
            };
            cursor = sound.clone();
            if sound.same_source(&current) {
                continue;
            }
            match sound.load() {
                Ok(loaded) => {
                    if self.assign_loaded_sound(sound.clone(), loaded) && skipped > 0 {
                        self.status
                            .push_str(&format!(" · skipped {skipped} unavailable sound(s)"));
                    }
                    return;
                }
                Err(_) => skipped += 1,
            }
        }
        if skipped > 0 {
            self.status = format!(
                "No other loadable sound; {skipped} unavailable file(s). Load files or remove them from the library."
            );
        }
    }

    pub(super) fn sound_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("sound-bar")
            .frame(
                theme::panel()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    theme::label(ui, "SOUND");
                    let selected_track = self.roll.selected_track;
                    let mut next_track = selected_track;
                    egui::ComboBox::from_id_salt("sound-track")
                        .width(135.0)
                        .selected_text(
                            self.project
                                .tracks
                                .get(selected_track)
                                .map_or("Select track", |track| track.name.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for (index, track) in self.project.tracks.iter().enumerate() {
                                ui.selectable_value(
                                    &mut next_track,
                                    index,
                                    format!("{:02}  {}", index + 1, track.name),
                                );
                            }
                        });
                    if next_track != selected_track {
                        self.select_sound_track(next_track);
                    }
                    ui.menu_button("+ Track", |ui| {
                        ui.add_enabled_ui(self.recorder.is_none(), |ui| {
                            if ui.button("Synth track").clicked() {
                                ui.close();
                                self.add_track();
                            }
                            if ui.button("Drum track").clicked() {
                                ui.close();
                                self.add_drum_track();
                            }
                        });
                    });
                    let Some(current) = self.selected_sound() else {
                        ui.label("Add a synth or drum track to load sounds.");
                        return;
                    };
                    let choices =
                        self.sounds
                            .choices(&self.project, self.patch_base(), current.kind);
                    let can_cycle = self.recorder.is_none()
                        && choices.iter().any(|sound| !sound.same_source(&current));
                    if ui
                        .add_enabled(can_cycle, egui::Button::new("‹ Previous"))
                        .on_hover_text("Previous sound in the filtered library · Alt+Left")
                        .clicked()
                    {
                        self.cycle_sound(true);
                    }
                    let name = choices
                        .iter()
                        .find(|sound| sound.same_source(&current))
                        .map_or(current.name.as_str(), |sound| sound.name.as_str());
                    if ui
                        .add_sized(
                            [180.0, 30.0],
                            egui::Button::new(
                                RichText::new(format!("{name}…")).color(theme::IVORY),
                            )
                            .truncate(),
                        )
                        .on_hover_text(format!(
                            "Browse {} sounds\n{}",
                            current.kind.label(),
                            current.reference
                        ))
                        .clicked()
                    {
                        self.sounds.open = !self.sounds.open;
                    }
                    if ui
                        .add_enabled(can_cycle, egui::Button::new("Next ›"))
                        .on_hover_text("Next sound in the filtered library · Alt+Right")
                        .clicked()
                    {
                        self.cycle_sound(false);
                    }
                    if ui
                        .add_enabled(self.recorder.is_none(), egui::Button::new("Load files…"))
                        .on_hover_text(
                            "Load one or more instruments; remember them in your library",
                        )
                        .clicked()
                    {
                        self.load_sound_files();
                    }
                    theme::label(
                        ui,
                        format!("{} · {} sounds", current.kind.label(), choices.len()),
                    );
                    if self.recorder.is_some() {
                        ui.label("Finish take to change sounds");
                    }
                });
            });
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
                    .hint_text("Search sounds or file paths…").desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    for (collection, title) in [(Collection::All, "All sounds"), (Collection::Factory, "Factory"), (Collection::User, "User loaded")] {
                        ui.selectable_value(&mut self.sounds.collection, collection, title);
                    }
                    load = ui.add_enabled(self.recorder.is_none(), egui::Button::new("Load files…")).clicked();
                });
                ui.separator();
                let choices = self.sounds.choices(&self.project, self.patch_base(), current.kind);
                egui::ScrollArea::vertical().id_salt("sound-list")
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
                                .on_hover_text(&sound.reference).clicked() {
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
                ui.separator();
                ui.label("Select a sound to assign it. Previous / Next follows this search and collection.");
                let instrument_index = self.project.tracks[self.roll.selected_track].instrument;
                let shared = self.project.tracks.iter().filter(|track| track.instrument == instrument_index).count();
                if shared > 1 {
                    ui.colored_label(theme::IVORY, format!("This instrument is shared by {shared} tracks; they change together."));
                }
                ui.small("JSON synth patches · EVILKIT drum banks · User files stay in their original locations.");
            });
        self.sounds.open = open;
        if let Some(sound) = forgotten {
            self.sounds.forget(&sound);
        }
        if load {
            self.load_sound_files();
        }
        if let Some(sound) = selected {
            self.assign_sound(sound);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::SoundKind;

    #[test]
    fn cycling_skips_missing_files_and_wraps_without_changing_track_settings() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        let directory = tempfile::tempdir().unwrap();
        let first = Sound::file(&directory.path().join("first.json"), SoundKind::Synth);
        let missing = Sound::file(&directory.path().join("missing.json"), SoundKind::Synth);
        let last = Sound::file(&directory.path().join("last.json"), SoundKind::Synth);
        for sound in [&first, &last] {
            std::fs::write(
                &sound.reference,
                evil_synth::Patch::default().to_json_pretty().unwrap(),
            )
            .unwrap();
        }
        for sound in [&first, &missing, &last] {
            app.sounds.remember(sound.clone());
        }
        app.sounds.collection = Collection::User;
        let instrument = app.project.instrument_for_track_mut(0).unwrap();
        instrument.gain = 0.42;
        instrument.pan = -0.3;
        assert!(app.assign_sound(first.clone()));
        app.cycle_sound(false);
        assert!(app.selected_sound().unwrap().same_source(&last));
        assert!(app.status.contains("skipped 1"));
        assert_eq!(app.project.instrument_for_track(0).unwrap().gain, 0.42);
        assert_eq!(app.project.instrument_for_track(0).unwrap().pan, -0.3);
        app.cycle_sound(false);
        assert!(app.selected_sound().unwrap().same_source(&first));
        app.sounds.forget(&missing);
        assert!(!app.sounds.is_remembered(&missing));
    }

    #[test]
    fn invalid_sound_keeps_the_assigned_instrument_and_its_unsaved_draft() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.name = "Keep this draft".into();
        let project = app.project.clone();
        let directory = tempfile::tempdir().unwrap();
        assert!(!app.assign_sound(Sound::file(
            &directory.path().join("absent.json"),
            SoundKind::Synth
        )));
        assert_eq!(app.project, project);
        assert_eq!(
            app.patch_lab.as_ref().unwrap().patch.name,
            "Keep this draft"
        );
    }
}
