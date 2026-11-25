use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText, TextureHandle};

mod chrome;
#[cfg(test)]
mod command_outcome_tests;
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
use crate::history::{EditorHistory, Snapshot};
use crate::instance::SingleInstance;
use crate::instruments::{portable_file_reference, rebase_project_sources};
use crate::launch::LaunchOptions;
use crate::patch_lab::{self, PatchLab};
use crate::piano_roll::{self, RollState};
use crate::preview::PreviewPlayer;
use crate::regions::{self, Region};
use crate::render_job::{RenderJob, WavExportQueue};
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
    ) -> Self {
        Self::create(&context.egui_ctx, context.storage, options, instance)
    }

    fn create(
        context: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        options: LaunchOptions,
        instance: Option<SingleInstance>,
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
            pending_render: None,
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
        };
        if let Err(error) = app.add_track_quietly() {
            app.status = format!("Could not add a track: {error}");
        }
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
                    .add_enabled(!comparing, egui::Button::new("Save & assign…"))
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
                        patch_lab::show(ui, &mut lab)
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

    fn start_render(&mut self, action: PendingRender) {
        if self.render_job.is_some() {
            self.report("Already rendering; cancel it before starting another.");
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
            PendingRender::Wav(output) => RenderJob::start_wav(
                self.project.clone(),
                self.patch_base().to_path_buf(),
                output.clone(),
            ),
            PendingRender::RangeWav {
                output,
                start_tick,
                end_tick,
            } => RenderJob::start_range_wav(
                self.project.clone(),
                self.patch_base().to_path_buf(),
                *start_tick,
                *end_tick,
                output.clone(),
            ),
            PendingRender::WavExport { current, queue } => RenderJob::start_scoped_wav(
                self.project.clone(),
                self.patch_base().to_path_buf(),
                current.scope,
                queue.output_for(current),
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
                    rendered.unsupported_mixer_automation_events,
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
                    self.start_render(PendingRender::WavExport { current, queue });
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
                self.report(format!(
                    "Could not export {} after completing {} of {} file(s): {error}",
                    current.file_name, queue.completed, queue.total
                ));
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
                let path = (!matches!(source.as_str(), "maxwell" | "chiptune"))
                    .then(|| base.join(&source));
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
        if let Some(path) = self.recovery.show(root.ctx()) {
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
                let hits = self.beats.show(ui, self.project.tempo_bpm);
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
}
