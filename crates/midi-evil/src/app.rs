use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, TextureHandle};

mod chrome;
#[cfg(test)]
mod command_outcome_tests;
#[cfg(test)]
mod dialog_tests;
mod drafts;
mod files;
#[cfg(test)]
mod key_dispatch_tests;
#[cfg(test)]
mod keyboard_model_tests;
mod keyboard_page;
mod keys;
mod notes_view;
mod recording;
#[cfg(test)]
mod song_tests;
mod song_view;
mod sound_library;
mod sounds;
#[cfg(test)]
mod surface_names_tests;
#[cfg(test)]
mod test_harness;
mod track_rack;
mod tracks;
mod transport_readout;
mod undo;

use crate::{
    mixer::{self, MixerState},
    theme,
    workspace::View,
};
use evil_project::{Instrument, InstrumentSource, Project, Track};

use crate::announcer::Announcer;
use crate::art;
use crate::automation::{self, AutomationState};
use crate::dialogs::Dialogs;
use crate::history::{EditorHistory, Snapshot};
use crate::instance::SingleInstance;
use crate::instruments::{portable_file_reference, rebase_project_sources};
use crate::launch::LaunchOptions;
use crate::patch_lab::{self, PatchLab};
use crate::piano_roll::{self, RollState};
use crate::preview::PreviewPlayer;
use crate::regions::{self, Region};
use crate::render_job::{RenderJob, RenderOwner, WavExportQueue};
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
    mixer: MixerState,
    transport: Transport,
    instance: Option<SingleInstance>,
    history: EditorHistory<undo::EditorSelection>,
    resetting_document: bool,
    saved_this_frame: Option<Project>,
    render_job: Option<RenderJob>,
    draining_render: Option<RenderJob>,
    draining_action: Option<PendingRender>,
    render_owner: RenderOwner,
    pending_render: Option<PendingRender>,
    queued_render: Option<AcceptedRender>,
    cancelled_render: Option<PendingRender>,
    completed_cancelled_status: Option<String>,
    recording_commit_status: Option<String>,
    patch_lab: Option<PatchLab>,
    keyboard: crate::keyboard::KeyboardState,
    keyboard_player: Option<crate::live_player::LivePlayer>,
    keyboard_badge: TextureHandle,
    keyboard_binding: Option<(usize, String)>,
    keyboard_error: Option<String>,
    beats: crate::beats::BeatsPage,
    beats_binding: Option<(usize, String)>,
    recorder: Option<crate::recording::Recorder>,
    recording_started_at: Option<web_time::Instant>,
    recording_snapshot: Option<Snapshot<undo::EditorSelection>>,
    record_quantize: bool,
    midi_input: crate::midi_input::MidiInput,
    pending_midi_notes: Vec<crate::keyboard::NoteEvent>,
    sounds: crate::instruments::SoundBrowser,
    recovery: crate::recovery::RecoveryWindow,
    drafts: drafts::SoundDrafts,
    announcer: Announcer,
    /// Counts the documents this window has held; a cursor never crosses from one to the next.
    document: u64,
    frame_start: Option<undo::FrameStart>,
    held_gesture: Option<undo::HeldGesture>,
    gestures_started: u64,
    song: crate::arranger::SongState,
    track_name_before_edit: Option<(usize, String)>,
    show_key_help: bool,
    dialogs: Box<dyn Dialogs>,
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
    RangeWav {
        output: PathBuf,
        start_tick: u64,
        end_tick: Option<u64>,
    },
    WavExport {
        current: evil_render::ExportSpec,
        queue: WavExportQueue,
    },
}

struct AcceptedRender {
    action: PendingRender,
    snapshot: std::sync::Arc<evil_render::RenderSnapshot>,
}

enum PatchLabAction {
    New,
    Open,
    Duplicate,
    Reset,
    Compare,
    SaveAs,
}

impl StudioApp {
    pub fn new(
        context: &eframe::CreationContext<'_>,
        options: LaunchOptions,
        instance: Option<SingleInstance>,
        dialogs: Box<dyn Dialogs>,
    ) -> Self {
        let egui_context = &context.egui_ctx;
        Self::with_dialogs(egui_context, context.storage, options, instance, dialogs)
    }

    fn with_dialogs(
        context: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        options: LaunchOptions,
        instance: Option<SingleInstance>,
        dialogs: Box<dyn Dialogs>,
    ) -> Self {
        theme::install(context);
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
            mascot: art::load_mascot(context),
            splash: art::load_splash(context),
            controls: art::load_controls(context),
            view: View::default(),
            show_export: false,
            show_automation: false,
            show_about: true,
            preview: None,
            mixer: MixerState::default(),
            transport: Transport::new(1_920),
            instance,
            history: EditorHistory::default(),
            resetting_document: false,
            saved_this_frame: None,
            render_job: None,
            draining_render: None,
            draining_action: None,
            render_owner: RenderOwner::default(),
            pending_render: None,
            queued_render: None,
            cancelled_render: None,
            completed_cancelled_status: None,
            recording_commit_status: None,
            patch_lab: None,
            keyboard: crate::keyboard::KeyboardState::default(),
            keyboard_player: None,
            keyboard_badge: art::load_keyboard(context),
            keyboard_binding: None,
            keyboard_error: None,
            beats: crate::beats::BeatsPage::new(context),
            beats_binding: None,
            recorder: None,
            recording_started_at: None,
            recording_snapshot: None,
            record_quantize: true,
            midi_input: crate::midi_input::MidiInput::default(),
            pending_midi_notes: Vec::new(),
            sounds: crate::instruments::SoundBrowser::restore(storage),
            recovery: crate::recovery::RecoveryWindow::default(),
            drafts: drafts::SoundDrafts::default(),
            announcer: Announcer::default(),
            document: 0,
            frame_start: None,
            held_gesture: None,
            gestures_started: 0,
            song: crate::arranger::SongState::default(),
            track_name_before_edit: None,
            show_key_help: false,
            dialogs,
        };
        if let Err(error) = app.add_track_quietly() {
            app.status = format!("Could not add a track: {error}");
        }
        app.dirty = false;
        app.open_launch(options);
        app
    }

    pub(crate) fn open_launch(&mut self, options: LaunchOptions) {
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

    fn invalidate_beats_link(&mut self) {
        self.beats_binding = None;
        self.beats.clear_instrument_link();
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
                let file_name = if source.to_ascii_lowercase().ends_with(".json") {
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

    fn save_patch_lab(&mut self, lab: &mut PatchLab) {
        if self.instrument_resolution_in_progress() {
            self.report_source_write_refusal("a patch");
            return;
        }
        if lab.track_index >= self.project.tracks.len() {
            self.status = "The patch's original track no longer exists".to_owned();
            return;
        }
        let Some(output) = self
            .file_dialog()
            .add_filter("Synth patch", &["json"])
            .set_file_name(lab.save_file_name())
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
        instrument.apply_patch_metadata(&lab.patch);
        self.keyboard_binding = Some((lab.track_index, reference.clone()));
        self.sounds.remember(crate::instruments::Sound {
            name: lab.patch.name.clone(),
            ..crate::instruments::Sound::file(&output, crate::instruments::SoundKind::Synth)
        });
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
            let comparing = lab.is_comparing();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!comparing, egui::Button::new("New"))
                    .clicked()
                {
                    action = Some(PatchLabAction::New);
                }
                if ui
                    .add_enabled(!comparing, egui::Button::new("Load patch…"))
                    .clicked()
                {
                    action = Some(PatchLabAction::Open);
                }
                if ui
                    .add_enabled(!comparing, egui::Button::new("Duplicate"))
                    .clicked()
                {
                    action = Some(PatchLabAction::Duplicate);
                }
                if ui
                    .add_enabled(!comparing, egui::Button::new("Reset"))
                    .clicked()
                {
                    action = Some(PatchLabAction::Reset);
                }
                if ui
                    .add_enabled(
                        !lab.recording_active(),
                        egui::Button::new("Compare").selected(comparing),
                    )
                    .on_hover_text(
                        "Audition the original preset; activate again to restore the parked edits",
                    )
                    .clicked()
                {
                    action = Some(PatchLabAction::Compare);
                }
                if ui
                    .add_enabled(
                        !comparing && !self.instrument_resolution_in_progress(),
                        egui::Button::new("Save & assign…"),
                    )
                    .clicked()
                {
                    action = Some(PatchLabAction::SaveAs);
                }
            });
            if comparing {
                ui.label(
                    RichText::new("Comparing original preset · edited version is parked")
                        .color(theme::IVORY),
                );
            }
            if lab.has_unsaved_changes() {
                ui.label(
                    RichText::new(
                        "Unsaved sound draft · Play it below; Save & assign updates song playback.",
                    )
                    .color(theme::IVORY),
                );
            }
            ui.separator();
            patch_lab::tabs(ui, &mut lab);
            let message = egui::ScrollArea::vertical()
                .id_salt(("patch-controls", lab.page))
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_enabled_ui(!comparing, |ui| {
                        if lab.page == patch_lab::PatchPage::Samples || lab.recording_active() {
                            self.sample_capture_controls(ui, &mut lab);
                        }
                        patch_lab::show(ui, &mut lab, self.dialogs.as_mut())
                    })
                    .inner
                })
                .inner;
            if let Some(message) = message {
                self.status = message;
            }
        });
        let load_patch = matches!(action, Some(PatchLabAction::Open));
        match action {
            Some(PatchLabAction::New) if self.confirm_sound_discard(lab.has_unsaved_changes()) => {
                lab = PatchLab::new_preset(lab.track_index);
                self.status = "New synth patch; use Save & assign to assign it".to_owned();
            }
            Some(PatchLabAction::Open) => {}
            Some(PatchLabAction::Duplicate) => {
                lab.duplicate();
                self.status = "Duplicated synth patch; use Save & assign to assign it".to_owned();
            }
            Some(PatchLabAction::Reset) => {
                lab.reset();
                self.status = "Restored the last opened, duplicated, or saved patch".to_owned();
            }
            Some(PatchLabAction::Compare) => {
                lab.toggle_compare();
                self.status = if lab.is_comparing() {
                    "Comparing the original preset; edits are parked".to_owned()
                } else {
                    "Restored the edited preset".to_owned()
                };
            }
            Some(PatchLabAction::SaveAs) => self.save_patch_lab(&mut lab),
            Some(PatchLabAction::New) | None => {}
        }
        self.play_keyboard_events(ui, &lab.patch, events);
        self.patch_lab = Some(lab);
        if load_patch {
            self.load_patch_draft();
        }
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

    fn preview_project(&mut self) {
        let (start_tick, end_tick) = self.transport.range();
        self.start_render(PendingRender::Preview {
            start_tick,
            end_tick,
            repeat: self.transport.repeat,
        });
    }

    fn queued_export_blocks_source_write(&self) -> bool {
        self.queued_render
            .as_ref()
            .is_some_and(|queued| queued.snapshot.is_resolving() || !queued.snapshot.is_resolved())
    }

    fn instrument_resolution_in_progress(&self) -> bool {
        self.render_job
            .as_ref()
            .is_some_and(|job| !job.is_cancelling() && job.is_resolving())
            || self.queued_export_blocks_source_write()
    }

    fn report_source_write_refusal(&mut self, target: &str) {
        if self
            .queued_render
            .as_ref()
            .is_some_and(|queued| Self::is_file_render(&queued.action))
        {
            self.report(format!(
                "Withdraw the queued WAV export before saving {target}"
            ));
        } else {
            self.report(format!(
                "Wait for render instrument resolution before saving {target}"
            ));
        }
    }

    fn file_export_in_progress(&self) -> bool {
        self.render_job.as_ref().is_some_and(|job| {
            self.pending_render
                .as_ref()
                .is_some_and(Self::is_file_render)
                && !job.is_cancelling()
        }) || self
            .queued_render
            .as_ref()
            .is_some_and(|queued| Self::is_file_render(&queued.action))
    }

    fn is_file_render(action: &PendingRender) -> bool {
        matches!(
            action,
            PendingRender::Wav(_)
                | PendingRender::RangeWav { .. }
                | PendingRender::WavExport { .. }
        )
    }

    fn render_output(action: &PendingRender) -> Option<PathBuf> {
        match action {
            PendingRender::Wav(output) | PendingRender::RangeWav { output, .. } => {
                Some(output.clone())
            }
            PendingRender::WavExport { current, queue } => Some(queue.output_for(current)),
            PendingRender::Preview { .. } | PendingRender::Resample { .. } => None,
        }
    }

    fn accept_render(&self, action: PendingRender) -> AcceptedRender {
        let snapshot = match &action {
            PendingRender::WavExport { queue, .. } => queue.snapshot(),
            PendingRender::Preview {
                start_tick,
                end_tick,
                ..
            }
            | PendingRender::Resample {
                start_tick,
                end_tick,
                ..
            }
            | PendingRender::RangeWav {
                start_tick,
                end_tick,
                ..
            } => std::sync::Arc::new(evil_render::RenderSnapshot::with_range(
                self.project.clone(),
                self.patch_base().to_path_buf(),
                *start_tick,
                *end_tick,
            )),
            PendingRender::Wav(_) => std::sync::Arc::new(evil_render::RenderSnapshot::new(
                self.project.clone(),
                self.patch_base().to_path_buf(),
            )),
        };
        AcceptedRender { action, snapshot }
    }

    fn start_render(&mut self, action: PendingRender) {
        if self.file_export_in_progress() {
            self.report("WAV export in progress; cancel it before requesting another render");
            return;
        }
        if Self::is_file_render(&action)
            || matches!(
                action,
                PendingRender::Resample { .. } | PendingRender::Preview { .. }
            )
        {
            self.finish_recording();
        }
        let accepted = self.accept_render(action);
        self.recording_commit_status = None;
        if self.render_job.is_some() {
            self.stop_preview();
            let file_outcome = Self::is_file_render(&accepted.action);
            self.queued_render = Some(accepted);
            if file_outcome {
                self.report("Cancelling current render; requested action queued…");
            } else {
                self.status = "Cancelling current render; requested action queued…".to_owned();
            }
            return;
        }
        self.stop_preview();
        self.start_render_now(accepted);
    }

    fn start_render_now(&mut self, accepted: AcceptedRender) {
        let AcceptedRender { action, snapshot } = accepted;
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
            } => RenderJob::start_range(snapshot, *start_tick, *end_tick, &self.render_owner),
            PendingRender::Wav(output) => {
                RenderJob::start_wav(snapshot, output.clone(), &self.render_owner)
            }
            PendingRender::RangeWav {
                output,
                start_tick,
                end_tick,
            } => RenderJob::start_range_wav(
                snapshot,
                *start_tick,
                *end_tick,
                output.clone(),
                &self.render_owner,
            ),
            PendingRender::WavExport { current, queue } => RenderJob::start_scoped_wav(
                snapshot,
                current.scope,
                queue.output_for(current),
                &self.render_owner,
            ),
        };
        self.render_job = Some(job);
        let status = match &action {
            PendingRender::Resample { .. } => {
                "Resampling the section in the background…".to_owned()
            }
            PendingRender::Preview { .. }
            | PendingRender::Wav(_)
            | PendingRender::RangeWav { .. } => "Rendering in the background…".to_owned(),
            PendingRender::WavExport { current, queue } => format!(
                "Rendering export {} of {}: {}…",
                queue.completed + 1,
                queue.total,
                current.file_name
            ),
        };
        // A render the person asked for says it has begun; playback and the later items of an
        // export are the transport's and the queue's to show.
        match &action {
            PendingRender::Wav(_) | PendingRender::RangeWav { .. } => self.report(status),
            PendingRender::WavExport { queue, .. } if queue.completed == 0 => self.report(status),
            _ => self.status = status,
        }
        self.pending_render = Some(action);
    }

    fn poll_render(&mut self) {
        let result = self.render_job.as_mut().and_then(RenderJob::try_finish);
        let Some(result) = result else {
            let detached = self.draining_render.is_none()
                && self
                    .render_job
                    .as_mut()
                    .is_some_and(RenderJob::detach_if_stalled);
            if detached {
                self.draining_render = self.render_job.take();
                self.draining_action = self.cancelled_render.take();
                if let Some(accepted) = self.queued_render.take() {
                    self.start_render_now(accepted);
                }
            } else {
                self.poll_draining_render();
            }
            if self.render_job.is_none()
                && self.draining_render.is_none()
                && self.queued_render.is_none()
            {
                self.compose_completed_cancelled_status();
            }
            return;
        };
        self.render_job = None;
        let action = self.pending_render.take();
        match (action, result) {
            (Some(PendingRender::Preview { repeat, .. }), Ok(rendered)) => {
                let status = automation::render_status(
                    format!("Previewing {} notes", rendered.note_count),
                    rendered.unsupported_controller_events,
                    rendered.unsupported_mixer_automation_events,
                    rendered.skipped_drum_notes,
                );
                self.play_render(rendered.output, status, repeat);
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
                                "Resampled section into Osc A; use Save & assign to keep it"
                                    .to_owned();
                        } else {
                            self.status =
                                "Resampling finished, but the original Patch Lab is no longer open"
                                    .to_owned();
                        }
                    }
                    Err(error) => self.status = format!("Could not embed resampled audio: {error}"),
                }
            }
            (
                Some(PendingRender::Wav(output) | PendingRender::RangeWav { output, .. }),
                Ok(rendered),
            ) => {
                self.report(automation::render_status(
                    format!(
                        "Rendered {} notes from {} tracks to {}",
                        rendered.note_count,
                        rendered.track_count,
                        output.file_name().map_or_else(
                            || output.display().to_string(),
                            |name| name.to_string_lossy().into_owned()
                        )
                    ),
                    rendered.unsupported_controller_events,
                    rendered.unsupported_mixer_automation_events,
                    rendered.skipped_drum_notes,
                ));
            }
            (
                Some(PendingRender::WavExport {
                    current: _,
                    mut queue,
                }),
                Ok(rendered),
            ) => {
                queue.complete_current();
                let warnings = (
                    rendered.unsupported_controller_events,
                    rendered.unsupported_mixer_automation_events,
                    rendered.skipped_drum_notes,
                );
                // Release the completed song buffer before starting the next worker.
                drop(rendered);
                if let Some(current) = queue.next_spec() {
                    let accepted = self.accept_render(PendingRender::WavExport { current, queue });
                    self.start_render_now(accepted);
                } else {
                    self.report(automation::render_status(
                        format!(
                            "Exported {} WAV file(s); skipped {} noteless item(s)",
                            queue.completed, queue.skipped
                        ),
                        warnings.0,
                        warnings.1,
                        warnings.2,
                    ));
                }
            }
            (Some(PendingRender::WavExport { current, queue }), Err(error)) => {
                self.report(if error.stage == crate::render_job::RenderStage::Resolution && !error.cancelled
                {
                    format!("Export batch stopped before any output; {error}")
                } else {
                    format!(
                        "Export batch stopped before {} after completing {} of {} file(s): {error}",
                        queue.output_for(&current).display(),
                        queue.completed,
                        queue.total
                    )
                });
            }
            (None, result) => {
                let cancelled = self.cancelled_render.take();
                let file_outcome = cancelled.as_ref().is_some_and(Self::is_file_render);
                self.finish_cancelled_render(cancelled, result);
                if file_outcome && (self.queued_render.is_some() || self.draining_render.is_some())
                {
                    self.completed_cancelled_status = Some(self.status.clone());
                }
            }
            (_, Err(error)) => {
                self.report(if self.recorder.is_some() {
                    format!(
                        "Recording with the independent clock; audio preview unavailable: {error}"
                    )
                } else {
                    format!("Could not render audio: {error}")
                });
            }
        }
        if let Some(accepted) = self.queued_render.take() {
            self.start_render_now(accepted);
        }
        self.poll_draining_render();
        if self.render_job.is_none()
            && self.draining_render.is_none()
            && self.queued_render.is_none()
        {
            self.compose_completed_cancelled_status();
        }
    }

    fn compose_completed_cancelled_status(&mut self) {
        if let Some(outcome) = self.completed_cancelled_status.take() {
            self.status = format!("{outcome}; {}", self.status);
        }
    }

    fn poll_draining_render(&mut self) {
        let Some(result) = self
            .draining_render
            .as_mut()
            .and_then(RenderJob::try_finish)
        else {
            return;
        };
        let active_status = self.status.clone();
        self.draining_render = None;
        let cancelled = self.draining_action.take();
        let file_outcome = cancelled.as_ref().is_some_and(Self::is_file_render);
        let previous_file_outcome = file_outcome
            .then(|| self.completed_cancelled_status.take())
            .flatten();
        self.finish_cancelled_render(cancelled, result);
        let outcome_status = self.status.clone();
        if file_outcome && outcome_status != active_status {
            self.status = previous_file_outcome.map_or_else(
                || outcome_status.clone(),
                |previous| format!("{outcome_status}; {previous}"),
            );
            if self.render_job.is_some() || self.queued_render.is_some() {
                self.completed_cancelled_status = Some(outcome_status);
            }
        }
    }

    fn finish_cancelled_render(
        &mut self,
        cancelled: Option<PendingRender>,
        result: Result<crate::render_job::RenderSuccess, crate::render_job::RenderError>,
    ) {
        match (cancelled, result) {
            (Some(PendingRender::WavExport { current, mut queue }), Ok(success))
                if success.published =>
            {
                queue.complete_current();
                self.report(format!(
                    "Published {} before cancelling the remaining batch; completed {} of {} file(s)",
                    queue.output_for(&current).display(),
                    queue.completed,
                    queue.total
                ));
            }
            (Some(PendingRender::WavExport { current, queue }), Err(error)) => {
                self.report(if error.stage == crate::render_job::RenderStage::Resolution && !error.cancelled
                {
                    format!("Export batch stopped before any output; {error}")
                } else {
                    format!(
                        "Export batch stopped before {} after completing {} of {} file(s): {error}",
                        queue.output_for(&current).display(),
                        queue.completed,
                        queue.total
                    )
                });
            }
            (
                Some(PendingRender::Wav(output) | PendingRender::RangeWav { output, .. }),
                Ok(success),
            ) if success.published => {
                self.report(format!(
                    "Published WAV export to {} before cancellation",
                    output.display()
                ));
            }
            (
                Some(PendingRender::Wav(output) | PendingRender::RangeWav { output, .. }),
                Err(error),
            ) => {
                self.report(format!(
                    "Cancelled WAV export to {}; no file completed: {error}",
                    output.display()
                ));
            }
            (Some(PendingRender::Preview { .. } | PendingRender::Resample { .. }), _)
                if self.status == "Cancelling current render…" =>
            {
                self.status = self
                    .recording_commit_status
                    .take()
                    .unwrap_or_else(|| "Stopped".to_owned());
            }
            (Some(PendingRender::Preview { .. } | PendingRender::Resample { .. }), _) => {}
            _ => {}
        }
    }

    fn play_render(&mut self, rendered: evil_render::RenderOutput, status: String, repeat: bool) {
        self.mixer.last_analysis = Some(rendered.mix_analysis.clone());
        self.mixer.last_channel_meters = rendered.channel_meters.clone();
        let result =
            PreviewPlayer::play_with_meters(rendered.sound, rendered.channel_meters, repeat);
        match result {
            Ok(player) => {
                self.preview = Some(player);
                self.status = status;
            }
            Err(error) => self.status = format!("Could not start audio preview: {error}"),
        }
    }

    fn withdraw_queued_render(&mut self) {
        let Some(withdrawn) = self.queued_render.take() else {
            return;
        };
        self.completed_cancelled_status = None;
        self.recording_commit_status = None;
        if let Some(output) = Self::render_output(&withdrawn.action) {
            self.report(format!(
                "Withdrew queued WAV export to {}",
                output.display()
            ));
        } else if self.status.starts_with("Cancelling current render") {
            self.status = "Stopped".to_owned();
        }
    }

    fn stop_preview(&mut self) {
        self.finish_recording();
        self.stop_keyboard();
        self.beats.stop();
        let preserve_job = self.render_job.as_ref().is_some_and(|job| {
            job.is_cancelling()
                || self
                    .pending_render
                    .as_ref()
                    .is_some_and(Self::is_file_render)
        });
        if !preserve_job {
            if self.render_job.is_some() {
                self.completed_cancelled_status = None;
            }
            if let Some(job) = &self.render_job {
                job.cancel();
            }
            self.pending_render = None;
        }
        let preserve_file_queue = self
            .queued_render
            .as_ref()
            .is_some_and(|queued| Self::is_file_render(&queued.action));
        if !preserve_file_queue {
            self.withdraw_queued_render();
        }
        if let Some(preview) = self.preview.take() {
            preview.stop();
        }
    }

    fn render_is_cancelling(&self) -> bool {
        self.render_job
            .as_ref()
            .is_some_and(RenderJob::is_cancelling)
    }

    fn cancel_active_render(&mut self) {
        let mut cancelled = None;
        if let Some(job) = &self.render_job {
            if !job.is_cancelling() {
                cancelled = self.pending_render.take();
                if cancelled
                    .as_ref()
                    .is_some_and(|action| !Self::is_file_render(action))
                {
                    self.completed_cancelled_status = None;
                }
                job.cancel();
            }
            self.pending_render = None;
            self.status = "Cancelling current render…".to_owned();
        }
        self.withdraw_queued_render();
        self.stop_preview();
        if cancelled.is_some() {
            self.cancelled_render = cancelled;
        }
    }

    fn shutdown_render_jobs(&mut self) {
        let jobs = [self.render_job.take(), self.draining_render.take()]
            .into_iter()
            .flatten()
            .collect();
        RenderJob::shutdown(jobs);
    }

    fn stop_transport(&mut self) {
        if self.queued_render.is_some() {
            self.withdraw_queued_render();
            self.stop_preview();
        } else if self.render_job.is_some() {
            if self
                .pending_render
                .as_ref()
                .is_some_and(Self::is_file_render)
            {
                let ended_take = self.recorder.is_some();
                self.stop_preview();
                if !ended_take && self.render_job.is_some() {
                    self.report("WAV export continues; explicit Cancel is required to stop it");
                }
            } else {
                self.cancel_active_render();
            }
        } else {
            self.stop_preview();
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
                let path = (!matches!(source.as_str(), "maxwell" | "chiptune"))
                    .then(|| base.join(&source));
                self.beats.load_instrument(kit, path, track_index);
                self.status = format!("Pads linked to song track {:02}", track_index + 1);
            }
            Err(error) => self.status = format!("Could not load track drum bank: {error}"),
        }
        self.beats_binding = Some(binding);
    }

    fn save_beats_bank(&mut self) {
        if self.instrument_resolution_in_progress() {
            self.report_source_write_refusal("a bank");
            return;
        }
        self.beats.save(self.dialogs.as_mut());
        self.link_saved_beats_bank();
    }

    fn link_saved_beats_bank(&mut self) {
        if self.instrument_resolution_in_progress() {
            return;
        }
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
                self.sounds.remember(crate::instruments::Sound::file(
                    &path,
                    crate::instruments::SoundKind::Kit,
                ));
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
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown_render_jobs();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.sounds.save(storage);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let previous_base = self.patch_base().to_path_buf();
        self.begin_project_frame();
        // Poll background renders, audio completion and incoming launch requests while idle.
        root.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));

        self.poll_render();
        self.poll_instance(root.ctx());
        self.poll_midi_input();
        self.sync_sound_drafts();
        self.dispatch_keys(root.ctx());
        if self
            .preview
            .as_ref()
            .is_some_and(PreviewPlayer::is_finished)
        {
            self.preview = None;
        }
        self.toolbar(root);
        self.sound_bar(root);
        self.status_bar(root);
        if self.view != View::Pads {
            self.track_panel(root);
            self.beats.stop();
        }
        if !matches!(self.view, View::Synth | View::Keyboard) || self.project.tracks.is_empty() {
            self.stop_keyboard();
        }
        if matches!(self.view, View::Synth | View::Keyboard | View::Pads)
            || self.recorder.is_some()
            || (self.view == View::Mixer && self.preview.is_some())
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
                regions::group(ui, Region::Editor, |ui| self.editor(ui));
            });
        self.sync_sound_drafts();
        self.about_window(root.ctx());
        self.sound_browser(root.ctx());
        self.key_help_window(root.ctx());
        if let Some(path) = self.recovery.show(root.ctx(), self.dialogs.as_mut()) {
            self.open_project_path(&path);
        }
        if let Some(outcome) = self.recovery.take_outcome() {
            self.report(outcome);
        }
        crate::accessibility::name_window(root.ctx());
        self.announcer.end_frame(root.ctx());
        regions::end_frame(root.ctx());
        let has_focus = |widget: Option<egui::Id>| {
            widget.is_some_and(|widget| root.ctx().memory(|memory| memory.has_focus(widget)))
        };
        self.roll.had_focus = has_focus(self.roll.widget_id);
        self.song.had_focus = has_focus(self.song.widget_id);
        if let Some(start) = self.frame_start.take() {
            self.finish_project_frame(start, &previous_base);
        }
    }
}

impl StudioApp {
    fn editor(&mut self, ui: &mut egui::Ui) {
        if self.project.tracks.is_empty() && self.view != View::Pads {
            ui.centered_and_justified(|ui| {
                if ui.button("+ Add a track to begin composing").clicked() {
                    self.add_track();
                }
            });
            return;
        }
        match self.view {
            View::Song => self.song_view(ui),
            View::Notes => self.notes_view(ui),
            View::Mixer => {
                let mut selected = self.roll.selected_track;
                let meter = self.preview.as_ref().map_or(
                    crate::preview::MeterSnapshot::default(),
                    PreviewPlayer::meter,
                );
                let channel_meters = self
                    .preview
                    .as_ref()
                    .map_or_else(Vec::new, PreviewPlayer::channel_meters);
                mixer::mixer(
                    ui,
                    &mut self.project,
                    &mut selected,
                    &mut self.mixer,
                    meter,
                    &channel_meters,
                );
                if selected != self.roll.selected_track {
                    self.select_sound_track(selected);
                }
            }
            View::Synth => self.patch_lab_panel(ui),
            View::Keyboard => self.keyboard_panel(ui),
            View::Pads => {
                self.sync_beats_instrument();
                let allow_bank_save = !self.instrument_resolution_in_progress();
                let hits = self.beats.show(
                    ui,
                    self.project.tempo_bpm,
                    self.dialogs.as_mut(),
                    allow_bank_save,
                );
                self.link_saved_beats_bank();
                self.record_pad_hits(&hits);
            }
        }
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

    fn app_with_queued_render() -> StudioApp {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.render_job = Some(RenderJob::cancelled_for_test());
        app.preview_project();
        assert!(app.queued_render.is_some());
        app
    }

    fn press_space(context: &egui::Context, app: &mut StudioApp) {
        let _ = context.run_ui(
            egui::RawInput {
                events: vec![
                    test_harness::press(egui::Key::Space, egui::Modifiers::NONE),
                    test_harness::release(egui::Key::Space),
                ],
                ..Default::default()
            },
            |ui| app.dispatch_keys(ui.ctx()),
        );
    }

    #[test]
    fn command_shift_z_redoes_without_first_undoing_another_edit() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.history
            .record(app.project.clone(), app.editor_selection(), None);
        app.project.name = "First edit".into();
        app.history
            .record(app.project.clone(), app.editor_selection(), None);
        app.project.name = "Second edit".into();
        app.undo();
        assert_eq!(app.project.name, "First edit");
        let modifiers = egui::Modifiers {
            command: true,
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        let _ = context.run_ui(
            egui::RawInput {
                modifiers,
                events: vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| app.dispatch_keys(ui.ctx()),
        );
        assert_eq!(app.project.name, "Second edit");
    }

    #[test]
    fn save_as_stays_clean_and_preserves_the_musical_undo_history() {
        let directory = tempfile::tempdir().unwrap();
        let old_base = directory.path().join("old");
        let new_base = directory.path().join("new");
        std::fs::create_dir(&old_base).unwrap();
        std::fs::create_dir(&new_base).unwrap();
        std::fs::write(
            old_base.join("lead.json"),
            evil_synth::Patch::default().to_json_pretty().unwrap(),
        )
        .unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project_path = Some(old_base.join("song.evil"));
        app.project.instruments[0].source = InstrumentSource::Synth {
            patch: "lead.json".into(),
        };
        app.history
            .record(app.project.clone(), app.editor_selection(), None);
        app.project.tracks[0]
            .notes
            .push(evil_project::Note::default());
        let previous = app.project.clone();
        let output = new_base.join("song.evil");
        app.save_to(output.clone());
        app.finish_project_frame(previous, &old_base);
        assert!(!app.dirty);
        assert_eq!(app.project, Project::load(&output).unwrap());
        app.undo();
        assert!(app.project.tracks[0].notes.is_empty());
        assert!(!app.history.can_undo(), "Save As must not add an undo step");
        assert!(
            evil_render::load_patch(app.project.instruments[0].patch().unwrap(), &new_base).is_ok()
        );
        app.redo();
        assert_eq!(app.project.tracks[0].notes.len(), 1);
        assert!(
            evil_render::load_patch(app.project.instruments[0].patch().unwrap(), &new_base).is_ok()
        );
    }

    #[test]
    fn edits_in_the_save_frame_remain_undoable_and_post_save_edits_stay_dirty() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        let previous = app.project.clone();
        let base = app.patch_base().to_path_buf();
        app.project.name = "Before save".into();
        let path = directory.path().join("song.evil");
        app.save_to(path.clone());
        app.finish_project_frame(previous, &base);
        assert!(!app.dirty);
        app.undo();
        assert_eq!(app.project.name, "Untitled");
        app.redo();
        let previous = app.project.clone();
        app.finish_project_frame(previous, directory.path());

        let previous = app.project.clone();
        app.save_to(path.clone());
        app.project.name = "After save".into();
        app.finish_project_frame(previous, directory.path());
        assert!(app.dirty);
        assert_eq!(Project::load(&path).unwrap().name, "Before save");
        app.undo();
        assert_eq!(app.project.name, "Before save");
    }

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

    #[test]
    fn render_job_export_snapshot_survives_save_as_and_track_reordering() {
        let directory = tempfile::tempdir().unwrap();
        let old_base = directory.path().join("old");
        let new_base = directory.path().join("new");
        let exports = directory.path().join("exports");
        std::fs::create_dir(&old_base).unwrap();
        std::fs::create_dir(&new_base).unwrap();
        std::fs::create_dir(&exports).unwrap();
        let patch_path = old_base.join("shared.json");
        std::fs::write(
            &patch_path,
            evil_synth::Patch::default().to_json_pretty().unwrap(),
        )
        .unwrap();

        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project_path = Some(old_base.join("song.evil"));
        app.project.instruments[0].source = InstrumentSource::Synth {
            patch: "shared.json".to_owned(),
        };
        app.project.tracks[0].name = "First".to_owned();
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        app.project
            .add_track(Track {
                name: "Second".to_owned(),
                notes: vec![evil_project::Note {
                    pitch: 72,
                    ..evil_project::Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let expected_project = app.project.clone();
        for (track_index, name) in [(0, "first"), (1, "second")] {
            let expected = evil_render::render_project_scoped(
                &expected_project,
                &old_base,
                evil_render::RenderScope::Track { track_index },
            )
            .unwrap();
            expected
                .sound
                .write_wav(directory.path().join(format!("expected-{name}.wav")))
                .unwrap();
        }

        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        app.start_wav_export_plan(exports.clone(), plan);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.instrument_resolution_in_progress() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!app.instrument_resolution_in_progress());
        std::fs::remove_file(patch_path).unwrap();
        app.save_to(new_base.join("song.evil"));
        app.project.move_track(0, 1).unwrap();
        app.project.tracks[0].notes[0].pitch = 80;
        app.project.tracks[1].notes[0].pitch = 84;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(
            app.render_job.is_none(),
            "export did not finish: {}",
            app.status
        );
        for (file_name, name) in [("01-first.wav", "first"), ("02-second.wav", "second")] {
            assert_eq!(
                std::fs::read(exports.join(file_name)).unwrap(),
                std::fs::read(directory.path().join(format!("expected-{name}.wav"))).unwrap()
            );
        }
    }

    #[test]
    fn render_job_stop_then_play_queues_until_the_cancelled_worker_drains() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.show_about = false;
        app.project.tracks[0].notes = vec![evil_project::Note {
            duration_ticks: 50_000,
            ..evil_project::Note::default()
        }];
        let owner = app.render_owner.clone();
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let first_worker = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        let gate_for_hook = gate.clone();
        let first_worker_for_hook = first_worker.clone();
        owner.set_worker_test_hook(move || {
            if !first_worker_for_hook.swap(true, std::sync::atomic::Ordering::AcqRel) {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        press_space(&context, &mut app);
        entered_receiver.recv().unwrap();
        assert_eq!(owner.worker_count(), 1);
        press_space(&context, &mut app);
        app.project.tracks[0].notes[0].duration_ticks = 480;
        press_space(&context, &mut app);
        assert!(app.queued_render.is_some());
        assert_eq!(owner.worker_count(), 1);
        assert!(app.status.contains("Cancelling"));

        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while (app.render_job.is_some()
            || app.draining_render.is_some()
            || app.queued_render.is_some())
            && std::time::Instant::now() < deadline
        {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(app.queued_render.is_none());
        assert!(app.render_job.is_none());
        assert!(app.draining_render.is_none());
        assert_eq!(owner.worker_count(), 0);
        assert!(!app.status.contains("Already rendering"));

        app.project.tracks[0].notes[0].duration_ticks = 50_000;
        app.preview_project();
        drop(app);
        let retirement_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while owner.worker_count() != 0 && std::time::Instant::now() < retirement_deadline {
            std::thread::yield_now();
        }
        assert_eq!(owner.worker_count(), 0);
    }

    #[test]
    fn render_job_cancel_drain_capacity_is_one_active_plus_one_draining() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.show_about = false;
        app.project.tracks[0].notes = vec![evil_project::Note {
            duration_ticks: 50_000,
            ..evil_project::Note::default()
        }];
        for _ in 0..15 {
            app.project
                .add_track(Track {
                    notes: vec![evil_project::Note {
                        duration_ticks: 50_000,
                        ..evil_project::Note::default()
                    }],
                    ..Track::default()
                })
                .unwrap();
        }
        let owner = app.render_owner.clone();
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let first_result = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        let gate_for_hook = gate.clone();
        let first_result_for_hook = first_result.clone();
        owner.set_result_test_hook(move || {
            if !first_result_for_hook.swap(true, std::sync::atomic::Ordering::AcqRel) {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        press_space(&context, &mut app);
        press_space(&context, &mut app);
        press_space(&context, &mut app);
        assert!(app.queued_render.is_some());
        entered_receiver.recv().unwrap();
        app.render_job.as_ref().unwrap().expire_cancellation_grace();
        app.poll_render();
        assert!(app.render_job.is_some());
        assert!(app.draining_render.is_some());
        assert!(owner.worker_count() <= 2);
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();

        for track in &mut app.project.tracks {
            track.notes[0].duration_ticks = 480;
        }
        press_space(&context, &mut app);
        press_space(&context, &mut app);
        assert!(app.queued_render.is_some());
        assert!(owner.worker_count() <= 2);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while (app.render_job.is_some()
            || app.draining_render.is_some()
            || app.queued_render.is_some())
            && std::time::Instant::now() < deadline
        {
            app.poll_render();
            assert!(owner.worker_count() <= 2);
            std::thread::yield_now();
        }
        assert!(app.queued_render.is_none());
        assert_eq!(
            owner.worker_count(),
            0,
            "status={} active={} draining={} queued={}",
            app.status,
            app.render_job.is_some(),
            app.draining_render.is_some(),
            app.queued_render.is_some()
        );
    }

    #[test]
    fn render_job_cancelled_publisher_detaches_and_queued_preview_starts() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("publisher.wav");
        std::fs::write(&output, b"original").unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note {
            duration_ticks: 50_000,
            ..evil_project::Note::default()
        }];
        let owner = app.render_owner.clone();
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        owner.set_publication_test_hook(&output, move |_, point| {
            if point == crate::render_job::PublicationTestPoint::BeforeWrite {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        app.start_render(PendingRender::Wav(output));
        entered_receiver.recv().unwrap();
        app.cancel_active_render();
        app.preview_project();
        app.render_job.as_ref().unwrap().expire_cancellation_grace();

        app.poll_render();

        assert!(app.render_job.is_some());
        assert!(app.draining_render.is_some());
        assert!(owner.worker_count() <= 2);
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while (app.render_job.is_some() || app.draining_render.is_some())
            && std::time::Instant::now() < deadline
        {
            app.poll_render();
            std::thread::yield_now();
        }
        assert_eq!(owner.worker_count(), 0);
    }

    #[test]
    fn render_job_toolbar_restart_preserves_a_recorder_created_while_queued() {
        let tracks = vec![
            vec![evil_project::Note {
                duration_ticks: 50_000,
                ..evil_project::Note::default()
            }];
            16
        ];
        let track_notes: Vec<_> = tracks.iter().map(Vec::as_slice).collect();
        let project = test_harness::project_with(&track_notes);
        let mut harness = test_harness::Harness::new(project);
        let labels: Vec<_> = harness
            .nodes()
            .into_iter()
            .filter_map(|node| node.label().map(str::to_owned))
            .collect();
        assert!(
            labels.iter().any(|label| label == "PLAY"),
            "available controls: {labels:?}"
        );
        let play_node = harness.node("PLAY").0;
        harness.action(play_node, eframe::egui::accesskit::Action::Click, None);
        let owner = harness.app.render_owner.clone();
        assert_eq!(owner.worker_count(), 1);
        harness.run(vec![]);
        assert!(harness.app.render_job.is_some(), "{}", harness.app.status);
        harness.toolbar_click_without_polling(play_node);
        assert!(harness.app.render_is_cancelling());
        harness.toolbar_click_without_polling(play_node);
        assert!(
            harness.app.queued_render.is_some(),
            "status={} cancelling={} workers={}",
            harness.app.status,
            harness.app.render_is_cancelling(),
            owner.worker_count()
        );

        for track in &mut harness.app.project.tracks {
            track.notes[0].duration_ticks = 480;
        }
        harness.app.toggle_recording();
        assert!(harness.app.recorder.is_some());
        assert!(harness.app.queued_render.is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while (harness.app.render_job.is_some() || harness.app.queued_render.is_some())
            && std::time::Instant::now() < deadline
        {
            harness.run(vec![]);
        }
        assert!(harness.app.recorder.is_some());
        assert_eq!(owner.worker_count(), 0);
    }

    #[test]
    fn render_job_file_export_teardown_requires_explicit_cancel_and_reports_progress() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note {
            duration_ticks: 50_000,
            ..evil_project::Note::default()
        }];
        app.project
            .add_track(Track {
                name: "Second".to_owned(),
                notes: vec![evil_project::Note {
                    duration_ticks: 50_000,
                    ..evil_project::Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        app.start_wav_export_plan(directory.path().to_path_buf(), plan);
        app.stop_preview();
        assert!(!app.render_is_cancelling());
        app.preview_project();
        assert!(!app.render_is_cancelling());
        assert!(app.queued_render.is_none());
        assert!(app.status.contains("WAV export in progress"));

        app.cancel_active_render();
        assert_eq!(
            app.transport_state().playing,
            crate::speech::PlayingState::Cancelling
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(app.status.contains("0 of 2"), "{}", app.status);
        assert!(app.status.contains("01-track-1.wav"), "{}", app.status);
        assert_eq!(
            app.transport_state().playing,
            crate::speech::PlayingState::Stopped
        );
    }

    #[test]
    fn render_job_file_export_refusal_does_not_commit_active_take() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note {
            duration_ticks: 50_000,
            ..evil_project::Note::default()
        }];
        app.start_render(PendingRender::Wav(directory.path().join("active.wav")));
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(60, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.preview_project();
        assert!(app.recorder.is_some());
        assert!(app.status.contains("WAV export in progress"));
    }

    #[test]
    fn render_job_queued_actions_are_withdrawn_by_stop_and_document_or_transport_changes() {
        let directory = tempfile::tempdir().unwrap();
        let project_path = directory.path().join("open.evil");
        Project::new("Open").save(&project_path).unwrap();
        let mut midi_project = Project::new("MIDI");
        midi_project
            .add_track(Track {
                notes: vec![evil_project::Note::default()],
                ..Track::default()
            })
            .unwrap();
        let midi_path = directory.path().join("import.mid");
        evil_exports::export_native_project_to_path(&midi_project, directory.path(), &midi_path)
            .unwrap();

        for operation in ["new", "transport", "open", "midi"] {
            let mut app = app_with_queued_render();
            app.recording_commit_status = Some("old recording".to_owned());
            match operation {
                "new" => app.new_project(),
                "transport" => app.stop_preview(),
                "open" => app.open_project_path(&project_path),
                "midi" => app.import_midi_path(&midi_path),
                _ => unreachable!(),
            }
            assert!(app.queued_render.is_none(), "{operation}");
            assert!(
                app.render_job.is_some(),
                "{operation} retained cleanup only"
            );
            app.poll_render();
            assert!(!app.status.contains("old recording"), "{operation}");
        }

        let context = egui::Context::default();
        let mut app = app_with_queued_render();
        press_space(&context, &mut app);
        assert!(app.queued_render.is_none());
        assert_eq!(app.status, "Stopped");
    }

    #[test]
    fn render_job_queued_wav_withdrawal_reports_output_and_commits_recording() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("queued.wav");
        let mut app = app_with_queued_render();
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(60, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.start_render(PendingRender::Wav(output.clone()));
        assert!(app.recorder.is_none());
        app.stop_transport();
        assert!(app.queued_render.is_none());
        assert!(app.status.contains("Withdrew queued WAV export"));
        assert!(app.status.contains("queued.wav"));
        assert_eq!(app.project.tracks[0].notes.len(), 1);
    }

    #[test]
    fn render_job_queued_file_refuses_preview_but_recording_continues_independently() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("queued-refusal.wav");
        let mut app = app_with_queued_render();
        let accepted = app.accept_render(PendingRender::Wav(output));
        app.queued_render = Some(accepted);

        app.preview_project();
        assert!(app.queued_render.is_some());
        assert!(app.status.contains("WAV export in progress"));

        app.toggle_recording();
        assert!(app.queued_render.is_some());
        assert!(app.recorder.is_some());
        assert!(app.status.contains("independent recording clock"));
    }

    #[test]
    fn render_job_document_replacement_withdraws_queued_wav() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("document.wav");
        let mut app = app_with_queued_render();
        let accepted = app.accept_render(PendingRender::Wav(output));
        app.queued_render = Some(accepted);
        assert!(app.instrument_resolution_in_progress());
        app.new_project();
        assert!(app.queued_render.is_none());
        assert!(!app.instrument_resolution_in_progress());
        assert!(!app.status.contains("requested action queued"));
    }

    #[test]
    fn render_job_failed_document_open_preserves_queued_wav_and_current_document() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = app_with_queued_render();
        let accepted = app.accept_render(PendingRender::Wav(directory.path().join("queued.wav")));
        app.queued_render = Some(accepted);
        app.completed_cancelled_status = Some("Published old-export.wav".to_owned());
        let project = app.project.clone();

        app.open_project_path(&directory.path().join("missing.evil"));

        assert!(app.queued_render.is_some());
        assert_eq!(app.project, project);
        assert!(app.completed_cancelled_status.is_some());
        assert!(app.status.contains("Could not open missing.evil"));
    }

    #[test]
    fn render_job_incidental_loop_sound_and_undo_edits_preserve_queued_wav() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("queued-loop.wav");
        let mut project = test_harness::project_with(&[&[evil_project::Note::default()]]);
        let pattern = project
            .add_pattern(evil_project::Pattern {
                length_ticks: 480,
                notes: vec![evil_project::Note::default()],
                ..evil_project::Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                evil_project::Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 480,
                },
            )
            .unwrap();
        let mut harness = test_harness::Harness::new(project);
        harness.app.render_job = Some(RenderJob::cancelled_for_test());
        harness.app.preview_project();
        let accepted = harness
            .app
            .accept_render(PendingRender::Wav(output.clone()));
        harness.app.queued_render = Some(accepted);
        harness.app.completed_cancelled_status = Some("stale queued outcome".to_owned());
        harness.app.status = "Cancelling current render; requested action queued…".to_owned();
        harness.app.roll.select_part(0, Some(0));
        assert!(harness.app.instrument_resolution_in_progress());

        harness
            .app
            .run_song_command(crate::arranger::edits::SongCommand::LoopPart);

        assert!(harness.app.queued_render.is_some());
        assert!(harness.app.completed_cancelled_status.is_some());
        assert!(harness.app.instrument_resolution_in_progress());
        assert!(!harness.app.status.contains("Withdrew queued WAV"));
        harness.app.status = "Cancelling current render; requested action queued…".to_owned();
        harness.app.stop_preview();
        assert!(harness.app.status.contains("requested action queued"));

        harness.app.cycle_sound(false);
        assert!(harness.app.queued_render.is_some());
        assert!(harness.app.instrument_resolution_in_progress());

        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(64, 100, 0, &harness.app.project);
        harness.app.recorder = Some(recorder);
        harness.app.recording_started_at = Some(std::time::Instant::now());
        harness.app.undo();
        assert!(harness.app.queued_render.is_some());
        assert!(harness.app.instrument_resolution_in_progress());
    }

    #[test]
    fn render_job_file_export_commits_an_empty_before_take_project_before_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.project.tracks[0].notes.is_empty());
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(60, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.start_render(PendingRender::Wav(directory.path().join("take.wav")));
        assert!(app.recorder.is_none());
        assert_eq!(app.project.tracks[0].notes.len(), 1);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let rendered = loop {
            if let Some(result) = app.render_job.as_mut().and_then(RenderJob::try_finish) {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(rendered.note_count, 1);
        assert!(rendered.published);

        app.render_job = None;
        app.pending_render = None;
        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        app.project.tracks[0].notes.clear();
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(64, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.start_wav_export_plan(directory.path().to_path_buf(), plan);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let rendered = loop {
            if let Some(result) = app.render_job.as_mut().and_then(RenderJob::try_finish) {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(rendered.note_count, 1);
    }

    #[test]
    fn render_job_document_result_does_not_repeat_announced_export_outcome() {
        let directory = tempfile::tempdir().unwrap();
        let project_path = directory.path().join("opened.evil");
        Project::new("Opened").save(&project_path).unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        app.preview_project();
        app.completed_cancelled_status = Some("Published old-export.wav".to_owned());

        app.open_project_path(&project_path);

        assert!(app.completed_cancelled_status.is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(
            app.status.starts_with("Opened opened.evil"),
            "{}",
            app.status
        );
        assert!(!app.status.contains("old-export.wav"), "{}", app.status);
    }

    #[test]
    fn render_job_document_replacement_clears_status_with_only_draining_worker() {
        let directory = tempfile::tempdir().unwrap();
        let project_path = directory.path().join("opened.evil");
        Project::new("Opened").save(&project_path).unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.draining_render = Some(RenderJob::cancelled_for_test());
        app.completed_cancelled_status = Some("Published old-export.wav".to_owned());

        app.open_project_path(&project_path);

        assert!(app.completed_cancelled_status.is_none());
        assert!(app.status.starts_with("Opened opened.evil"));
    }

    #[test]
    fn render_job_space_and_toolbar_clear_completed_nonfile_outcomes() {
        for toolbar in [false, true] {
            let mut harness = test_harness::Harness::new(test_harness::project_with(&[&[
                evil_project::Note::default(),
            ]]));
            harness.app.preview_project();
            harness.app.completed_cancelled_status = Some("Published old-export.wav".to_owned());
            if toolbar {
                harness.run(vec![]);
                let cancel = harness.node("CANCEL").0;
                harness.toolbar_click_without_polling(cancel);
            } else {
                harness.run(vec![
                    test_harness::press(egui::Key::Space, egui::Modifiers::NONE),
                    test_harness::release(egui::Key::Space),
                ]);
            }
            assert!(
                harness.app.completed_cancelled_status.is_none(),
                "{toolbar}"
            );
        }
    }

    #[test]
    fn render_job_next_batch_stem_preserves_a_recorder_created_between_jobs() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        app.project
            .add_track(Track {
                name: "Second".to_owned(),
                notes: vec![evil_project::Note::default()],
                ..Track::default()
            })
            .unwrap();
        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        let first = directory.path().join(&plan.exports[0].file_name);
        app.start_wav_export_plan(directory.path().to_path_buf(), plan);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !first.exists() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(first.exists());
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(64, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            app.poll_render();
            if matches!(
                &app.pending_render,
                Some(PendingRender::WavExport { queue, .. }) if queue.completed == 1
            ) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "next stem did not start"
            );
            std::thread::yield_now();
        }
        assert!(app.recorder.is_some());
        app.cancel_active_render();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
    }

    #[test]
    fn render_job_deferred_preview_drain_preserves_unrelated_later_status() {
        for status in ["Opened later.evil, 1 track.", "Previewing a later render"] {
            let context = egui::Context::default();
            let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
            app.render_job = Some(RenderJob::cancelled_for_test());
            app.cancelled_render = Some(PendingRender::Preview {
                start_tick: 0,
                end_tick: None,
                repeat: false,
            });
            app.status = status.to_owned();
            app.poll_render();
            assert_eq!(app.status, status);
            assert!(app.completed_cancelled_status.is_none());
        }
    }

    #[test]
    fn render_job_empty_project_take_then_immediate_play_commits_before_preview() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.project.tracks[0].notes.is_empty());
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(60, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.preview_project();
        assert!(app.recorder.is_none());
        assert_eq!(app.project.tracks[0].notes.len(), 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let rendered = loop {
            if let Some(result) = app.render_job.as_mut().and_then(RenderJob::try_finish) {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(rendered.note_count, 1);
    }

    #[test]
    fn render_job_resample_commits_active_take_before_snapshot() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(60, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.start_render(PendingRender::Resample {
            track_index: 0,
            start_tick: 0,
            end_tick: Some(480),
        });
        assert!(app.recorder.is_none());
        assert_eq!(app.project.tracks[0].notes.len(), 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let rendered = loop {
            if let Some(result) = app.render_job.as_mut().and_then(RenderJob::try_finish) {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(rendered.note_count, 1);
    }

    #[test]
    fn render_job_space_ends_take_without_cancelling_file_export() {
        for recording in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("transport.wav");
            let context = egui::Context::default();
            let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
            app.project.tracks[0].notes = vec![evil_project::Note::default()];
            app.start_render(PendingRender::Wav(output.clone()));
            if recording {
                let mut recorder = crate::recording::Recorder::new(
                    crate::recording::RecordingTarget::Track(0),
                    0,
                    1_920,
                    None,
                );
                recorder.hit(64, 100, 0, &app.project);
                app.recorder = Some(recorder);
                app.recording_started_at = Some(std::time::Instant::now());
            }

            press_space(&context, &mut app);

            assert!(app.render_job.is_some());
            assert!(!app.render_is_cancelling());
            assert!(app.recorder.is_none());
            if recording {
                assert!(app.status.contains("Recorded 1 notes"), "{}", app.status);
            } else {
                assert!(app.status.contains("export continues"), "{}", app.status);
                assert!(app.status.contains("explicit Cancel"), "{}", app.status);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while app.render_job.is_some() && std::time::Instant::now() < deadline {
                app.poll_render();
                std::thread::yield_now();
            }
            assert!(output.exists());
            assert_eq!(
                app.project.tracks[0].notes.len(),
                if recording { 2 } else { 1 }
            );
        }
    }

    #[test]
    fn render_job_stop_preserves_recording_commit_status() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        app.preview_project();
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(64, 100, 0, &app.project);
        app.recorder = Some(recorder);
        app.recording_started_at = Some(std::time::Instant::now());
        app.finish_recording();
        assert!(app.status.contains("Recorded 1 notes"));
        app.cancel_active_render();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(app.status.contains("Recorded 1 notes"), "{}", app.status);
    }

    #[test]
    fn render_job_two_cancelled_exports_keep_distinct_outcomes_in_both_orders() {
        for first_publishes in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let first = directory.path().join("first.wav");
            let second = directory.path().join("second.wav");
            let context = egui::Context::default();
            let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
            let gate =
                std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
            let first_worker = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
            let gate_for_hook = gate.clone();
            let first_worker_for_hook = first_worker.clone();
            app.render_owner.set_result_test_hook(move || {
                if !first_worker_for_hook.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    entered_sender.send(()).unwrap();
                    let (lock, condition) = &*gate_for_hook;
                    let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    while !*allowed {
                        allowed = condition
                            .wait(allowed)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                }
            });
            app.project.tracks[0].notes = vec![evil_project::Note {
                duration_ticks: if first_publishes { 480 } else { 50_000 },
                ..evil_project::Note::default()
            }];
            app.start_render(PendingRender::Wav(first.clone()));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            if first_publishes {
                while !first.exists() && std::time::Instant::now() < deadline {
                    std::thread::yield_now();
                }
                assert!(first.exists());
            }
            app.cancel_active_render();
            app.project.tracks[0].notes[0].duration_ticks =
                if first_publishes { 50_000 } else { 480 };
            app.start_render(PendingRender::Wav(second.clone()));
            entered_receiver.recv().unwrap();
            app.render_job.as_ref().unwrap().expire_cancellation_grace();
            app.poll_render();
            assert!(app.draining_render.is_some());

            if first_publishes {
                let (lock, condition) = &*gate;
                *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
                condition.notify_all();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while app.draining_render.is_some() && std::time::Instant::now() < deadline {
                    app.poll_render();
                    std::thread::yield_now();
                }
                assert!(app.draining_render.is_none());
                assert!(app.render_job.is_some());
                app.cancel_active_render();
            } else {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !second.exists() && std::time::Instant::now() < deadline {
                    std::thread::yield_now();
                }
                assert!(second.exists());
                app.cancel_active_render();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while app.render_job.is_some() && std::time::Instant::now() < deadline {
                    app.poll_render();
                    std::thread::yield_now();
                }
                assert!(app.render_job.is_none());
                assert!(app.draining_render.is_some());
                let (lock, condition) = &*gate;
                *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
                condition.notify_all();
            }

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while (app.render_job.is_some() || app.draining_render.is_some())
                && std::time::Instant::now() < deadline
            {
                app.poll_render();
                std::thread::yield_now();
            }
            assert!(app.status.contains("first.wav"), "{}", app.status);
            assert!(app.status.contains("second.wav"), "{}", app.status);
            assert_eq!(first.exists(), first_publishes);
            assert_eq!(second.exists(), !first_publishes);
            let (published, cancelled) = if first_publishes {
                (&first, &second)
            } else {
                (&second, &first)
            };
            let published_outcome = format!("Published WAV export to {}", published.display());
            let cancelled_outcome = format!("Cancelled WAV export to {}", cancelled.display());
            assert!(app.status.contains(&published_outcome), "{}", app.status);
            assert!(app.status.contains(&cancelled_outcome), "{}", app.status);
        }
    }

    #[test]
    fn render_job_fail_fast_resolution_names_source_not_valid_output() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        let missing = app
            .project
            .add_instrument(Instrument::synth("Missing", "missing.json"))
            .unwrap();
        app.project
            .add_track(Track {
                instrument: missing,
                notes: vec![evil_project::Note::default()],
                ..Track::default()
            })
            .unwrap();
        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        app.start_wav_export_plan(directory.path().to_path_buf(), plan);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.render_job.is_some() && std::time::Instant::now() < deadline {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(app.status.contains("before any output"), "{}", app.status);
        assert!(app.status.contains("missing.json"), "{}", app.status);
        assert!(!app.status.contains("01-track-1.wav"), "{}", app.status);
        assert!(!directory.path().join("01-track-1.wav").exists());
    }

    #[test]
    fn render_job_cancel_after_publication_counts_completed_file_and_cancels_only_remainder() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project.tracks[0].notes = vec![evil_project::Note::default()];
        app.project
            .add_track(Track {
                name: "Second".to_owned(),
                notes: vec![evil_project::Note {
                    duration_ticks: 50_000,
                    ..evil_project::Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let plan = evil_render::stem_export_plan(&app.project, None).unwrap();
        app.start_wav_export_plan(directory.path().to_path_buf(), plan);
        let first = directory.path().join("01-track-1.wav");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !first.exists() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(first.exists());
        app.cancel_active_render();
        app.stop_preview();
        app.preview_project();
        assert!(app.cancelled_render.is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while (app.render_job.is_some() || app.draining_render.is_some())
            && std::time::Instant::now() < deadline
        {
            app.poll_render();
            std::thread::yield_now();
        }
        assert!(app.status.contains("Published"), "{}", app.status);
        assert!(app.status.contains("1 of 2"), "{}", app.status);
        assert!(!directory.path().join("02-second.wav").exists());
    }

    #[test]
    fn render_job_resolution_blocks_shared_bank_shortcut_writes() {
        let directory = tempfile::tempdir().unwrap();
        let bank_path = directory.path().join("shared.evilkit");
        let bank = evil_beats::maxwell_kit().unwrap();
        std::fs::write(&bank_path, bank.to_json().unwrap()).unwrap();
        let original = std::fs::read(&bank_path).unwrap();
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project_path = Some(directory.path().join("song.evil"));
        app.project.instruments[0].source = InstrumentSource::DrumKit {
            kit: "shared.evilkit".to_owned(),
            base_note: 36,
        };
        let shared = app
            .project
            .add_instrument(Instrument::drum_kit("Also shared", "shared.evilkit"))
            .unwrap();
        app.project
            .add_track(Track {
                instrument: shared,
                notes: vec![evil_project::Note {
                    pitch: 36,
                    ..evil_project::Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let project = app.project.clone();
        app.view = View::Pads;
        app.render_job = Some(RenderJob::resolving_for_test());

        app.save_beats_bank();

        assert_eq!(std::fs::read(&bank_path).unwrap(), original);
        assert_eq!(app.project, project);
        assert!(app.status.contains("instrument resolution"));

        app.render_job = Some(RenderJob::cancelled_for_test());
        app.preview_project();
        assert!(app.queued_render.is_some());
        assert!(!app.render_job.as_ref().is_some_and(RenderJob::is_resolving));
        assert!(app.instrument_resolution_in_progress());
        app.save_beats_bank();
        assert_eq!(std::fs::read(&bank_path).unwrap(), original);
        assert_eq!(app.project, project);
        assert!(app.status.contains("Wait for render instrument resolution"));
    }

    #[test]
    fn render_job_subprocess_exit_cleanup() {
        const FIXTURE_DIRECTORY: &str = "MIDI_EVIL_EXIT_FIXTURE_DIRECTORY";
        if let Some(directory) = std::env::var_os(FIXTURE_DIRECTORY) {
            let context = egui::Context::default();
            let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
            app.project.tracks[0].notes = vec![evil_project::Note {
                duration_ticks: 50_000,
                ..evil_project::Note::default()
            }];
            let output = std::path::PathBuf::from(directory).join("shutdown.wav");
            app.start_render(PendingRender::Wav(output));
            app.shutdown_render_jobs();
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("app::tests::render_job_subprocess_exit_cleanup")
            .arg("--nocapture")
            .env(FIXTURE_DIRECTORY, directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let leftovers: Vec<_> = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(".midi-evil-"))
            })
            .collect();
        assert!(leftovers.is_empty(), "shutdown leftovers: {leftovers:?}");
    }
}
