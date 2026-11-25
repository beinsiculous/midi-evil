//! The track list's commands — add a synth or a drum track, duplicate, move, delete, rename and
//! mute — each refused at the command boundary during a take and each reporting what it did.

use super::*;
use crate::speech;

impl StudioApp {
    pub(super) fn remove_selected_track(&mut self) {
        if !self.can_replace_sound() {
            return;
        }
        let index = self.roll.selected_track;
        if let Ok(removed) = self.project.remove_track(index) {
            self.invalidate_beats_link();
            self.automation.clear_selection();
            let new_index = if self.project.tracks.is_empty() {
                0
            } else {
                index.min(self.project.tracks.len() - 1)
            };
            self.discard_sound_drafts();
            self.roll.select_track(new_index);
            self.dirty = true;
            self.report(speech::track_removed(
                &removed.name,
                self.project.tracks.len(),
            ));
        }
    }

    pub(super) fn duplicate_selected_track(&mut self) {
        if !self.structural_edit_allowed() {
            return;
        }
        match self.project.duplicate_track(self.roll.selected_track) {
            Ok(index) => {
                self.roll.select_track(index);
                self.sync_sound_drafts();
                self.dirty = true;
                self.report(format!(
                    "Duplicated as {}, track {} of {}.",
                    self.project.tracks[index].name,
                    index + 1,
                    self.project.tracks.len()
                ));
            }
            Err(error) => self.report(format!("Could not duplicate the track: {error}.")),
        }
    }

    pub(super) fn move_selected_track(&mut self, offset: isize) {
        if !self.structural_edit_allowed() {
            return;
        }
        let index = self.roll.selected_track;
        let new_index = index as isize + offset;
        if new_index < 0 || new_index >= self.project.tracks.len() as isize {
            self.report(if offset < 0 {
                "Already the first track."
            } else {
                "Already the last track."
            });
            return;
        }
        if self.project.move_track(index, new_index as usize).is_ok() {
            self.roll.select_track(new_index as usize);
            self.sync_sound_drafts();
            self.dirty = true;
            self.report(format!(
                "Moved {} to track {} of {}.",
                self.project.tracks[new_index as usize].name,
                new_index + 1,
                self.project.tracks.len()
            ));
        }
    }

    pub(super) fn add_track(&mut self) {
        if !self.structural_edit_allowed() {
            return;
        }
        match self.add_track_quietly() {
            Ok(name) => self.report(speech::track_added(&name, "synth", "Lead")),
            Err(error) => self.report(format!("Could not add a track: {error}.")),
        }
    }

    /// Adds a synth track with the Lead sound and selects it, saying nothing: a new project's
    /// first track is part of the project, not a command of its own.
    pub(super) fn add_track_quietly(&mut self) -> Result<String, evil_project::Error> {
        let name = format!("Track {}", self.project.tracks.len() + 1);
        let instrument = self
            .project
            .add_instrument(Instrument::synth(&name, "lead"))?;
        let index = self.project.add_track(Track {
            name: name.clone(),
            instrument,
            ..Track::default()
        })?;
        self.roll.select_track(index);
        self.sync_sound_drafts();
        self.dirty = true;
        if self.view == View::Pads {
            self.view = View::Synth;
        }
        Ok(name)
    }

    pub(super) fn add_drum_track(&mut self) {
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
                    name: name.clone(),
                    instrument,
                    ..Track::default()
                })
            });
        match result {
            Ok(index) => {
                self.roll.select_track(index);
                self.sync_sound_drafts();
                self.view = View::Pads;
                self.dirty = true;
                self.report(speech::track_added(&name, "drum", "Maxwell kit"));
            }
            Err(error) => self.report(format!("Could not add a drum track: {error}.")),
        }
    }

    /// The selected track's name field: what it said when it took focus, and, when it lets focus
    /// go having changed, the new name said once — not a keystroke at a time. The field is drawn
    /// for whichever track is selected, so a rename ends when the selection moves under it.
    pub(super) fn track_name_edited(&mut self, response: &egui::Response, index: usize) {
        if self
            .track_name_before_edit
            .as_ref()
            .is_some_and(|(edited, _)| *edited != index)
        {
            self.finish_track_rename();
            if response.has_focus() {
                self.track_name_before_edit =
                    Some((index, self.project.tracks[index].name.clone()));
            }
        }
        if response.gained_focus() {
            self.track_name_before_edit = Some((index, self.project.tracks[index].name.clone()));
        }
        if response.lost_focus() {
            self.finish_track_rename();
        }
    }

    /// Says the rename in progress, if it changed the name, and ends it.
    fn finish_track_rename(&mut self) {
        if let Some((edited, before)) = self.track_name_before_edit.take()
            && let Some(track) = self.project.tracks.get(edited)
            && track.name != before
        {
            let text = format!("Renamed {before} to {}.", track.name);
            self.report(text);
        }
    }

    /// The mute box was toggled.
    pub(super) fn track_mute_toggled(&mut self, index: usize) {
        let track = &self.project.tracks[index];
        let text = format!(
            "{} {}.",
            track.name,
            if track.muted { "muted" } else { "unmuted" }
        );
        self.report(text);
    }
}
