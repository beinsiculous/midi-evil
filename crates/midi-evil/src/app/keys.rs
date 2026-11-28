//! One ordered key dispatch per frame. Each key event goes, in the order it arrived, to the
//! focused editor when that editor takes the chord and to the application's commands when not,
//! and is consumed by whoever handled it. Running the global commands first and the editors
//! after would turn Delete then Control+Z, arriving in one frame, into an undo then a delete.
//! The application's editors are the piano roll and the arranger; at most one is shown.

use super::*;
use crate::arranger::edits::SongCommand;
use crate::key_help::AppCommand;
use crate::piano_roll::{
    EditedSequence,
    edits::{Outcome, RollCommand},
};
use crate::regions;
use crate::speech;

impl StudioApp {
    pub(super) fn dispatch_keys(&mut self, context: &egui::Context) {
        let events = context.input(|input| input.events.clone());
        // A menu, a combo box's list or a window owns Escape and the arrows while it is up.
        let behind_a_popup = self.popup_or_window_open(context);
        let mut roll = self.roll.widget_id.filter(|_| {
            self.view() == View::Notes && !self.project.tracks.is_empty() && !behind_a_popup
        });
        let mut song = self.song.widget_id.filter(|_| {
            self.view() == View::Song && !self.project.tracks.is_empty() && !behind_a_popup
        });
        // A field whose edit ended keeps its place without egui's focus, and Tab moves on from
        // it: the Tab is taken here, before egui walks from nothing to the first control.
        let tab = events.iter().position(|event| {
            matches!(event, egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                if modifiers.is_none() || modifiers.shift_only())
        });
        let next_stop_after_the_place_kept = tab.and_then(|position| match events[position] {
            egui::Event::Key { modifiers, .. }
                if context.memory(|memory| memory.focused()).is_none() =>
            {
                self.announcer
                    .next_stop_after_the_place_kept(!modifiers.shift)
            }
            _ => None,
        });
        if let Some(next) = next_stop_after_the_place_kept {
            // egui read the Tab before this frame began; its walk is called off.
            context.memory_mut(|memory| {
                memory.request_focus(next);
                memory.move_focus(egui::FocusDirection::None);
            });
        }
        let mut focused = context.memory(|memory| memory.focused());
        let editor_focused =
            |focused: Option<egui::Id>, roll: Option<egui::Id>, song: Option<egui::Id>| {
                focused.is_some() && (focused == roll || focused == song)
            };
        if self.held_gesture.is_some() && !editor_focused(focused, roll, song) {
            // Focus left the editor; whatever the held key does next is not its gesture.
            self.end_gesture();
        }
        if roll.is_some() && focused.is_none() && self.roll.had_focus && self.roll_has_cursor() {
            let escape_pressed = events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            });
            // egui drops focus on Escape before any widget runs; with a cursor, Escape is the
            // roll's, and it clears the cursor instead.
            if let (true, Some(roll)) = (escape_pressed, roll) {
                context.memory_mut(|memory| memory.request_focus(roll));
                focused = Some(roll);
            }
        }
        // Only a text field being edited keeps the application's chords from it; a focused
        // editor that does not claim a chord leaves it to the application.
        let text_editing = context.text_edit_focused();
        let platform = crate::platform::of(context);
        let mut handled = vec![false; events.len()];
        let mut last_focus_move_was_an_editors = false;
        for (position, event) in events.iter().enumerate() {
            let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = *event
            else {
                continue;
            };
            self.announcer.observe_key(key, pressed, repeat);
            // A release moves no focus; only a later press can take the last move from the roll.
            if !pressed {
                self.track_gesture(key, modifiers, false, false, false);
                continue;
            }
            if moves_egui_focus(key, modifiers) {
                last_focus_move_was_an_editors = false;
            }
            let roll_focused = roll.is_some() && focused == roll;
            let song_focused = song.is_some() && focused == song;
            let roll_command = RollCommand::from_key(key, modifiers).filter(|_| roll_focused);
            let song_command = SongCommand::from_key(key, modifiers).filter(|_| song_focused);
            self.track_gesture(
                key,
                modifiers,
                true,
                repeat,
                roll_command.is_some_and(RollCommand::repeats_when_held)
                    || song_command.is_some_and(SongCommand::repeats_when_held),
            );
            if key == egui::Key::Tab && (modifiers.is_none() || modifiers.shift_only()) {
                // Taken above, from a kept place: egui must not walk it again.
                if next_stop_after_the_place_kept.is_some() {
                    handled[position] = true;
                    continue;
                }
                // Focus leaves at the end of the frame; nothing after the Tab is the editor's.
                if roll_focused || song_focused {
                    focused = None;
                }
                continue;
            }
            let app_command = AppCommand::from_press(key, modifiers, platform);
            if let Some(region @ (AppCommand::NextRegion | AppCommand::PreviousRegion)) =
                app_command
            {
                let forwards = region == AppCommand::NextRegion;
                if !repeat
                    && !behind_a_popup
                    && let Some(target) = regions::next_stop(context, focused, forwards)
                {
                    context.memory_mut(|memory| memory.request_focus(target));
                    focused = Some(target);
                }
                handled[position] = !behind_a_popup;
                continue;
            }
            if let Some(command) = roll_command {
                // A held key repeats a move or a nudge; a held Delete or Enter would empty the
                // roll or fill it, one repeat at a time.
                if !repeat || command.repeats_when_held() {
                    self.roll_key(command);
                    last_focus_move_was_an_editors |= moves_egui_focus(key, modifiers);
                }
                handled[position] = true;
                continue;
            }
            if let Some(command) = song_command {
                // A held key repeats a move or a nudge; a held Delete would empty the track.
                if (!repeat || command.repeats_when_held()) && self.song_key(command) {
                    // Opened in the roll: nothing after it in this frame is the arranger's.
                    song = None;
                }
                last_focus_move_was_an_editors |= moves_egui_focus(key, modifiers);
                handled[position] = true;
                continue;
            }
            if !repeat {
                // Space is the transport's unless the focused widget acts on Space itself: a
                // button or a checkbox does; the application's own editors do not.
                let space_is_free = focused.is_none_or(|widget| {
                    roll_focused
                        || song_focused
                        || crate::custom_editors::was_registered(context, widget)
                });
                let showing =
                    piano_roll::sequence_context(&self.project, &self.roll, self.document);
                let view = self.view();
                handled[position] = app_command.is_some_and(|command| {
                    self.command_key(command, platform, text_editing, space_is_free)
                });
                // An undo, a redo or a replaced document may take away what the roll showed;
                // nothing after it in this frame is the roll's unless it shows the same notes.
                let still_showing =
                    piano_roll::sequence_context(&self.project, &self.roll, self.document);
                if handled[position]
                    && (still_showing != showing
                        || self.view() != view
                        || self.project.tracks.is_empty())
                {
                    roll = None;
                }
                if handled[position] && (self.view() != view || self.project.tracks.is_empty()) {
                    song = None;
                }
                if handled[position] {
                    focused = context.memory(|memory| memory.focused());
                }
            }
        }
        context.input_mut(|input| {
            let mut position = 0;
            input.events.retain(|_| {
                let keep = !handled.get(position).copied().unwrap_or(false);
                position += 1;
                keep
            });
        });
        if last_focus_move_was_an_editors {
            // egui chose a direction to move focus from this frame's keys before the editor
            // could claim them; on the frame focus arrives its focus-lock filter is not in force.
            context.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
    }

    /// A menu or a combo box's list (both kept open in egui's memory), or one of the
    /// application's windows.
    fn popup_or_window_open(&self, context: &egui::Context) -> bool {
        egui::Popup::is_any_open(context)
            || self.show_about
            || self.sounds.open
            || self.recovery.open
            || self.show_key_help
    }

    fn roll_has_cursor(&self) -> bool {
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        self.roll.cursor.index(context).is_some()
    }

    fn roll_key(&mut self, command: RollCommand) {
        if command == RollCommand::ReadTransport {
            self.announcer
                .announce(speech::describe_transport(&self.transport_state()));
            return;
        }
        // What the frame changed before this key is an entry of its own; this key's change is
        // recorded here, at the key, under its gesture, and the frame's end sees only what the
        // rest of the frame changes.
        self.record_pending_edit();
        let before = self.editor_snapshot();
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        let edited = EditedSequence::new(&self.project, context);
        let Some(sequence) = piano_roll::sequence_mut(&mut self.project, context) else {
            return;
        };
        let result = piano_roll::run_command(
            sequence,
            &mut self.roll,
            &edited,
            command,
            &mut self.announcer,
        );
        if matches!(
            result.outcome,
            Outcome::Edited | Outcome::Added | Outcome::Deleted
        ) {
            self.dirty = true;
        }
        let name = edited.roll_name(sequence.notes(), &self.roll);
        self.status = match speech::describe_outcome(result.outcome, edited.origin) {
            Some(outcome) => format!("{outcome} {name}"),
            None => name,
        };
        self.record_key_edit(before);
    }

    /// The application's own chords, on a key's first press. Returns whether the key was taken.
    fn command_key(
        &mut self,
        command: AppCommand,
        platform: crate::platform::Platform,
        text_editing: bool,
        space_is_free: bool,
    ) -> bool {
        match command {
            AppCommand::NewProject => self.new_project(),
            AppCommand::OpenProject => self.open_project(),
            AppCommand::SaveProject if self.view() == View::Pads => self.save_beats_bank(),
            AppCommand::SaveProject => self.save_project(),
            // A text field being edited keeps its own undo, and its own Space and arrows.
            AppCommand::Undo | AppCommand::Redo if text_editing => return false,
            AppCommand::Undo => self.undo(),
            AppCommand::Redo => self.redo(),
            _ if text_editing => return false,
            AppCommand::PreviousSound | AppCommand::NextSound if self.show_about => return false,
            AppCommand::PreviousSound => self.cycle_sound(true),
            AppCommand::NextSound => self.cycle_sound(false),
            AppCommand::PlayOrStop if !space_is_free => return false,
            AppCommand::PlayOrStop => {
                if self.queued_render.is_some() {
                    self.stop_transport();
                } else if self.render_is_cancelling() {
                    self.play(platform);
                } else if self.preview.is_some() || self.render_job.is_some() {
                    self.stop_transport();
                } else {
                    self.play(platform);
                }
            }
            AppCommand::NextRegion | AppCommand::PreviousRegion => return false,
        }
        true
    }
}

/// The keys egui itself moves focus with, when no focus-lock filter holds them.
fn moves_egui_focus(key: egui::Key, modifiers: egui::Modifiers) -> bool {
    use egui::Key;
    match key {
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight => modifiers.is_none(),
        Key::Tab => modifiers.is_none() || modifiers.shift_only(),
        _ => false,
    }
}
