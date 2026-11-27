//! The Song view, and the one boundary every arranger command passes through — a key's, a
//! button's, the inspector's and the pointer's alike — where a take's refusal is decided and
//! announced, and where every outcome is reported.

use super::*;
use crate::arranger::{
    self, SongView,
    cursor::{self, SongCursor},
    edits::{self, SongCommand, SongOutcome},
};
use crate::speech::{self, Meter, Unavailable};

impl StudioApp {
    /// Why the arrangement and the track list cannot change now, if they cannot: a take holds
    /// its target track and part by index. The buttons show it; `structural_edit_allowed`
    /// enforces it.
    pub(super) fn structure_refused(&self) -> Option<Unavailable> {
        self.recorder
            .is_some()
            .then_some(Unavailable::WhileRecording)
    }

    /// The boundary's check: false, and the refusal said, while a take is running.
    pub(super) fn structural_edit_allowed(&mut self) -> bool {
        match self.structure_refused() {
            Some(reason) => {
                self.report(speech::unavailable_reason(reason));
                false
            }
            None => true,
        }
    }

    /// The arranger's cursor is the selection the roll edits: the track and its part, or its
    /// timeline.
    pub(super) fn song_cursor(&self) -> Option<SongCursor> {
        cursor::kept_inside(
            &self.project,
            SongCursor {
                track: self.roll.selected_track,
                part: self.roll.selected_part,
            },
        )
    }

    pub(super) fn song_view(&mut self, ui: &mut egui::Ui) {
        let previous_transport = self.transport;
        let playhead_tick = self.song_playhead_tick();
        let cursor = self.song_cursor();
        let sound = self
            .selected_sound()
            .map(|sound| sound.name)
            .unwrap_or_default();
        let transport = self.transport;
        let structure_refused = self.structure_refused();
        let commands = arranger::show(
            ui,
            &mut self.project,
            &mut self.song,
            SongView {
                cursor,
                transport: &transport,
                playhead_tick,
                structure_refused,
                sound: &sound,
            },
            &mut self.announcer,
        );
        for command in commands {
            self.run_song_command(command);
        }
        if self.transport != previous_transport {
            self.stop_preview();
        }
    }

    /// One arranger command, from wherever it came. Returns whether it changed the view.
    pub(super) fn run_song_command(&mut self, command: SongCommand) -> bool {
        if command == SongCommand::ReadTransport {
            self.announcer
                .announce(speech::describe_transport(&self.transport_state()));
            return false;
        }
        let Some(cursor) = self.song_cursor() else {
            return false;
        };
        if command.changes_structure() && !self.structural_edit_allowed() {
            return false;
        }
        let before_transport = self.transport;
        let edited = edits::apply(&mut self.project, cursor, command, &mut self.transport);
        let parts_changed = matches!(
            edited.outcome,
            SongOutcome::Removed
                | SongOutcome::Merged
                | SongOutcome::PartAdded { .. }
                | SongOutcome::PartDuplicated { .. }
        );
        // A move onto another track is a change of the selected track, decided where every
        // such change is: refused during a take, and the sound drafts kept.
        if edited.cursor.track != self.roll.selected_track
            && !self.select_sound_track(edited.cursor.track)
        {
            return false;
        }
        self.roll
            .select_part(edited.cursor.track, edited.cursor.part);
        if parts_changed {
            // The part at the cursor's index may be another part now; the roll's note cursor
            // belonged to the old one.
            self.roll.cursor.clear();
        }
        if self.transport != before_transport {
            self.stop_preview();
        }
        if let Some(text) = speech::describe_song_outcome(&edited.outcome, Meter::of(&self.project))
        {
            self.report(text);
        }
        if edited.open_in_roll {
            self.open_in_roll(edited.cursor);
            return true;
        }
        false
    }

    /// Shows the cursor's timeline or part in the roll and moves focus there; what is open is
    /// said once the roll's name has been heard.
    fn open_in_roll(&mut self, cursor: SongCursor) {
        self.select_view(View::Notes);
        self.roll.focus_when_shown = true;
        let track = &self.project.tracks[cursor.track];
        let pattern = cursor.part.map(|part| {
            self.project.patterns[track.parts[part].pattern]
                .name
                .as_str()
        });
        let text = speech::opened_in_roll(&track.name, pattern);
        self.report_after_focus_moves(text);
    }

    /// One arranger key, recorded for undo at the key under its gesture, as a roll key is.
    pub(super) fn song_key(&mut self, command: SongCommand) -> bool {
        self.record_pending_edit();
        let before = self.editor_snapshot();
        let changed_view = self.run_song_command(command);
        self.song.scroll_to_cursor = true;
        self.record_key_edit(before);
        changed_view
    }
}
