use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, TextureHandle};
use evil_project::{Project, Track};

use crate::art;
use crate::automation::{self, AutomationState};
use crate::history::EditorHistory;
use crate::instance::SingleInstance;
use crate::launch::LaunchOptions;
use crate::patch_lab::{self, PatchLab};
use crate::piano_roll::{self, RollState};
use crate::preview::PreviewPlayer;
use crate::render_job::RenderJob;

pub struct StudioApp {
    project: Project,
    project_path: Option<PathBuf>,
    source_midi: Option<PathBuf>,
    project_base: Option<PathBuf>,
    roll: RollState,
    automation: AutomationState,
    dirty: bool,
    status: String,
    mascot: TextureHandle,
    splash: TextureHandle,
    show_about: bool,
    preview: Option<PreviewPlayer>,
    instance: Option<SingleInstance>,
    history: EditorHistory,
    resetting_document: bool,
    restoring_history: bool,
    render_job: Option<RenderJob>,
    pending_render: Option<PendingRender>,
    patch_lab: Option<PatchLab>,
}

enum PendingRender {
    Preview,
    Audition(evil_synth::Patch),
    Wav(PathBuf),
}

enum PatchLabAction {
    New,
    Open,
    Duplicate,
    Reset,
    SaveAs,
    Audition,
}

impl StudioApp {
    pub fn new(
        context: &eframe::CreationContext<'_>,
        options: LaunchOptions,
        instance: Option<SingleInstance>,
    ) -> Self {
        let mut app = Self {
            project: Project::new("Untitled"),
            project_path: None,
            source_midi: None,
            project_base: options.project.clone(),
            roll: RollState::default(),
            automation: AutomationState::default(),
            dirty: false,
            status: "Welcome to MIDI Evil".to_owned(),
            mascot: art::load_mascot(&context.egui_ctx),
            splash: art::load_splash(&context.egui_ctx),
            show_about: true,
            preview: None,
            instance,
            history: EditorHistory::default(),
            resetting_document: false,
            restoring_history: false,
            render_job: None,
            pending_render: None,
            patch_lab: None,
        };
        app.add_track();
        app.dirty = false;
        app.open_launch(options);
        app
    }

    pub fn open_launch(&mut self, options: LaunchOptions) {
        if let Some(project) = options.project {
            self.project_base = Some(project);
        }
        if let Some(path) = options.file {
            let is_midi = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("mid") || extension.eq_ignore_ascii_case("midi")
                });
            if is_midi {
                self.import_midi_path(&path);
            } else {
                self.open_project_path(&path);
            }
        }
        if let Some(track) = options.track {
            self.select_track_by_name(&track);
        }
    }

    fn select_track_by_name(&mut self, name: &str) {
        if let Some(index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.name == name)
        {
            self.roll.select_track(index);
            self.status = format!("Selected track '{name}'");
        } else {
            eprintln!("warning: track '{name}' was not found");
            self.status = format!("Track '{name}' was not found");
        }
    }

    fn reset_history(&mut self) {
        self.history.clear();
        self.resetting_document = true;
    }

    fn undo(&mut self) {
        if let Some(project) = self.history.undo(&self.project) {
            self.project = project;
            self.roll.clamp(&self.project);
            self.restoring_history = true;
            self.dirty = true;
            self.status = "Undo".to_owned();
        }
    }

    fn redo(&mut self) {
        if let Some(project) = self.history.redo(&self.project) {
            self.project = project;
            self.roll.clamp(&self.project);
            self.restoring_history = true;
            self.dirty = true;
            self.status = "Redo".to_owned();
        }
    }

    fn remove_selected_track(&mut self) {
        let index = self.roll.selected_track;
        if self.project.remove_track(index).is_ok() {
            self.automation.clear_selection();
            let new_index = if self.project.tracks.is_empty() {
                0
            } else {
                index.min(self.project.tracks.len() - 1)
            };
            self.roll.select_track(new_index);
            self.dirty = true;
            self.status = format!("Deleted track {index}");
        }
    }

    fn duplicate_selected_track(&mut self) {
        match self.project.duplicate_track(self.roll.selected_track) {
            Ok(index) => {
                self.roll.select_track(index);
                self.dirty = true;
                self.status = format!("Duplicated track {index}");
            }
            Err(error) => self.status = format!("Could not duplicate track: {error}"),
        }
    }

    fn move_selected_track(&mut self, offset: isize) {
        let index = self.roll.selected_track;
        let new_index = index as isize + offset;
        if new_index < 0 || new_index >= self.project.tracks.len() as isize {
            return;
        }
        if self.project.move_track(index, new_index as usize).is_ok() {
            self.roll.select_track(new_index as usize);
            self.dirty = true;
        }
    }

    fn can_discard_changes(&self) -> bool {
        !self.dirty
            || rfd::MessageDialog::new()
                .set_title("Unsaved changes")
                .set_description("Discard unsaved changes?")
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                == rfd::MessageDialogResult::Yes
    }

    fn shortcuts(&mut self, context: &egui::Context) {
        let command =
            |key| context.input(|input| input.modifiers.command && input.key_pressed(key));
        if command(egui::Key::N) {
            self.new_project();
        }
        if command(egui::Key::O) {
            self.open_project();
        }
        if command(egui::Key::S) {
            self.save_project();
        }
        if command(egui::Key::Z) {
            self.undo();
        }
        if command(egui::Key::Y) {
            self.redo();
        }
        if context.input(|input| {
            input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::Z)
        }) {
            self.redo();
        }
        if !context.egui_wants_keyboard_input()
            && context.input(|input| input.key_pressed(egui::Key::Space))
        {
            if self.preview.is_some() {
                self.stop_preview();
            } else {
                self.preview_project();
            }
        }
    }

    fn add_track(&mut self) {
        let number = self.project.tracks.len() + 1;
        self.project.tracks.push(Track {
            name: format!("Track {number}"),
            ..Track::default()
        });
        self.roll.select_track(self.project.tracks.len() - 1);
        self.dirty = true;
        self.status = "Track added".to_owned();
    }

    fn new_project(&mut self) {
        if !self.can_discard_changes() {
            return;
        }
        self.stop_preview();
        self.patch_lab = None;
        self.project = Project::new("Untitled");
        self.show_about = false;
        self.project_path = None;
        self.source_midi = None;
        self.roll = RollState::default();
        self.automation = AutomationState::default();
        self.add_track();
        self.reset_history();
        self.dirty = false;
        self.status = "New project".to_owned();
    }

    fn open_patch_lab(&mut self) {
        let track_index = self.roll.selected_track;
        let track = &self.project.tracks[track_index];
        match evil_render::load_patch(&track.patch, self.patch_base()) {
            Ok(patch) => {
                let file_name = if track.patch.ends_with(".json") {
                    track.patch.clone()
                } else {
                    format!("{}-custom.json", track.patch)
                };
                self.patch_lab = Some(PatchLab::new(patch, file_name, track_index));
            }
            Err(error) => self.status = format!("Could not load synth patch: {error}"),
        }
    }

    fn open_patch_preset(&mut self, lab: &mut PatchLab) {
        let Some(path) = self
            .file_dialog()
            .add_filter("Synth patch", &["json"])
            .pick_file()
        else {
            return;
        };
        match evil_render::load_patch_file(&path) {
            Ok(patch) => {
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "untitled.json".to_owned());
                *lab = PatchLab::new(patch, file_name, lab.track_index);
                self.status = format!("Opened synth patch {}", path.display());
            }
            Err(error) => self.status = format!("Could not open synth patch: {error}"),
        }
    }

    fn save_patch_lab(&mut self, lab: &mut PatchLab) {
        if lab.track_index >= self.project.tracks.len() {
            self.status = "The patch's original track no longer exists".to_owned();
            return;
        }
        let Some(output) = self
            .file_dialog()
            .add_filter("Synth patch", &["json"])
            .set_file_name(lab.file_name.as_str())
            .save_file()
        else {
            return;
        };
        let json = match lab.patch.to_json_pretty() {
            Ok(json) => json,
            Err(error) => {
                self.status = format!("Could not validate patch: {error}");
                return;
            }
        };
        if let Err(error) = evil_project::atomic_write(&output, json.as_bytes()) {
            self.status = format!("Could not save patch: {error}");
            return;
        }
        let patch_base = self.patch_base();
        let reference = output
            .strip_prefix(patch_base)
            .unwrap_or(&output)
            .to_string_lossy()
            .into_owned();
        let track = &mut self.project.tracks[lab.track_index];
        track.patch = reference.clone();
        track.midi_program = lab.patch.midi_program;
        lab.mark_saved(reference);
        self.dirty = true;
        self.status = format!("Saved synth patch to {}", output.display());
    }

    fn audition_patch(&mut self, patch: &evil_synth::Patch) {
        self.start_render(PendingRender::Audition(patch.clone()));
    }

    fn patch_lab_window(&mut self, context: &egui::Context) {
        let Some(mut lab) = self.patch_lab.take() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        egui::Window::new("Evil Synth")
            .open(&mut open)
            .resizable(true)
            .show(context, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("New").clicked() {
                        action = Some(PatchLabAction::New);
                    }
                    if ui.button("Open").clicked() {
                        action = Some(PatchLabAction::Open);
                    }
                    if ui.button("Duplicate").clicked() {
                        action = Some(PatchLabAction::Duplicate);
                    }
                    if ui.button("Reset").clicked() {
                        action = Some(PatchLabAction::Reset);
                    }
                    if ui.button("Save As").clicked() {
                        action = Some(PatchLabAction::SaveAs);
                    }
                    if ui.button("Audition").clicked() {
                        action = Some(PatchLabAction::Audition);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    patch_lab::show(ui, &mut lab.patch);
                });
            });
        match action {
            Some(PatchLabAction::New) => {
                lab = PatchLab::new_preset(lab.track_index);
                self.status = "New synth patch; use Save As to assign it".to_owned();
            }
            Some(PatchLabAction::Open) => self.open_patch_preset(&mut lab),
            Some(PatchLabAction::Duplicate) => {
                lab.duplicate();
                self.status = "Duplicated synth patch; use Save As to assign it".to_owned();
            }
            Some(PatchLabAction::Reset) => {
                lab.reset();
                self.status = "Restored the last opened, duplicated, or saved patch".to_owned();
            }
            Some(PatchLabAction::SaveAs) => self.save_patch_lab(&mut lab),
            Some(PatchLabAction::Audition) => self.audition_patch(&lab.patch),
            None => {}
        }
        if open {
            self.patch_lab = Some(lab);
        }
    }

    fn open_project(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("MIDI Evil Projects", &["evil", "rmp"])
            .pick_file()
        else {
            return;
        };
        self.open_project_path(&path);
    }

    fn open_project_path(&mut self, path: &Path) {
        if !self.can_discard_changes() {
            return;
        }
        self.stop_preview();
        match Project::load(path) {
            Ok(project) => {
                self.patch_lab = None;
                self.project = project;
                self.show_about = false;
                self.source_midi = self.project.source_midi.as_ref().map(PathBuf::from);
                self.roll = RollState::default();
                self.automation = AutomationState::default();
                self.reset_history();
                self.dirty = false;
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.status = format!("Could not open project: {error}"),
        }
    }

    fn save_project(&mut self) {
        let Some(path) = self.project_path.clone() else {
            self.save_project_as();
            return;
        };
        self.save_to(path);
    }

    fn save_project_as(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("MIDI Evil Projects", &["evil", "rmp"])
            .set_file_name("Untitled.evil")
            .save_file()
        else {
            return;
        };
        self.source_midi = None;
        self.project.source_midi = None;
        self.save_to(path);
    }

    fn save_to(&mut self, path: PathBuf) {
        let midi_output = match &self.source_midi {
            Some(midi_path) => {
                match evil_midi::export_native_project(&self.project, self.patch_base()) {
                    Ok(report) => Some((midi_path.clone(), report)),
                    Err(error) => {
                        self.status = format!("Could not export MIDI: {error}");
                        return;
                    }
                }
            }
            None => None,
        };
        self.project.source_midi = self
            .source_midi
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        if let Err(error) = self.project.save(&path) {
            self.status = format!("Could not save project: {error}");
            return;
        }
        if let Some((midi_path, report)) = midi_output {
            if let Err(error) = evil_project::atomic_write(&midi_path, &report.data) {
                self.status = format!("Could not save MIDI {}: {error}", midi_path.display());
                return;
            }
            self.status = format!(
                "Saved {} and exported {} notes to {}",
                path.display(),
                report.note_count,
                midi_path.display()
            );
        } else {
            self.status = format!("Saved {}", path.display());
        }
        self.project_path = Some(path);
        self.dirty = false;
    }

    fn relink_midi(&mut self) {
        if self.source_midi.is_none() {
            self.status = "This project has no linked MIDI file".to_owned();
            return;
        }
        let Some(path) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid", "midi"])
            .pick_file()
        else {
            return;
        };
        self.source_midi = Some(path.clone());
        self.project.source_midi = Some(path.to_string_lossy().into_owned());
        self.dirty = true;
        self.status = format!("Relinked MIDI to {}", path.display());
    }

    fn export_midi(&mut self) {
        let Some(output) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid"])
            .set_file_name("Untitled.mid")
            .save_file()
        else {
            return;
        };
        match evil_midi::export_native_project_to_path(&self.project, self.patch_base(), &output) {
            Ok(report) => {
                self.status = format!(
                    "Exported {} notes from {} tracks to {}",
                    report.note_count,
                    report.track_count,
                    output.display()
                );
            }
            Err(error) => self.status = format!("Could not export MIDI: {error}"),
        }
    }

    fn import_midi(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid", "midi"])
            .pick_file()
        else {
            return;
        };
        self.import_midi_path(&path);
    }

    fn import_midi_path(&mut self, path: &Path) {
        if !self.can_discard_changes() {
            return;
        }
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Imported MIDI")
            .to_owned();
        let result = evil_midi::import_midi_file(path)
            .map_err(|error| error.to_string())
            .and_then(|performance| {
                let skipped = performance.skipped_controller_events();
                let bends = performance.pitch_bend_count();
                let pressure = performance.aftertouch_count();
                let controllers = performance.controller_count();
                Project::from_performance(name, &performance)
                    .map(|project| (project, skipped, bends, pressure, controllers))
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok((project, skipped, bends, pressure, controllers)) => {
                self.stop_preview();
                self.patch_lab = None;
                self.project = project;
                self.show_about = false;
                self.source_midi = Some(path.to_path_buf());
                self.project.source_midi = Some(path.to_string_lossy().into_owned());
                self.project_path = Some(path.with_extension("evil"));
                self.roll = RollState::default();
                self.automation = AutomationState::default();
                self.reset_history();
                self.dirty = true;
                self.status = format!(
                    "Imported {} — {} notes{}{}{}{}",
                    path.display(),
                    self.project
                        .tracks
                        .iter()
                        .map(|track| track.notes.len())
                        .sum::<usize>(),
                    if bends > 0 {
                        format!("; {bends} pitch-bend events preserved")
                    } else {
                        String::new()
                    },
                    if pressure > 0 {
                        format!("; {pressure} aftertouch events preserved")
                    } else {
                        String::new()
                    },
                    if controllers > 0 {
                        format!("; {controllers} controller events preserved")
                    } else {
                        String::new()
                    },
                    if skipped > 0 {
                        format!("; {skipped} controller/automation events not rendered")
                    } else {
                        String::new()
                    }
                );
            }
            Err(error) => self.status = format!("Could not import MIDI: {error}"),
        }
    }

    fn preview_project(&mut self) {
        self.start_render(PendingRender::Preview);
    }

    fn start_render(&mut self, action: PendingRender) {
        if self.render_job.is_some() {
            self.status = "Already rendering; cancel it before starting another".to_owned();
            return;
        }
        self.stop_preview();
        let job = match &action {
            PendingRender::Audition(patch) => RenderJob::start_audition(patch.clone()),
            PendingRender::Preview | PendingRender::Wav(_) => {
                RenderJob::start(self.project.clone(), self.patch_base().to_path_buf())
            }
        };
        self.render_job = Some(job);
        self.status = match &action {
            PendingRender::Audition(patch) => format!("Auditioning '{}'…", patch.name),
            PendingRender::Preview | PendingRender::Wav(_) => {
                "Rendering in the background…".to_owned()
            }
        };
        self.pending_render = Some(action);
    }

    fn poll_render(&mut self) {
        let Some(result) = self.render_job.as_ref().and_then(RenderJob::try_finish) else {
            return;
        };
        self.render_job = None;
        let action = self.pending_render.take();
        match (action, result) {
            (Some(PendingRender::Preview), Ok(rendered)) => {
                let status = automation::render_status(
                    format!("Previewing {} notes", rendered.note_count),
                    rendered.unsupported_controller_events,
                );
                self.play_render(rendered, status);
            }
            (Some(PendingRender::Audition(patch)), Ok(rendered)) => {
                self.play_render(rendered, format!("Auditioning '{}'", patch.name));
            }
            (Some(PendingRender::Wav(output)), Ok(rendered)) => {
                match rendered.sound.write_wav(&output) {
                    Ok(()) => {
                        self.status = automation::render_status(
                            format!(
                                "Rendered {} notes from {} tracks to {}",
                                rendered.note_count,
                                rendered.track_count,
                                output.display()
                            ),
                            rendered.unsupported_controller_events,
                        );
                    }
                    Err(error) => self.status = format!("Could not write audio: {error}"),
                }
            }
            (_, Err(error)) => self.status = format!("Could not render audio: {error}"),
            _ => {}
        }
    }

    fn play_render(&mut self, rendered: evil_render::RenderOutput, status: String) {
        match PreviewPlayer::play(rendered.sound) {
            Ok(player) => {
                self.preview = Some(player);
                self.status = status;
            }
            Err(error) => self.status = format!("Could not start audio preview: {error}"),
        }
    }

    fn stop_preview(&mut self) {
        self.render_job = None;
        self.pending_render = None;
        if let Some(preview) = self.preview.take() {
            preview.stop();
        }
    }

    fn patch_base(&self) -> &Path {
        self.project_path
            .as_deref()
            .and_then(Path::parent)
            .filter(|parent| !parent.as_os_str().is_empty())
            .or(self.project_base.as_deref())
            .unwrap_or_else(|| Path::new("."))
    }

    fn file_dialog(&self) -> rfd::FileDialog {
        let mut dialog = rfd::FileDialog::new();
        if let Some(project) = &self.project_base {
            dialog = dialog.set_directory(project);
        }
        dialog
    }

    fn render_audio(&mut self) {
        let Some(output) = self
            .file_dialog()
            .add_filter("WAV audio", &["wav"])
            .set_file_name("Untitled.wav")
            .save_file()
        else {
            return;
        };
        self.start_render(PendingRender::Wav(output));
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.image((self.mascot.id(), egui::vec2(28.0, 28.0)));
                ui.label(RichText::new("MIDI Evil").strong().size(20.0));
                ui.separator();
                if ui.button("New").clicked() {
                    self.new_project();
                }
                if ui
                    .add_enabled(self.history.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.history.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if ui.button("Open").clicked() {
                    self.open_project();
                }
                if ui.button("Import MIDI").clicked() {
                    self.import_midi();
                }
                if ui.button("Save").clicked() {
                    self.save_project();
                }
                if ui.button("Save as").clicked() {
                    self.save_project_as();
                }
                if self.source_midi.is_some() && ui.button("Relink MIDI").clicked() {
                    self.relink_midi();
                }
                if ui.button("Export MIDI").clicked() {
                    self.export_midi();
                }
                if ui.button("Render WAV").clicked() {
                    self.render_audio();
                }
                let preview_label = if self.render_job.is_some() {
                    "Cancel"
                } else if self.preview.is_some() {
                    "Stop"
                } else {
                    "Preview"
                };
                if ui.button(preview_label).clicked() {
                    if self.render_job.is_some() || self.preview.is_some() {
                        self.stop_preview();
                    } else {
                        self.preview_project();
                    }
                }
                if ui.button("About").clicked() {
                    self.show_about = true;
                }
                ui.separator();
                ui.label("Name");
                if ui.text_edit_singleline(&mut self.project.name).changed() {
                    self.dirty = true;
                }
                ui.label("BPM");
                if ui
                    .add(egui::DragValue::new(&mut self.project.tempo_bpm).range(1.0..=999.0))
                    .changed()
                {
                    self.dirty = true;
                }
                ui.label("Beats/bar");
                if ui
                    .add(egui::DragValue::new(&mut self.project.beats_per_bar).range(1..=32))
                    .changed()
                {
                    self.dirty = true;
                }
            });
        });
    }

    fn automation_panel(&mut self, ui: &mut egui::Ui) {
        if self.project.tracks.is_empty() {
            return;
        }
        egui::Panel::bottom("automation")
            .resizable(true)
            .default_size(220.0)
            .min_size(150.0)
            .max_size(360.0)
            .show(ui, |ui| {
                if automation::show(ui, &mut self.project, &mut self.automation, &self.roll) {
                    self.dirty = true;
                    self.status = automation::edit_status(
                        &self.project.tracks[self.roll.selected_track],
                        &self.automation,
                    );
                }
            });
    }

    fn track_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tracks")
            .default_size(230.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Tracks");
                    if ui.button("+").on_hover_text("Add track").clicked() {
                        self.add_track();
                    }
                    if !self.project.tracks.is_empty() {
                        if ui.button("-").on_hover_text("Delete track").clicked() {
                            self.remove_selected_track();
                        }
                        if ui.button("Dup").on_hover_text("Duplicate track").clicked() {
                            self.duplicate_selected_track();
                        }
                        if ui.button("↑").on_hover_text("Move track up").clicked() {
                            self.move_selected_track(-1);
                        }
                        if ui.button("↓").on_hover_text("Move track down").clicked() {
                            self.move_selected_track(1);
                        }
                    }
                });
                ui.separator();
                for index in 0..self.project.tracks.len() {
                    let track = &self.project.tracks[index];
                    let label = format!(
                        "{}  {} ({} notes)",
                        if track.muted { "M" } else { " " },
                        track.name,
                        track.notes.len()
                    );
                    if ui
                        .selectable_label(self.roll.selected_track == index, label)
                        .clicked()
                    {
                        self.roll.select_track(index);
                    }
                }
                ui.separator();
                let mut open_patch_lab = false;
                if let Some(track) = self
                    .project
                    .tracks
                    .get_mut(self.roll.selected_track)
                {
                    ui.heading("Track settings");
                    ui.label("Name");
                    if ui.text_edit_singleline(&mut track.name).changed() {
                        self.dirty = true;
                    }
                    ui.label("Patch (preset or relative .json)");
                    if ui.text_edit_singleline(&mut track.patch).changed() {
                        self.dirty = true;
                    }
                    if ui.button("Edit synth").clicked() {
                        open_patch_lab = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Bend range");
                        if ui
                            .add(
                                egui::DragValue::new(&mut track.pitch_bend_range_semitones)
                                    .range(0.01..=24.0),
                            )
                            .changed()
                        {
                            self.dirty = true;
                        }
                    });
                    if !track.pitch_bends.is_empty() {
                        ui.label(format!("{} pitch-bend events", track.pitch_bends.len()));
                    }
                    if !track.aftertouch.is_empty() {
                        ui.label(format!("{} aftertouch events", track.aftertouch.len()));
                    }
                    if !track.controllers.is_empty() {
                        ui.label(format!("{} controller events", track.controllers.len()));
                    }
                    if ui
                        .add(egui::Slider::new(&mut track.gain, 0.0..=1.0).text("Gain"))
                        .changed()
                    {
                        self.dirty = true;
                    }
                    if ui
                        .add(egui::Slider::new(&mut track.pan, -1.0..=1.0).text("Pan"))
                        .changed()
                    {
                        self.dirty = true;
                    }
                    if ui.checkbox(&mut track.muted, "Muted").changed() {
                        self.dirty = true;
                    }
                }
                if open_patch_lab {
                    self.open_patch_lab();
                }
                ui.separator();
                ui.label("Click empty space to add a note. Drag notes to move them or drag the right edge to resize. Delete removes the selected note. Ctrl/Cmd+Z/Y undoes/redoes edits.");
            });
    }

    fn note_inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("note-inspector").show(ui, |ui| {
            let Some(note_index) = self.roll.selected_note else {
                ui.label("No note selected");
                return;
            };
            let Some(note) = self
                .project
                .tracks
                .get_mut(self.roll.selected_track)
                .and_then(|track| track.notes.get_mut(note_index))
            else {
                self.roll.selected_note = None;
                return;
            };
            let mut changed = false;
            let mut remove = false;
            ui.horizontal(|ui| {
                ui.label("Note");
                ui.label("Pitch");
                changed |= ui
                    .add(egui::DragValue::new(&mut note.pitch).range(0..=127))
                    .changed();
                ui.label("Velocity");
                changed |= ui
                    .add(egui::DragValue::new(&mut note.velocity).range(1..=127))
                    .changed();
                ui.label("Start");
                changed |= ui.add(egui::DragValue::new(&mut note.start_tick)).changed();
                ui.label("Duration");
                changed |= ui
                    .add(egui::DragValue::new(&mut note.duration_ticks).range(1..=1_000_000))
                    .changed();
                remove = ui.button("Delete note").clicked();
            });
            if changed {
                self.dirty = true;
            }
            if remove {
                self.project.tracks[self.roll.selected_track]
                    .notes
                    .remove(note_index);
                self.roll.selected_note = None;
                self.dirty = true;
            }
        });
    }

    fn poll_instance(&mut self, context: &egui::Context) {
        for _ in 0..4 {
            let message = match &self.instance {
                Some(instance) => match instance.try_recv() {
                    Ok(message) => message,
                    Err(error) => {
                        self.status = format!("Could not receive launch request: {error}");
                        None
                    }
                },
                None => None,
            };
            let Some(options) = message else {
                break;
            };
            self.open_launch(options);
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn about_window(&mut self, context: &egui::Context) {
        if !self.show_about {
            return;
        }
        let splash = self.splash.clone();
        let mut start_composing = false;
        let mut open_project = false;
        egui::Window::new("Welcome to MIDI Evil")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut self.show_about)
            .show(context, |ui| {
                ui.image((splash.id(), egui::vec2(320.0, 192.0)));
                ui.label("A tiny synthesizer and MIDI composer.");
                ui.horizontal(|ui| {
                    start_composing = ui.button("Start composing").clicked();
                    open_project = ui.button("Open project").clicked();
                });
            });
        if start_composing {
            self.show_about = false;
        }
        if open_project {
            self.show_about = false;
            self.open_project();
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(if self.dirty { "●" } else { "✓" });
                    ui.label(&self.status);
                    if let Some(path) = &self.project_path {
                        ui.label(RichText::new(path.display().to_string()).color(Color32::GRAY));
                    }
                });
            });
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let previous_project = self.project.clone();
        root.ctx().set_visuals(egui::Visuals::dark());
        self.poll_render();
        self.poll_instance(root.ctx());
        self.shortcuts(root.ctx());
        if self
            .preview
            .as_ref()
            .is_some_and(PreviewPlayer::is_finished)
        {
            self.preview = None;
        }
        self.toolbar(root);
        self.note_inspector(root);
        self.status_bar(root);
        self.automation_panel(root);
        self.track_panel(root);
        egui::CentralPanel::default().show(root, |ui| {
            if self.project.tracks.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Add a track to begin composing");
                });
            } else {
                let track_name = self.project.tracks[self.roll.selected_track].name.clone();
                ui.heading(track_name);
                if piano_roll::show(ui, &mut self.project, &mut self.roll) {
                    self.dirty = true;
                    self.status = "Notes edited".to_owned();
                }
            }
        });
        self.about_window(root.ctx());
        self.patch_lab_window(root.ctx());
        if self.resetting_document {
            self.resetting_document = false;
        } else if previous_project != self.project {
            if !self.restoring_history {
                self.history.record(previous_project);
            }
            self.dirty = true;
        }
        self.restoring_history = false;
    }
}
