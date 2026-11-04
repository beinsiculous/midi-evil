use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, TextureHandle};

mod chrome;
mod keyboard_page;
mod recording;

use crate::{
    theme,
    workspace::{self, View},
};
use evil_project::{Instrument, InstrumentSource, Project, Track};

use crate::art;
use crate::automation::{self, AutomationState};
use crate::history::EditorHistory;
use crate::instance::SingleInstance;
use crate::launch::LaunchOptions;
use crate::patch_lab::{self, PatchLab};
use crate::piano_roll::{self, RollState};
use crate::preview::PreviewPlayer;
use crate::render_job::RenderJob;
use crate::transport::Transport;

pub struct StudioApp {
    project: Project,
    project_path: Option<PathBuf>,
    source_midi: Option<PathBuf>,
    /// The file track index each project track came from, for a project imported from a
    /// Standard MIDI File: the import drops tracks holding no events (a tempo-only track
    /// 0, a muted track exported empty), so the engine's `--track-index` is a file index,
    /// not a project one. `None` for a project opened from `.evil` or started new, where
    /// the project order is the only order there is.
    imported_track_indices: Option<Vec<usize>>,
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
    transport: Transport,
    instance: Option<SingleInstance>,
    history: EditorHistory,
    resetting_document: bool,
    restoring_history: bool,
    render_job: Option<RenderJob>,
    pending_render: Option<PendingRender>,
    patch_lab: Option<PatchLab>,
    keyboard: crate::keyboard::KeyboardState,
    keyboard_player: Option<crate::live_player::LivePlayer>,
    keyboard_badge: TextureHandle,
    keyboard_binding: Option<(usize, String)>,
    keyboard_error: Option<String>,
    beats: crate::beats::BeatsPage,
    beats_binding: Option<(usize, String)>,
    recorder: Option<crate::recording::Recorder>,
    recording_started_at: Option<std::time::Instant>,
    recording_snapshot: Option<Project>,
    record_quantize: bool,
    midi_input: crate::midi_input::MidiInput,
    pending_midi_notes: Vec<crate::keyboard::NoteEvent>,
}

enum PendingRender {
    Preview {
        start_tick: u64,
        end_tick: Option<u64>,
        repeat: bool,
    },
    Resample {
        track_index: usize,
        start_tick: u64,
        end_tick: Option<u64>,
    },
    Wav(PathBuf),
}

enum PatchLabAction {
    New,
    Open,
    Duplicate,
    Reset,
    SaveAs,
}

fn portable_file_reference(path: &Path, base: &Path) -> String {
    let absolute_base = std::path::absolute(base).unwrap_or_else(|_| base.to_path_buf());
    path.strip_prefix(&absolute_base)
        .or_else(|_| path.strip_prefix(base))
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
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
            imported_track_indices: None,
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
            transport: Transport::new(1_920),
            instance,
            history: EditorHistory::default(),
            resetting_document: false,
            restoring_history: false,
            render_job: None,
            pending_render: None,
            patch_lab: None,
            keyboard: crate::keyboard::KeyboardState::default(),
            keyboard_player: None,
            keyboard_badge: art::load_keyboard(&context.egui_ctx),
            keyboard_binding: None,
            keyboard_error: None,
            beats: crate::beats::BeatsPage::new(&context.egui_ctx),
            beats_binding: None,
            recorder: None,
            recording_started_at: None,
            recording_snapshot: None,
            record_quantize: true,
            midi_input: crate::midi_input::MidiInput::default(),
            pending_midi_notes: Vec::new(),
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
        match (options.track_index, options.track) {
            (Some(index), _) => self.select_track_by_file_index(index),
            (None, Some(track)) => self.select_track_by_name(&track),
            (None, None) => {}
        }
    }

    /// The engine's `--track-index`: the track's 0-based position in the file's own track
    /// order, which wins over `--track` when both are given.
    fn select_track_by_file_index(&mut self, file_index: usize) {
        let position = track_position(
            file_index,
            self.imported_track_indices.as_deref(),
            self.project.tracks.len(),
        );
        if let Some(position) = position {
            let name = self.project.tracks[position].name.clone();
            self.roll.select_track(position);
            self.status = format!("Selected track {file_index} '{name}'");
        } else {
            eprintln!("warning: track index {file_index} was not found");
            self.status = format!("Track index {file_index} was not found");
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

    fn structural_edit_allowed(&mut self) -> bool {
        if self.recorder.is_some() {
            self.status = "Commit the current take before changing tracks".to_owned();
            false
        } else {
            true
        }
    }

    fn invalidate_beats_link(&mut self) {
        self.beats_binding = None;
        self.beats.clear_instrument_link();
    }

    fn reset_history(&mut self) {
        self.history.clear();
        self.resetting_document = true;
    }

    fn undo(&mut self) {
        let recording_snapshot = self.recording_snapshot.clone();
        if self.recorder.is_some() {
            self.stop_preview();
            if let Some(snapshot) = recording_snapshot {
                self.history.record(snapshot);
            }
        }
        if let Some(project) = self.history.undo(&self.project) {
            self.project = project;
            self.invalidate_beats_link();
            self.roll.clamp(&self.project);
            self.restoring_history = true;
            self.dirty = true;
            self.status = "Undo".to_owned();
        }
    }

    fn redo(&mut self) {
        if self.recorder.is_some() {
            self.stop_preview();
            return;
        }
        if let Some(project) = self.history.redo(&self.project) {
            self.project = project;
            self.invalidate_beats_link();
            self.roll.clamp(&self.project);
            self.restoring_history = true;
            self.dirty = true;
            self.status = "Redo".to_owned();
        }
    }

    fn remove_selected_track(&mut self) {
        if !self.structural_edit_allowed() {
            return;
        }
        let index = self.roll.selected_track;
        if self.project.remove_track(index).is_ok() {
            self.invalidate_beats_link();
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
        if !self.structural_edit_allowed() {
            return;
        }
        match self.project.duplicate_track(self.roll.selected_track) {
            Ok(index) => {
                self.invalidate_beats_link();
                self.roll.select_track(index);
                self.dirty = true;
                self.status = format!("Duplicated track {index}");
            }
            Err(error) => self.status = format!("Could not duplicate track: {error}"),
        }
    }

    fn move_selected_track(&mut self, offset: isize) {
        if !self.structural_edit_allowed() {
            return;
        }
        let index = self.roll.selected_track;
        let new_index = index as isize + offset;
        if new_index < 0 || new_index >= self.project.tracks.len() as isize {
            return;
        }
        if self.project.move_track(index, new_index as usize).is_ok() {
            self.invalidate_beats_link();
            self.roll.select_track(new_index as usize);
            self.dirty = true;
        }
    }

    fn can_discard_changes(&mut self) -> bool {
        let unsaved = self.dirty || self.recorder.is_some();
        if unsaved {
            self.stop_keyboard();
            self.beats.stop();
        }
        !unsaved
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
            if self.view == View::Pads {
                self.beats.save();
                self.link_saved_beats_bank();
            } else {
                self.save_project();
            }
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
        if !self.structural_edit_allowed() {
            return;
        }
        let number = self.project.tracks.len() + 1;
        let name = format!("Track {number}");
        let result = self
            .project
            .add_instrument(Instrument::synth(&name, "lead"))
            .and_then(|instrument| {
                self.project.add_track(Track {
                    name,
                    instrument,
                    ..Track::default()
                })
            });
        match result {
            Ok(index) => {
                self.roll.select_track(index);
                self.dirty = true;
                self.status = "Track added".to_owned();
            }
            Err(error) => self.status = format!("Could not add track: {error}"),
        }
    }

    fn add_drum_track(&mut self) {
        if !self.structural_edit_allowed() {
            return;
        }
        let number = self.project.tracks.len() + 1;
        let name = format!("Drums {number}");
        let result = self
            .project
            .add_instrument(Instrument::drum_kit(&name, "maxwell"))
            .and_then(|instrument| {
                self.project.add_track(Track {
                    name,
                    instrument,
                    ..Track::default()
                })
            });
        match result {
            Ok(index) => {
                self.roll.select_track(index);
                self.view = View::Pads;
                self.dirty = true;
                self.status = "Drum track added with the Maxwell kit".to_owned();
            }
            Err(error) => self.status = format!("Could not add drum track: {error}"),
        }
    }

    fn new_project(&mut self) {
        if !self.can_discard_changes() {
            return;
        }
        self.stop_preview();
        self.patch_lab = None;
        self.project = Project::new("Untitled");
        self.imported_track_indices = None;
        self.show_about = false;
        self.project_path = None;
        self.source_midi = None;
        self.roll = RollState::default();
        self.transport = Transport::new(piano_roll::bar_ticks(&self.project));
        self.automation = AutomationState::default();
        self.invalidate_beats_link();
        self.add_track();
        self.reset_history();
        self.dirty = false;
        self.status = "New project".to_owned();
    }

    fn open_patch_lab(&mut self) -> bool {
        let track_index = self.roll.selected_track;
        let Ok(instrument) = self.project.instrument_for_track(track_index) else {
            return false;
        };
        let Some(source) = instrument.patch().map(str::to_owned) else {
            self.status =
                "The selected instrument is a drum kit; use the Pads workspace".to_owned();
            return false;
        };
        match evil_render::load_patch(&source, self.patch_base()) {
            Ok(patch) => {
                let file_name = if source.ends_with(".json") {
                    source
                } else {
                    format!("{source}-custom.json")
                };
                self.patch_lab = Some(PatchLab::new(patch, file_name, track_index));
                self.view = View::Synth;
                true
            }
            Err(error) => {
                self.status = format!("Could not load synth patch: {error}");
                false
            }
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
        let reference = portable_file_reference(&output, patch_base);
        let instrument = match self.project.instrument_for_track_mut(lab.track_index) {
            Ok(instrument) => instrument,
            Err(error) => {
                self.status = format!("Could not assign synth patch: {error}");
                return;
            }
        };
        instrument.source = InstrumentSource::Synth {
            patch: reference.clone(),
        };
        instrument.midi_program = lab.patch.midi_program;
        self.keyboard_binding = Some((lab.track_index, reference.clone()));
        lab.mark_saved(reference);
        self.dirty = true;
        self.status = format!("Saved synth patch to {}", output.display());
    }

    fn patch_lab_panel(&mut self, ui: &mut egui::Ui) {
        if !self.prepare_keyboard(ui) {
            return;
        }
        let Some(mut lab) = self.patch_lab.take() else {
            ui.label("Select a track and open its instrument to start designing a sound.");
            if ui.button("Open selected instrument").clicked() {
                self.open_patch_lab();
            }
            return;
        };
        let events = egui::Panel::bottom("synth-keyboard")
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin {
                        top: 10,
                        ..Default::default()
                    }),
            )
            .show(ui, |ui| {
                crate::keyboard::show_compact(ui, &mut self.keyboard)
            })
            .inner;
        let mut action = None;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Evil Synth");
                ui.label(RichText::new(&lab.patch.name).color(theme::PURPLE));
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
            });
            ui.separator();
            patch_lab::tabs(ui, &mut lab);
            let message = egui::ScrollArea::vertical()
                .id_salt(("patch-controls", lab.page))
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if lab.page == patch_lab::PatchPage::Samples || lab.recording_active() {
                        self.sample_capture_controls(ui, &mut lab);
                    }
                    patch_lab::show(ui, &mut lab)
                })
                .inner;
            if let Some(message) = message {
                self.status = message;
            }
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
            None => {}
        }
        self.play_keyboard_events(ui, &lab.patch, events);
        self.patch_lab = Some(lab);
    }

    fn sample_capture_controls(&mut self, ui: &mut egui::Ui, lab: &mut PatchLab) {
        ui.horizontal_wrapped(|ui| {
            let can_capture = !lab.patch.oscillators.iter().skip(1)
                .any(|osc| osc.waveform == evil_synth::Waveform::Sample);
            if ui.add_enabled(
                can_capture && !lab.recording_active(),
                egui::Button::new("Resample section to Osc A"),
            )
            .on_hover_text(
                "Embed the saved instruments' song section, using the transport start and loop end. \
                 Only one sample oscillator can be active.",
            )
            .clicked() {
                let (start_tick, end_tick) = self.transport.range();
                self.start_render(PendingRender::Resample {
                    track_index: lab.track_index,
                    start_tick,
                    end_tick,
                });
            }
            let recording = lab.recording_active();
            let record_label = if recording { "Stop & embed mic" } else { "Record mic to Osc A" };
            if ui.add_enabled(
                can_capture || recording,
                egui::Button::new(record_label).selected(recording),
            )
            .on_hover_text(
                "Record up to 30 seconds into Osc A; stop to embed the audio. \
                 Only one sample oscillator can be active.",
            )
            .clicked() {
                let result = if recording { lab.stop_recording() } else { lab.start_recording() };
                self.status = match result {
                    Ok(message) | Err(message) => message,
                };
            }
        });
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
                self.imported_track_indices = None;
                self.show_about = false;
                self.source_midi = self.project.source_midi.as_ref().map(PathBuf::from);
                self.roll = RollState::default();
                self.transport = Transport::new(piano_roll::bar_ticks(&self.project));
                self.automation = AutomationState::default();
                self.invalidate_beats_link();
                self.project_path = Some(path.to_path_buf());
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
                match evil_exports::export_native_project(&self.project, self.patch_base()) {
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
        match evil_exports::export_native_project_to_path(&self.project, self.patch_base(), &output)
        {
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
        let result = evil_imports::import_midi_file(path)
            .map_err(|error| error.to_string())
            .and_then(|performance| {
                let skipped = performance.skipped_controller_events();
                let bends = performance.pitch_bend_count();
                let pressure = performance.aftertouch_count();
                let controllers = performance.controller_count();
                let file_indices: Vec<usize> = performance
                    .tracks
                    .iter()
                    .map(|track| track.track_index)
                    .collect();
                Project::from_performance(name, &performance)
                    .map(|project| (project, file_indices, skipped, bends, pressure, controllers))
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok((project, file_indices, skipped, bends, pressure, controllers)) => {
                self.stop_preview();
                self.patch_lab = None;
                self.project = project;
                self.imported_track_indices = Some(file_indices);
                self.show_about = false;
                self.source_midi = Some(path.to_path_buf());
                self.project.source_midi = Some(path.to_string_lossy().into_owned());
                self.project_path = Some(path.with_extension("evil"));
                self.roll = RollState::default();
                self.transport = Transport::new(piano_roll::bar_ticks(&self.project));
                self.automation = AutomationState::default();
                self.invalidate_beats_link();
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

    fn song_playhead_tick(&self) -> u64 {
        if let Some(preview) = &self.preview {
            return self.tick_at_elapsed(preview.position().as_secs_f64());
        }
        if self.recording_started_at.is_some() {
            return self.recording_tick();
        }
        self.transport.start_tick
    }

    fn recording_tick(&self) -> u64 {
        self.recording_tick_offset(0.0)
    }

    fn recording_tick_offset(&self, offset_seconds: f64) -> u64 {
        let Some(started_at) = self.recording_started_at else {
            return self.transport.start_tick;
        };
        self.tick_at_elapsed(started_at.elapsed().as_secs_f64() + offset_seconds)
    }

    fn tick_at_elapsed(&self, elapsed: f64) -> u64 {
        let (_, end_tick) = self.transport.range();
        crate::recording::recording_tick_at_elapsed(
            &self.project,
            self.transport.start_tick,
            end_tick,
            elapsed,
        )
        .unwrap_or(self.transport.start_tick)
    }

    fn preview_project(&mut self) {
        let (start_tick, end_tick) = self.transport.range();
        self.start_render(PendingRender::Preview {
            start_tick,
            end_tick,
            repeat: self.transport.repeat,
        });
    }

    fn start_render(&mut self, action: PendingRender) {
        if self.render_job.is_some() {
            self.status = "Already rendering; cancel it before starting another".to_owned();
            return;
        }
        self.stop_preview();
        let job = match &action {
            PendingRender::Preview {
                start_tick,
                end_tick,
                ..
            }
            | PendingRender::Resample {
                start_tick,
                end_tick,
                ..
            } => RenderJob::start_range(
                self.project.clone(),
                self.patch_base().to_path_buf(),
                *start_tick,
                *end_tick,
            ),
            PendingRender::Wav(_) => {
                RenderJob::start(self.project.clone(), self.patch_base().to_path_buf())
            }
        };
        self.render_job = Some(job);
        self.status = match &action {
            PendingRender::Resample { .. } => {
                "Resampling the section in the background…".to_owned()
            }
            PendingRender::Preview { .. } | PendingRender::Wav(_) => {
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
            (Some(PendingRender::Preview { repeat, .. }), Ok(rendered)) => {
                let status = automation::render_status(
                    format!("Previewing {} notes", rendered.note_count),
                    rendered.unsupported_controller_events,
                    rendered.skipped_drum_notes,
                );
                self.play_render(rendered, status, repeat);
            }
            (Some(PendingRender::Resample { track_index, .. }), Ok(rendered)) => {
                match evil_synth::SampleOscillator::from_stereo("Project section", &rendered.sound)
                {
                    Ok(sample) => {
                        if let Some(lab) = self
                            .patch_lab
                            .as_mut()
                            .filter(|lab| lab.track_index == track_index)
                        {
                            let oscillator = &mut lab.patch.oscillators[0];
                            oscillator.sample = Some(sample);
                            oscillator.sample_zones.clear();
                            oscillator.waveform = evil_synth::Waveform::Sample;
                            self.status =
                                "Resampled section into Osc A; use Save As to keep it".to_owned();
                        } else {
                            self.status =
                                "Resampling finished, but the original Patch Lab is no longer open"
                                    .to_owned();
                        }
                    }
                    Err(error) => self.status = format!("Could not embed resampled audio: {error}"),
                }
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
                            rendered.skipped_drum_notes,
                        );
                    }
                    Err(error) => self.status = format!("Could not write audio: {error}"),
                }
            }
            (_, Err(error)) => {
                self.status = if self.recorder.is_some() {
                    format!(
                        "Recording with the independent clock; audio preview unavailable: {error}"
                    )
                } else {
                    format!("Could not render audio: {error}")
                };
            }
            _ => {}
        }
    }

    fn play_render(&mut self, rendered: evil_render::RenderOutput, status: String, repeat: bool) {
        let result = if repeat {
            PreviewPlayer::play_repeating(rendered.sound)
        } else {
            PreviewPlayer::play(rendered.sound)
        };
        match result {
            Ok(player) => {
                self.preview = Some(player);
                self.status = status;
            }
            Err(error) => self.status = format!("Could not start audio preview: {error}"),
        }
    }

    fn stop_preview(&mut self) {
        self.finish_recording();
        self.stop_keyboard();
        self.beats.stop();
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

    fn sync_beats_instrument(&mut self) {
        let track_index = self.roll.selected_track;
        let Some(source) = self
            .project
            .tracks
            .get(track_index)
            .and_then(|track| self.project.instruments.get(track.instrument))
            .and_then(|instrument| match &instrument.source {
                InstrumentSource::DrumKit { kit, .. } => Some(kit.clone()),
                InstrumentSource::Synth { .. } => None,
            })
        else {
            return;
        };
        let binding = (track_index, source.clone());
        if self.beats_binding.as_ref() == Some(&binding) || self.beats.has_unsaved_changes() {
            return;
        }
        let base = self.patch_base().to_path_buf();
        match evil_render::load_drum_kit(&source, &base) {
            Ok(kit) => {
                let path = (source != "maxwell").then(|| base.join(&source));
                self.beats.load_instrument(kit, path, track_index);
                self.status = format!("Pads linked to song track {:02}", track_index + 1);
            }
            Err(error) => self.status = format!("Could not load track drum bank: {error}"),
        }
        self.beats_binding = Some(binding);
    }

    fn link_saved_beats_bank(&mut self) {
        let Some((track_index, path)) = self.beats.take_saved_bank() else {
            return;
        };
        let reference = portable_file_reference(&path, self.patch_base());
        let result = self
            .project
            .instrument_for_track_mut(track_index)
            .map_err(|error| error.to_string())
            .and_then(|instrument| match &mut instrument.source {
                InstrumentSource::DrumKit { kit, .. } => {
                    *kit = reference.clone();
                    Ok(())
                }
                InstrumentSource::Synth { .. } => {
                    Err("The linked track is no longer a drum instrument".to_owned())
                }
            });
        match result {
            Ok(()) => {
                self.beats_binding = Some((track_index, reference));
                self.dirty = true;
                self.status = format!(
                    "Saved {} and linked song track {:02}",
                    path.display(),
                    track_index + 1
                );
            }
            Err(error) => {
                self.status = format!("Saved {}, but could not link it: {error}", path.display())
            }
        }
    }

    fn file_dialog(&mut self) -> rfd::FileDialog {
        // Native dialogs block UI note-off processing until they close.
        self.stop_keyboard();
        self.beats.stop();
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
                    let track = &self.project.tracks[self.roll.selected_track];
                    let sequence: &dyn evil_project::EventSequence = self
                        .roll
                        .selected_part
                        .and_then(|part| track.parts.get(part))
                        .map_or(track as &dyn evil_project::EventSequence, |part| {
                            &self.project.patterns[part.pattern]
                        });
                    self.status = automation::edit_status(sequence, &self.automation);
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
        self.poll_midi_input();
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
        if self.view != View::Pads {
            self.track_panel(root);
            self.beats.stop();
        }
        if !matches!(self.view, View::Synth | View::Keyboard) || self.project.tracks.is_empty() {
            self.stop_keyboard();
        }
        if matches!(self.view, View::Synth | View::Keyboard | View::Pads) || self.recorder.is_some()
        {
            root.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.view == View::Notes {
            self.note_inspector(root);
            if self.show_automation {
                self.automation_panel(root);
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(16))
            .show(root, |ui| {
                if self.project.tracks.is_empty() && self.view != View::Pads {
                    ui.centered_and_justified(|ui| {
                        if ui.button("+ Add a track to begin composing").clicked() {
                            self.add_track();
                        }
                    });
                    return;
                }
                match self.view {
                    View::Song => {
                        let previous_transport = self.transport;
                        let playhead_tick = self.song_playhead_tick();
                        let allow_structural_edits = self.recorder.is_none();
                        let result = workspace::song(
                            ui,
                            &mut self.project,
                            &mut self.roll,
                            &mut self.transport,
                            playhead_tick,
                            allow_structural_edits,
                        );
                        if result.edit_notes {
                            self.view = View::Notes;
                        }
                        if let Some(status) = result.status {
                            self.status = status;
                        }
                        if self.transport != previous_transport {
                            self.stop_preview();
                        }
                    }
                    View::Notes => {
                        let track = &self.project.tracks[self.roll.selected_track];
                        let part = self
                            .roll
                            .selected_part
                            .filter(|part| *part < track.parts.len());
                        let pattern = part.map(|part| track.parts[part].pattern);
                        ui.horizontal(|ui| {
                            ui.heading(&track.name);
                            if let Some(pattern) = pattern {
                                ui.label(
                                    RichText::new(format!(
                                        "/ {} (shared P{:03})",
                                        self.project.patterns[pattern].name,
                                        pattern + 1
                                    ))
                                    .color(theme::PURPLE),
                                );
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.toggle_value(&mut self.show_automation, "Automation");
                                    if ui.button("Arranger").clicked() {
                                        self.view = View::Song;
                                    }
                                },
                            );
                        });
                        let context = piano_roll::RollContext::new(&self.project);
                        let pattern_length =
                            pattern.map(|index| self.project.patterns[index].length_ticks);
                        let sequence: &mut dyn evil_project::EventSequence = match pattern {
                            Some(index) => &mut self.project.patterns[index],
                            None => &mut self.project.tracks[self.roll.selected_track],
                        };
                        if piano_roll::show(ui, context, sequence, &mut self.roll, pattern_length) {
                            self.dirty = true;
                            self.status = if pattern.is_some() {
                                "Shared pattern edited; all linked parts were updated".to_owned()
                            } else {
                                "Unique timeline notes edited".to_owned()
                            };
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
                    View::Keyboard => self.keyboard_panel(ui),
                    View::Pads => {
                        self.sync_beats_instrument();
                        let hits = self.beats.show(ui, self.project.tempo_bpm);
                        self.link_saved_beats_bank();
                        self.record_pad_hits(&hits);
                    }
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

/// The project track at a file's track index: through the import's record of where each
/// project track came from when there is one, else the index itself while it is in range.
fn track_position(
    file_index: usize,
    imported_track_indices: Option<&[usize]>,
    track_count: usize,
) -> Option<usize> {
    match imported_track_indices {
        Some(indices) => indices.iter().position(|&index| index == file_index),
        None => (file_index < track_count).then_some(file_index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_track_index_finds_the_project_track_past_the_tracks_the_import_dropped() {
        // A format-1 file whose track 0 held only the tempo and whose track 2 was a muted
        // track exported empty: the import kept file tracks 1, 3 and 4.
        let imported = [1, 3, 4];
        assert_eq!(track_position(3, Some(&imported), 3), Some(1));
        assert_eq!(track_position(4, Some(&imported), 3), Some(2));
        assert_eq!(
            track_position(0, Some(&imported), 3),
            None,
            "the tempo track was dropped"
        );
        assert_eq!(
            track_position(2, Some(&imported), 3),
            None,
            "the empty track was dropped"
        );
    }

    #[test]
    fn without_an_import_the_project_order_is_the_file_order() {
        assert_eq!(track_position(0, None, 2), Some(0));
        assert_eq!(track_position(1, None, 2), Some(1));
        assert_eq!(track_position(2, None, 2), None);
    }

    #[test]
    fn portable_references_work_with_relative_project_paths() {
        let base = Path::new("target/portable-reference");
        let path = std::path::absolute(base)
            .unwrap()
            .join("maxwell-kit.evilkit");
        assert_eq!(portable_file_reference(&path, base), "maxwell-kit.evilkit");
    }
}
