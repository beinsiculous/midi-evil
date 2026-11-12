use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct SoundDrafts {
    synths: HashMap<usize, PatchLab>,
    kits: HashMap<usize, crate::beats::BeatsDraft>,
    synth_owner: Option<usize>,
    kit_owner: Option<usize>,
}

impl StudioApp {
    /// Instruments keep their indices when tracks move; drafts follow the instrument.
    pub(super) fn sync_sound_drafts(&mut self) {
        let track_index = self.roll.selected_track;
        let selected = self
            .project
            .tracks
            .get(track_index)
            .map(|track| track.instrument);
        let synth = selected.filter(|index| {
            matches!(
                self.project.instruments[*index].source,
                InstrumentSource::Synth { .. }
            )
        });
        let kit = selected.filter(|index| {
            matches!(
                self.project.instruments[*index].source,
                InstrumentSource::DrumKit { .. }
            )
        });
        if self.drafts.synth_owner != synth {
            self.stop_keyboard();
            if let Some(lab) = self.patch_lab.take()
                && let Some(owner) = self.drafts.synth_owner
                && lab.has_unsaved_changes()
            {
                self.drafts.synths.insert(owner, lab);
            }
            self.patch_lab = synth.and_then(|owner| self.drafts.synths.remove(&owner));
            self.drafts.synth_owner = synth;
        }
        if let Some(lab) = &mut self.patch_lab {
            lab.track_index = track_index;
        }
        if self.drafts.kit_owner != kit {
            if let Some(owner) = self.drafts.kit_owner
                && let Some(draft) = self.beats.take_draft()
            {
                self.drafts.kits.insert(owner, draft);
            }
            self.invalidate_beats_link();
            self.beats.discard_draft();
            if let Some(owner) = kit
                && let Some(draft) = self.drafts.kits.remove(&owner)
            {
                self.beats.restore_draft(draft, track_index);
                if let InstrumentSource::DrumKit { kit, .. } =
                    &self.project.instruments[owner].source
                {
                    self.beats_binding = Some((track_index, kit.clone()));
                }
            }
            self.drafts.kit_owner = kit;
        }
        if kit.is_some() {
            self.beats.relink_track(track_index);
        }
    }

    pub(super) fn has_sound_drafts(&self) -> bool {
        self.patch_lab
            .as_ref()
            .is_some_and(PatchLab::has_unsaved_changes)
            || self.beats.has_unsaved_changes()
            || !self.drafts.synths.is_empty()
            || !self.drafts.kits.is_empty()
    }

    pub(super) fn clear_all_sound_drafts(&mut self) {
        self.discard_sound_drafts();
        self.drafts = SoundDrafts::default();
    }

    pub(super) fn confirm_history_change(&mut self) -> bool {
        if !self.has_sound_drafts() {
            return true;
        }
        self.stop_keyboard();
        self.beats.stop();
        rfd::MessageDialog::new()
            .set_title("Unsaved instrument drafts")
            .set_description("This undo or redo changes the project's instruments. Discard unsaved instrument drafts? Save them first to keep your edits.")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show() == rfd::MessageDialogResult::Yes
    }

    pub(super) fn history_preserves_sound_drafts(&self, target: Option<&Project>) -> bool {
        target.is_some_and(|project| project.instruments == self.project.instruments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_undo_and_redo_preserve_active_and_parked_sound_drafts() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.gain = 0.123;
        app.add_track();
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.gain = 0.456;
        app.history.record(app.project.clone());
        app.project.tracks[1]
            .notes
            .push(evil_project::Note::default());
        app.undo();
        assert!(app.project.tracks[1].notes.is_empty());
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.456);
        app.redo();
        assert_eq!(app.project.tracks[1].notes.len(), 1);
        app.select_sound_track(0);
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
        let mut changed_sound = app.project.clone();
        changed_sound.instruments[0].source = InstrumentSource::Synth {
            patch: "bass".into(),
        };
        assert!(!app.history_preserves_sound_drafts(Some(&changed_sound)));
    }

    #[test]
    fn undo_during_the_first_recording_preserves_the_unsaved_patch() {
        use crate::recording::{Recorder, RecordingTarget};

        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.gain = 0.123;
        let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 1920, None);
        recorder.hit(60, 100, 0, &app.project);
        app.recording_snapshot = Some(app.project.clone());
        app.recorder = Some(recorder);
        assert!(!app.history.can_undo());
        app.undo();
        assert!(app.recorder.is_none());
        assert!(app.project.tracks[0].notes.is_empty());
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
        assert!(app.patch_lab.as_ref().unwrap().has_unsaved_changes());
        app.redo();
        assert_eq!(app.project.tracks[0].notes.len(), 1);
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
    }

    #[test]
    fn synth_drafts_follow_instruments_across_navigation_and_reordering() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.gain = 0.123;
        app.add_track();
        assert!(app.open_patch_lab());
        app.patch_lab.as_mut().unwrap().patch.gain = 0.456;
        app.select_sound_track(0);
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
        app.move_selected_track(1);
        assert_eq!(app.roll.selected_track, 1);
        assert_eq!(app.patch_lab.as_ref().unwrap().track_index, 1);
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
        app.select_sound_track(0);
        assert_eq!(app.patch_lab.as_ref().unwrap().patch.gain, 0.456);
        assert!(app.has_sound_drafts());
        app.clear_all_sound_drafts();
        assert!(!app.has_sound_drafts());
    }

    #[test]
    fn adding_a_synth_from_pads_changes_the_workspace() {
        let context = egui::Context::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.add_drum_track();
        assert!(app.view == View::Pads);
        app.add_track();
        assert!(app.view == View::Synth);
        assert!(app.open_patch_lab());
    }
}
