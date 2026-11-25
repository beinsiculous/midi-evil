//! Undo and redo, and the selection that travels with each snapshot. Notes have no ids, so the
//! cursor cannot be found again by comparing notes after an undo; the snapshot carries it.

use super::*;
use crate::history::{Gesture, Snapshot};
use crate::piano_roll::edits::Outcome;
use crate::speech::{self, Origin};

/// The editor's selection as undo stores it beside a project.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct EditorSelection {
    track: usize,
    part: Option<usize>,
    cursor: Option<usize>,
}

/// The project and selection a frame began with. An edit made during the frame is recorded
/// against it, with the selection that belonged to that project, not the one the edit left.
pub(super) struct FrameStart {
    project: Project,
    selection: Option<EditorSelection>,
    /// Where the project's relative sources were resolved from when the frame began; a save in
    /// the frame moves them, and the frame's start is rebased before it is compared.
    base: Option<PathBuf>,
}

/// The hold that is going on, and which chord holds it.
pub(super) struct HeldGesture {
    gesture: Gesture,
    key: egui::Key,
    modifiers: egui::Modifiers,
}

impl From<Project> for FrameStart {
    fn from(project: Project) -> Self {
        Self {
            project,
            selection: None,
            base: None,
        }
    }
}

impl StudioApp {
    pub(super) fn editor_selection(&self) -> EditorSelection {
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        EditorSelection {
            track: self.roll.selected_track,
            part: self.roll.selected_part,
            cursor: self.roll.cursor.index(context),
        }
    }

    /// The selection as undo must store it: the cursor found again first, since a take or any
    /// other change the roll did not make may have re-sorted the notes under it.
    fn current_selection(&mut self) -> EditorSelection {
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        let edited = piano_roll::EditedSequence::new(&self.project, context);
        if let Some(sequence) = piano_roll::sequence_mut(&mut self.project, context) {
            piano_roll::reconcile(sequence, &mut self.roll, &edited, &mut self.announcer);
        }
        self.editor_selection()
    }

    pub(super) fn editor_snapshot(&mut self) -> Snapshot<EditorSelection> {
        Snapshot {
            selection: self.current_selection(),
            project: self.project.clone(),
        }
    }

    pub(super) fn begin_project_frame(&mut self) {
        let selection = self.current_selection();
        self.frame_start = Some(FrameStart {
            project: self.project.clone(),
            selection: Some(selection),
            base: Some(self.patch_base().to_path_buf()),
        });
    }

    /// A key's part in undo. A fresh press of one of the focused roll's repeating chords opens a
    /// gesture, whose edits the history keeps as one entry; only that chord's repeats continue
    /// it. Its release, a fresh press of anything, or a repeat of another chord ends it.
    pub(super) fn track_gesture(
        &mut self,
        key: egui::Key,
        modifiers: egui::Modifiers,
        pressed: bool,
        repeat: bool,
        roll_repeating_chord: bool,
    ) {
        let same_chord = self
            .held_gesture
            .as_ref()
            .is_some_and(|held| held.key == key && held.modifiers == modifiers);
        let ends_it = if pressed {
            !(repeat && same_chord)
        } else {
            self.held_gesture
                .as_ref()
                .is_some_and(|held| held.key == key)
        };
        if ends_it {
            self.end_gesture();
        }
        if pressed && !repeat && roll_repeating_chord {
            self.gestures_started += 1;
            self.held_gesture = Some(HeldGesture {
                gesture: Gesture::new(self.gestures_started),
                key,
                modifiers,
            });
        }
    }

    pub(super) fn end_gesture(&mut self) {
        self.held_gesture = None;
        self.history.end_gesture();
    }

    /// Records one roll key's change, made since `before`, under the open gesture: a repeat of
    /// the gesture pushes nothing, its entry already holding the state from before the hold.
    /// The frame's baseline moves past it.
    pub(super) fn record_key_edit(&mut self, before: Snapshot<EditorSelection>) {
        if before.project == self.project {
            return;
        }
        let gesture = self.held_gesture.as_ref().map(|held| held.gesture);
        self.history
            .record(before.project, before.selection, gesture);
        self.dirty = true;
        self.restart_project_frame();
    }

    fn restart_project_frame(&mut self) {
        if self.frame_start.is_some() {
            self.begin_project_frame();
        }
    }

    /// Records an edit made earlier in this frame, so that an undo arriving after it in the
    /// same frame undoes that edit and not the one before it.
    pub(super) fn record_pending_edit(&mut self) {
        if self.resetting_document {
            return;
        }
        let Some(mut start) = self.frame_start.take() else {
            return;
        };
        if self.saved_this_frame.is_some()
            && let Some(base) = &start.base
        {
            // Save As changes the reference base, not the musical edit history.
            rebase_project_sources(&mut start.project, base, self.patch_base());
        }
        if start.project != self.project {
            let selection = start.selection.unwrap_or_else(|| self.editor_selection());
            self.history.record(start.project, selection, None);
            self.dirty = true;
        }
        self.begin_project_frame();
    }

    pub(super) fn reset_history(&mut self) {
        // A rename in progress belonged to the document being replaced.
        self.track_name_before_edit = None;
        self.beats.discard_draft();
        self.history.clear();
        self.mixer.last_analysis = None;
        self.mixer.last_channel_meters.clear();
        self.resetting_document = true;
        self.saved_this_frame = None;
        self.document += 1;
    }

    pub(super) fn undo(&mut self) {
        self.record_pending_edit();
        let target = if self.recorder.is_some() {
            self.recording_snapshot
                .as_ref()
                .map(|snapshot| &snapshot.project)
        } else {
            self.history.undo_target()
        };
        let preserve_drafts = self.history_preserves_sound_drafts(target);
        if (self.history.can_undo() || self.recorder.is_some())
            && !preserve_drafts
            && !self.confirm_history_change()
        {
            return;
        }
        let recording_snapshot = self.recording_snapshot.clone();
        if self.recorder.is_some() {
            self.stop_preview();
            if let Some(snapshot) = recording_snapshot {
                self.history
                    .record(snapshot.project, snapshot.selection, None);
            }
        }
        let current = self.current_selection();
        if let Some(snapshot) = self.history.undo(&self.project, current) {
            self.restore_snapshot(snapshot, preserve_drafts, current);
            // After the focused editor's name, which the undo may have changed.
            self.report_after_the_name("Undid the last edit.");
        }
    }

    pub(super) fn redo(&mut self) {
        if self.recorder.is_some() {
            self.stop_preview();
            return;
        }
        self.record_pending_edit();
        let preserve_drafts = self.history_preserves_sound_drafts(self.history.redo_target());
        if self.history.can_redo() && !preserve_drafts && !self.confirm_history_change() {
            return;
        }
        let current = self.current_selection();
        if let Some(snapshot) = self.history.redo(&self.project, current) {
            self.restore_snapshot(snapshot, preserve_drafts, current);
            self.report_after_the_name("Redid the edit.");
        }
    }

    fn restore_snapshot(
        &mut self,
        snapshot: Snapshot<EditorSelection>,
        preserve_drafts: bool,
        left: EditorSelection,
    ) {
        if preserve_drafts {
            self.stop_keyboard();
            self.beats.stop();
        } else {
            self.clear_all_sound_drafts();
        }
        self.project = snapshot.project;
        // A rename in progress starts again from the name the restored project has.
        self.track_name_before_edit = self
            .track_name_before_edit
            .take()
            .and_then(|(track, _)| Some((track, self.project.tracks.get(track)?.name.clone())));
        self.invalidate_beats_link();
        // The workspace is where the person is; undo restores what was selected in it.
        let selection = snapshot.selection;
        self.roll.restore(
            &self.project,
            selection.track,
            selection.part,
            self.document,
            selection.cursor,
        );
        self.keep_view_with_sound_kind();
        let restored = self.editor_selection();
        if left.cursor.is_some()
            && restored.cursor.is_none()
            && let Some(text) = speech::describe_outcome(Outcome::SelectionReset, Origin::Song)
        {
            self.announcer.announce(text);
        }
        self.sync_sound_drafts();
        self.dirty = true;
        self.restart_project_frame();
    }

    /// Records what this frame changed, with the selection the frame began with. A test may
    /// pass a bare project, whose selection is then the current one.
    pub(super) fn finish_project_frame(
        &mut self,
        previous: impl Into<FrameStart>,
        previous_base: &Path,
    ) {
        let previous = previous.into();
        let saved = self.saved_this_frame.take();
        if self.resetting_document {
            self.resetting_document = false;
            return;
        }
        let mut previous_project = previous.project;
        if saved.is_some() {
            // Save As changes the reference base, not the musical edit history.
            let base = previous.base.as_deref().unwrap_or(previous_base);
            rebase_project_sources(&mut previous_project, base, self.patch_base());
        }
        if previous_project != self.project {
            let selection = previous
                .selection
                .unwrap_or_else(|| self.editor_selection());
            self.history.record(previous_project, selection, None);
            self.dirty = true;
        }
        if let Some(saved) = saved {
            self.dirty = self.project != saved;
        }
    }
}
