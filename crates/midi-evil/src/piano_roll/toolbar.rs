//! The roll's toolbar. Every edit a chord makes has a button here too, because a browser may
//! keep the chord; each button calls the same edit as its chord and is announced as it is.

use eframe::egui;
use evil_project::{EventSequence, Note};

use super::{EditedSequence, RollState, cursor, edits::RollCommand};
use crate::accessibility::Refusable;
use crate::{announcer::Announcer, speech, theme};

const ONE_NOTE_BUTTONS: [(RollCommand, &str); 6] = [
    (RollCommand::Earlier, "Earlier"),
    (RollCommand::Later, "Later"),
    (RollCommand::Shorter, "Shorter"),
    (RollCommand::Longer, "Longer"),
    (RollCommand::Add, "Add"),
    (RollCommand::Delete, "Delete"),
];

pub fn show(
    ui: &mut egui::Ui,
    edited: &EditedSequence,
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    announcer: &mut Announcer,
) {
    let context = edited.context;
    let cursor = state.cursor.index(context);
    let mut pressed = None;
    ui.horizontal_wrapped(|ui| {
        theme::label(
            ui,
            if edited.grid.pattern_length.is_some() {
                "SHARED PATTERN"
            } else {
                "UNIQUE TIMELINE"
            },
        );
        ui.add(egui::Slider::new(&mut state.zoom, 0.25..=3.0).text("Zoom"));
        if ui.button("Quantize").clicked() {
            whole_sequence(
                sequence,
                state,
                edited,
                announcer,
                Operation {
                    done: "Quantized NOTES.",
                    keeps_order: KeepsOrder::Yes,
                    reach: Reach::SelectedNoteElseAll,
                },
                |sequence| {
                    let _ = evil_project::quantize_notes(
                        sequence,
                        edited.grid.snap,
                        cursor,
                        edited.grid.pattern_length,
                    );
                },
            );
        }
        for (label, semitones) in [("−12", -12), ("−1", -1), ("+1", 1), ("+12", 12)] {
            if ui.button(label).clicked() {
                whole_sequence(
                    sequence,
                    state,
                    edited,
                    announcer,
                    Operation {
                        done: transposed(semitones),
                        keeps_order: KeepsOrder::Yes,
                        reach: Reach::SelectedNoteElseAll,
                    },
                    |sequence| {
                        let _ = evil_project::transpose_notes(sequence, semitones, cursor);
                    },
                );
            }
        }
        if ui.button("Clean overlaps").clicked() {
            whole_sequence(
                sequence,
                state,
                edited,
                announcer,
                Operation {
                    done: "Cleaned the overlaps of NOTES.",
                    keeps_order: KeepsOrder::No,
                    reach: Reach::AllNotes,
                },
                |sequence| {
                    evil_project::clean_overlaps(sequence);
                },
            );
        }
        if ui.button("Copy").clicked() {
            state.clipboard = match cursor.and_then(|index| sequence.notes().get(index)) {
                Some(note) => vec![*note],
                None => sequence.notes().to_vec(),
            };
            announcer.announce(format!("Copied {}.", notes_counted(state.clipboard.len())));
        }
        if !state.clipboard.is_empty() && ui.button("Paste").clicked() {
            let first_pasted = paste(
                sequence,
                &state.clipboard,
                cursor,
                edited.grid.pattern_length,
            );
            state
                .cursor
                .set(context, first_pasted.or(cursor), sequence.notes());
            // After the roll's name, which the cursor's move to the first pasted note changes.
            announcer.announce_next_frame(match first_pasted {
                Some(_) => "Pasted.",
                None => "Nothing pasted: the pattern ends here.",
            });
        }
        ui.separator();
        for (command, label) in ONE_NOTE_BUTTONS {
            let refused = (command.edits_one_note() && cursor.is_none())
                .then_some(speech::Unavailable::NoNoteSelected);
            let (_, key) = command.chord();
            let button = Refusable::add(ui, refused, egui::Button::new(label))
                .on_hover_text(chord_hint(command, key));
            if button.clicked() {
                pressed = Some(command);
            }
            if let Some(reason) = button.refusal() {
                announcer.announce(reason);
            }
        }
        ui.label(
            egui::RichText::new(if cursor.is_some() {
                "Operations affect the selected note · Copy/Paste inserts after it"
            } else {
                "Operations affect all notes · Click to draw · Drag to move/resize"
            })
            .size(11.0)
            .color(theme::MUTED),
        );
    });
    if let Some(command) = pressed {
        let roll_focused = state
            .widget_id
            .is_some_and(|roll| ui.memory(|memory| memory.has_focus(roll)));
        let result = super::run_command(sequence, state, edited, command, announcer);
        // Focused, the roll's name says where the cursor is; from a button pressed elsewhere
        // nothing would, so the note goes with the outcome.
        if !roll_focused {
            let notes = sequence.notes();
            announcer.announce(speech::roll_name(
                result.cursor.and_then(|index| notes.get(index)),
                notes.len(),
                edited.meter,
                edited.origin,
            ));
        }
    }
}

/// Whether an operation leaves every note it keeps in its place in the list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum KeepsOrder {
    Yes,
    No,
}

/// Which notes an operation acts on: the editor's own operations act on the selected note when
/// there is one; Clean overlaps acts on every note whatever is selected.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    SelectedNoteElseAll,
    AllNotes,
}

/// A whole-sequence operation as the toolbar runs it.
struct Operation {
    /// What was done, with NOTES where the notes it was done to are named.
    done: &'static str,
    keeps_order: KeepsOrder,
    reach: Reach,
}

/// Runs an operation on the whole sequence and keeps the cursor on its note. When the operation
/// keeps the order and drops no note, the index still names the same note; otherwise — a
/// re-sort, or Quantize dropping notes past a pattern's end — the cursor follows only a note it
/// can prove is the same, and is cleared, saying so, when it cannot.
fn whole_sequence(
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    edited: &EditedSequence,
    announcer: &mut Announcer,
    Operation {
        done,
        keeps_order,
        reach,
    }: Operation,
    operation: impl FnOnce(&mut dyn EventSequence),
) {
    let cursor = state.cursor.index(edited.context);
    let before = sequence.notes().to_vec();
    operation(sequence);
    let after = sequence.notes();
    if after == before.as_slice() {
        // Nothing changed, so nothing moved under the cursor.
        announcer.announce("Nothing changed.");
        return;
    }
    // A frame late, so that a selection reset, which a listener must hear, is heard first.
    let notes = match (reach, cursor) {
        (Reach::SelectedNoteElseAll, Some(_)) => "the selected note".to_owned(),
        _ => format!("all {}", notes_counted(after.len())),
    };
    announcer.announce_next_frame(done.replace("NOTES", &notes));
    let kept = cursor.and_then(|index| {
        if keeps_order == KeepsOrder::Yes && after.len() == before.len() {
            Some(index)
        } else {
            cursor::follow(&before, index, after)
        }
    });
    if cursor.is_some()
        && kept.is_none()
        && let Some(text) =
            speech::describe_outcome(super::edits::Outcome::SelectionReset, edited.origin)
    {
        announcer.announce(text);
    }
    state.cursor.set(edited.context, kept, after);
}

fn transposed(semitones: i16) -> &'static str {
    match semitones {
        12 => "Moved NOTES an octave up.",
        1 => "Moved NOTES a semitone up.",
        -1 => "Moved NOTES a semitone down.",
        _ => "Moved NOTES an octave down.",
    }
}

fn notes_counted(count: usize) -> String {
    if count == 1 {
        "1 note".to_owned()
    } else {
        format!("{count} notes")
    }
}

fn chord_hint(command: RollCommand, key: egui::Key) -> String {
    let modifier = command.chord().0.prefix(speech::platform_command_key());
    format!("{modifier}{} in the roll", key.name())
}

/// Pastes the clipboard after the cursor's note, or at the start; returns the first pasted
/// note, which the cursor moves to. Notes a shared pattern cannot hold are left out.
fn paste(
    sequence: &mut dyn EventSequence,
    clipboard: &[Note],
    cursor: Option<usize>,
    pattern_length: Option<u64>,
) -> Option<usize> {
    let origin = clipboard
        .iter()
        .map(|note| note.start_tick)
        .min()
        .unwrap_or(0);
    let destination = cursor
        .and_then(|index| sequence.notes().get(index))
        .map_or(0, |note| note.start_tick + note.duration_ticks);
    let additions: Vec<_> = clipboard
        .iter()
        .filter_map(|note| {
            let start_tick = destination.checked_add(note.start_tick - origin)?;
            start_tick.checked_add(note.duration_ticks)?;
            Some(Note {
                start_tick,
                ..*note
            })
        })
        .filter(|note| pattern_length.is_none_or(|length| note.start_tick < length))
        .collect();
    let first_pasted = (!additions.is_empty()).then_some(sequence.notes().len());
    sequence.notes_mut().extend(additions);
    first_pasted
}
