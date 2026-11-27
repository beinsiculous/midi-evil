//! The file commands — New, Open, Save, Save As, Import MIDI, Relink MIDI, Export MIDI and the
//! WAV renders. Each says what happened through the one report: what was opened or saved, what
//! was discarded, and what a cancelled dialog left as it was. The dialogs are the application's
//! one `Dialogs`, the desktop's own when it runs.

use super::*;
use crate::dialogs::FileDialog;

/// "song.evil": a file as it is spoken, without the folder it is in.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn track_count(project: &Project) -> String {
    match project.tracks.len() {
        1 => "1 track".to_owned(),
        count => format!("{count} tracks"),
    }
}

/// Whether a command may replace the project, and whether doing so discards anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Discard {
    NothingUnsaved,
    Discarded,
    Kept,
}

impl Discard {
    fn prefix(self) -> &'static str {
        match self {
            Self::Discarded => "Discarded the unsaved changes. ",
            Self::NothingUnsaved | Self::Kept => "",
        }
    }
}

impl StudioApp {
    /// Whether anything would be lost, an edit still in progress included: a held key must not
    /// let New or Open replace the project without asking.
    pub(super) fn unsaved_changes(&mut self) -> bool {
        self.record_pending_edit();
        self.dirty || self.recorder.is_some() || self.has_sound_drafts()
    }

    /// Asks, when anything is unsaved, whether to discard it.
    pub(super) fn discard_changes(&mut self) -> Discard {
        if !self.unsaved_changes() {
            return Discard::NothingUnsaved;
        }
        self.stop_keyboard();
        self.beats.stop();
        if self
            .dialogs
            .confirm("Unsaved changes", "Discard unsaved changes?")
        {
            Discard::Discarded
        } else {
            Discard::Kept
        }
    }

    pub(super) fn new_project(&mut self) {
        let discard = self.discard_changes();
        if discard == Discard::Kept {
            self.report("Kept the unsaved changes; no new project.");
            return;
        }
        self.stop_preview();
        self.clear_all_sound_drafts();
        self.project = Project::new("Untitled");
        self.imported_track_indices = None;
        self.show_about = false;
        self.project_path = None;
        self.source_midi = None;
        self.roll = RollState::default();
        self.transport = Transport::new(piano_roll::bar_ticks(&self.project));
        self.automation = AutomationState::default();
        self.invalidate_beats_link();
        let added = self.add_track_quietly();
        self.reset_history();
        self.dirty = false;
        self.report(match added {
            Ok(_) => format!(
                "{}New project, {}.",
                discard.prefix(),
                track_count(&self.project)
            ),
            Err(error) => format!(
                "{}New project; could not add a track: {error}.",
                discard.prefix()
            ),
        });
    }

    pub(super) fn open_project(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("MIDI Evil Projects", &["evil", "rmp"])
            .pick_file()
        else {
            self.report("Nothing opened.");
            return;
        };
        self.open_project_path(&path);
    }

    pub(super) fn open_project_path(&mut self, path: &Path) {
        let discard = self.discard_changes();
        if discard == Discard::Kept {
            self.report("Kept the unsaved changes; nothing opened.");
            return;
        }
        self.stop_preview();
        match Project::load(path) {
            Ok(project) => self.adopt_project(project, Some(path), &file_name(path), discard),
            Err(error) => self.report(format!("Could not open {}: {error}.", file_name(path))),
        }
    }

    /// Makes an opened project the document, saved to the file at `path` when it came from one,
    /// and says so by `name`.
    pub(super) fn adopt_project(
        &mut self,
        project: Project,
        path: Option<&Path>,
        name: &str,
        discard: Discard,
    ) {
        self.clear_all_sound_drafts();
        self.project = project;
        self.imported_track_indices = None;
        self.show_about = false;
        self.source_midi = self.project.source_midi.as_ref().map(PathBuf::from);
        self.roll = RollState::default();
        self.transport = Transport::new(piano_roll::bar_ticks(&self.project));
        self.automation = AutomationState::default();
        self.invalidate_beats_link();
        self.project_path = path.map(Path::to_path_buf);
        self.reset_history();
        self.dirty = false;
        self.report(format!(
            "{}Opened {name}, {}.",
            discard.prefix(),
            track_count(&self.project)
        ));
    }

    pub(super) fn save_project(&mut self) {
        let Some(path) = self.project_path.clone() else {
            self.save_project_as();
            return;
        };
        self.save_to(path);
    }

    pub(super) fn save_project_as(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("MIDI Evil Projects", &["evil", "rmp"])
            .set_file_name("Untitled.evil")
            .save_file()
        else {
            self.report("Not saved.");
            return;
        };
        self.source_midi = None;
        self.project.source_midi = None;
        self.save_to(path);
    }

    pub(super) fn save_to(&mut self, path: PathBuf) {
        let midi_output = match &self.source_midi {
            Some(midi_path) => {
                match evil_exports::export_native_project(&self.project, self.patch_base()) {
                    Ok(report) => Some((midi_path.clone(), report)),
                    Err(error) => {
                        self.report(format!("Not saved: could not export MIDI: {error}."));
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
        let old_base = self.patch_base().to_path_buf();
        let new_base = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut saved_project = self.project.clone();
        rebase_project_sources(&mut saved_project, &old_base, new_base);
        if let Err(error) = saved_project.save(&path) {
            self.report(format!("Could not save {}: {error}.", file_name(&path)));
            return;
        }
        if let Some((midi_path, report)) = midi_output {
            if let Err(error) = evil_project::atomic_write(&midi_path, &report.data) {
                self.report(format!(
                    "Saved {}, but could not save the MIDI file {}: {error}.",
                    file_name(&path),
                    file_name(&midi_path)
                ));
                return;
            }
            self.report(automation::midi_export_status(
                format!(
                    "Saved {} and exported {} notes to {}.",
                    file_name(&path),
                    report.note_count,
                    file_name(&midi_path)
                ),
                report.native_only_mixer_automation_events,
            ));
        } else {
            self.report(format!("Saved {}.", file_name(&path)));
        }
        self.history.rebase_instruments(&old_base, new_base);
        self.project = saved_project;
        self.saved_this_frame = Some(self.project.clone());
        self.stop_keyboard();
        self.beats_binding = None;
        self.project_path = Some(path);
        self.dirty = false;
    }

    pub(super) fn relink_midi(&mut self) {
        if self.source_midi.is_none() {
            self.report("This project has no linked MIDI file.");
            return;
        }
        let Some(path) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid", "midi"])
            .pick_file()
        else {
            self.report("The MIDI link is unchanged.");
            return;
        };
        self.source_midi = Some(path.clone());
        self.project.source_midi = Some(path.to_string_lossy().into_owned());
        self.dirty = true;
        self.report(format!("Relinked MIDI to {}.", file_name(&path)));
    }

    pub(super) fn export_midi(&mut self) {
        let Some(output) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid"])
            .set_file_name("Untitled.mid")
            .save_file()
        else {
            self.report("Nothing exported.");
            return;
        };
        self.export_midi_to(&output);
    }

    pub(super) fn export_midi_to(&mut self, output: &Path) {
        match evil_exports::export_native_project_to_path(&self.project, self.patch_base(), output)
        {
            Ok(report) => self.report(automation::midi_export_status(
                format!(
                    "Exported {} notes from {} tracks to {}.",
                    report.note_count,
                    report.track_count,
                    file_name(output)
                ),
                report.native_only_mixer_automation_events,
            )),
            Err(error) => self.report(format!("Could not export MIDI: {error}.")),
        }
    }

    pub(super) fn import_midi(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("Standard MIDI File", &["mid", "midi"])
            .pick_file()
        else {
            self.report("Nothing imported.");
            return;
        };
        self.import_midi_path(&path);
    }

    pub(super) fn import_midi_path(&mut self, path: &Path) {
        let discard = self.discard_changes();
        if discard == Discard::Kept {
            self.report("Kept the unsaved changes; nothing imported.");
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
                self.clear_all_sound_drafts();
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
                let preserved = |count: usize, what: &str| {
                    if count > 0 {
                        format!("; {count} {what}")
                    } else {
                        String::new()
                    }
                };
                self.report(format!(
                    "{}Imported {}, {}, {} notes{}{}{}{}.",
                    discard.prefix(),
                    file_name(path),
                    track_count(&self.project),
                    self.project
                        .tracks
                        .iter()
                        .map(|track| track.notes.len())
                        .sum::<usize>(),
                    preserved(bends, "pitch-bend events preserved"),
                    preserved(pressure, "aftertouch events preserved"),
                    preserved(controllers, "controller events preserved"),
                    preserved(skipped, "controller or automation events not rendered"),
                ));
            }
            Err(error) => self.report(format!("Could not import {}: {error}.", file_name(path))),
        }
    }

    pub(super) fn file_dialog(&mut self) -> FileDialog<'_> {
        // Native dialogs block UI note-off processing until they close.
        self.stop_keyboard();
        self.beats.stop();
        let dialog = FileDialog::new(self.dialogs.as_mut());
        match &self.project_base {
            Some(project) => dialog.set_directory(project),
            None => dialog,
        }
    }

    fn wav_output(&mut self, default_name: &str) -> Option<PathBuf> {
        let output = self
            .file_dialog()
            .add_filter("WAV audio", &["wav"])
            .set_file_name(default_name)
            .save_file();
        if output.is_none() {
            self.report("Nothing rendered.");
        }
        output
    }

    pub(super) fn render_audio(&mut self) {
        if let Some(output) = self.wav_output("Untitled.wav") {
            self.start_render(PendingRender::Wav(output));
        }
    }

    pub(super) fn render_section_audio(&mut self) {
        let Some(output) = self.wav_output("Section.wav") else {
            return;
        };
        let (start_tick, end_tick) = self.transport.range();
        self.start_render(PendingRender::RangeWav {
            output,
            start_tick,
            end_tick,
        });
    }

    pub(super) fn render_track_stems(&mut self) {
        let Some(output_directory) = self.file_dialog().pick_folder() else {
            self.report("Nothing rendered.");
            return;
        };
        match evil_render::stem_export_plan(&self.project, None) {
            Ok(plan) => self.start_wav_export_plan(output_directory, plan),
            Err(error) => self.report(format!("Could not plan the track stems: {error}.")),
        }
    }

    pub(super) fn render_arrangement_parts(&mut self) {
        let Some(output_directory) = self.file_dialog().pick_folder() else {
            self.report("Nothing rendered.");
            return;
        };
        match evil_render::part_export_plan(&self.project, None, None) {
            Ok(plan) => self.start_wav_export_plan(output_directory, plan),
            Err(error) => self.report(format!("Could not plan the arrangement parts: {error}.")),
        }
    }

    pub(super) fn start_wav_export_plan(
        &mut self,
        output_directory: PathBuf,
        plan: evil_render::ExportPlan,
    ) {
        let mut queue = WavExportQueue::new(output_directory, plan);
        if queue.is_empty() {
            self.report("No track or arrangement part with notes to export.");
            return;
        }
        let Some(current) = queue.next_spec() else {
            return;
        };
        self.start_render(PendingRender::WavExport { current, queue });
    }
}
