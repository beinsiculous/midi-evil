use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, TextureHandle};

mod chrome;

use crate::{
    theme,
    workspace::{self, View},
};
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
    controls: TextureHandle,
    view: View,
    show_export: bool,
    show_automation: bool,
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
        theme::install(&context.egui_ctx);
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
            controls: art::load_controls(&context.egui_ctx),
            view: View::default(),
            show_export: false,
            show_automation: false,
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
            if self.preview.is_some() || self.render_job.is_some() {
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
        let Some(track) = self.project.tracks.get(track_index) else {
            return;
        };
        match evil_render::load_patch(&track.patch, self.patch_base()) {
            Ok(patch) => {
                let file_name = if track.patch.ends_with(".json") {
                    track.patch.clone()
                } else {
                    format!("{}-custom.json", track.patch)
                };
                self.patch_lab = Some(PatchLab::new(patch, file_name, track_index));
                self.view = View::Synth;
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

    fn patch_lab_panel(&mut self, ui: &mut egui::Ui) {
        let Some(mut lab) = self.patch_lab.take() else {
            ui.label("Select a track and open its instrument to start designing a sound.");
            if ui.button("Open selected instrument").clicked() {
                self.open_patch_lab();
            }
            return;
        };
        let mut action = None;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Evil Synth");
                theme::label(
                    ui,
                    format!(
                        "PATCH LAB  /  {}",
                        self.project
                            .tracks
                            .get(lab.track_index)
                            .map_or("Unassigned", |track| track.name.as_str())
                    ),
                );
            });
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
                patch_lab::show(ui, &mut lab);
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
        self.patch_lab = Some(lab);
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
}

impl eframe::App for StudioApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let previous_project = self.project.clone();
        // Poll background renders, audio completion and incoming launch requests while idle.
        root.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));

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
        self.status_bar(root);
        self.track_panel(root);
        if self.view == View::Notes {
            self.note_inspector(root);
            if self.show_automation {
                self.automation_panel(root);
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(16))
            .show(root, |ui| {
                if self.project.tracks.is_empty() {
                    ui.centered_and_justified(|ui| {
                        if ui.button("+ Add a track to begin composing").clicked() {
                            self.add_track();
                        }
                    });
                    return;
                }
                match self.view {
                    View::Song => {
                        let mut selected = self.roll.selected_track;
                        if workspace::song(ui, &self.project, &mut selected) {
                            self.view = View::Notes;
                        }
                        if selected != self.roll.selected_track {
                            self.roll.select_track(selected);
                        }
                    }
                    View::Notes => {
                        ui.horizontal(|ui| {
                            ui.heading(&self.project.tracks[self.roll.selected_track].name);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.toggle_value(&mut self.show_automation, "Automation");
                                },
                            );
                        });
                        if piano_roll::show(ui, &mut self.project, &mut self.roll) {
                            self.dirty = true;
                            self.status = "Notes edited".to_owned();
                        }
                    }
                    View::Mixer => {
                        let mut selected = self.roll.selected_track;
                        workspace::mixer(ui, &mut self.project, &mut selected);
                        if selected != self.roll.selected_track {
                            self.roll.select_track(selected);
                        }
                    }
                    View::Synth => self.patch_lab_panel(ui),
                }
            });
        self.about_window(root.ctx());
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
