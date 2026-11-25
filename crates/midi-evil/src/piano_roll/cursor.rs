//! The roll's cursor: an identity, not a guess. Notes have no ids, and no comparison of the list
//! before and after can tell a note from its twin, so the cursor is kept by whoever changes the
//! list. The roll's own edits set it; undo restores it; a change of what is edited clears it;
//! and only a change nobody accounted for — a whole-sequence operation, a take — is followed by
//! value, under rules strict enough that it never lands on a different note.

use evil_project::Note;

/// What the roll is editing. A cursor is never compared across a different one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceContext {
    /// Counts documents opened, imported or started, so a cursor never survives into another.
    pub document: u64,
    pub track: usize,
    pub part: Option<usize>,
    /// The shared pattern the part plays; an arranger command can change it under the part.
    pub pattern: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    context: SequenceContext,
    index: usize,
}

/// What reconciling the cursor with the notes found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciled {
    Unchanged,
    Followed,
    /// The cursor was cleared by something other than the person's own selection.
    Reset,
}

/// The cursor and the notes as the cursor last knew them.
#[derive(Debug, Default)]
pub struct CursorKeeper {
    cursor: Option<Cursor>,
    seen: Option<(SequenceContext, Vec<Note>)>,
}

impl CursorKeeper {
    /// The cursor's index, only within the context it was set in.
    pub fn index(&self, context: SequenceContext) -> Option<usize> {
        self.cursor
            .filter(|cursor| cursor.context == context)
            .map(|cursor| cursor.index)
    }

    /// Sets the cursor after an edit the roll made itself, and takes the notes as they now are.
    pub fn set(&mut self, context: SequenceContext, index: Option<usize>, notes: &[Note]) {
        self.cursor = index
            .filter(|&index| index < notes.len())
            .map(|index| Cursor { context, index });
        self.seen = Some((context, notes.to_vec()));
    }

    /// The person chose another track, part or document: no cursor, and nothing to say.
    pub fn clear(&mut self) {
        self.cursor = None;
        self.seen = None;
    }

    /// Finds the cursor's note again after a change the roll did not make. Call it before the
    /// cursor's index is used in a frame; it is cheap when nothing changed.
    pub fn reconcile(&mut self, context: SequenceContext, notes: &[Note]) -> Reconciled {
        let result = match (self.cursor, &self.seen) {
            (None, _) => Reconciled::Unchanged,
            (Some(cursor), _) if cursor.context != context => {
                self.cursor = None;
                let same_selection = cursor.context.document == context.document
                    && cursor.context.track == context.track
                    && cursor.context.part == context.part;
                if same_selection {
                    Reconciled::Reset
                } else {
                    Reconciled::Unchanged
                }
            }
            (Some(_), Some((seen_context, seen_notes)))
                if *seen_context == context && seen_notes.as_slice() == notes =>
            {
                Reconciled::Unchanged
            }
            (Some(cursor), Some((seen_context, seen_notes))) if *seen_context == context => {
                match follow(seen_notes, cursor.index, notes) {
                    Some(index) => {
                        self.cursor = Some(Cursor { context, index });
                        Reconciled::Followed
                    }
                    None => {
                        self.cursor = None;
                        Reconciled::Reset
                    }
                }
            }
            (Some(cursor), _) if cursor.index < notes.len() => Reconciled::Unchanged,
            (Some(_), _) => {
                self.cursor = None;
                Reconciled::Reset
            }
        };
        if self.seen.as_ref().is_none_or(|(seen_context, seen_notes)| {
            *seen_context != context || seen_notes.as_slice() != notes
        }) {
            self.seen = Some((context, notes.to_vec()));
        }
        result
    }
}

/// Where a note is after a whole-sequence change that may re-sort the list. It follows only a
/// note that was unique by value before and occurs once after; failing that, when the count is
/// unchanged, a note unique by start and pitch on both sides (Clean overlaps shortens the notes
/// it re-sorts). Anything else is `None`: the cursor is cleared rather than land on a twin.
pub fn follow(before: &[Note], index: usize, after: &[Note]) -> Option<usize> {
    let note = *before.get(index)?;
    let equal_before = before.iter().filter(|other| **other == note).count();
    let equal_after: Vec<usize> = positions(after, |other| *other == note);
    if equal_before == 1 && equal_after.len() == 1 {
        return equal_after.first().copied();
    }
    if before.len() != after.len() {
        return None;
    }
    let same_onset =
        |other: &Note| other.start_tick == note.start_tick && other.pitch == note.pitch;
    let onset_before = before.iter().filter(|other| same_onset(other)).count();
    match positions(after, same_onset).as_slice() {
        [index] if onset_before == 1 => Some(*index),
        _ => None,
    }
}

fn positions(notes: &[Note], matches: impl Fn(&Note) -> bool) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, note)| matches(note))
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: SequenceContext = SequenceContext {
        document: 1,
        track: 0,
        part: None,
        pattern: None,
    };

    fn note_at(pitch: u8, start_tick: u64, duration_ticks: u64) -> Note {
        Note {
            pitch,
            velocity: 100,
            start_tick,
            duration_ticks,
        }
    }

    #[test]
    fn an_edit_in_place_by_the_roll_keeps_the_cursor_on_its_note_not_its_twin() {
        let twin = note_at(60, 0, 480);
        let mut notes = vec![twin, twin];
        let mut keeper = CursorKeeper::default();
        keeper.set(CONTEXT, Some(1), &notes);
        for velocity in [90, 80, 70] {
            notes[1].velocity = velocity;
            keeper.set(CONTEXT, keeper.index(CONTEXT), &notes);
            assert_eq!(keeper.reconcile(CONTEXT, &notes), Reconciled::Unchanged);
            assert_eq!(keeper.index(CONTEXT), Some(1));
        }
    }

    #[test]
    fn a_unique_note_is_followed_through_a_re_sort() {
        let before = vec![note_at(72, 120, 120), note_at(71, 120, 240)];
        let after = vec![note_at(71, 120, 240), note_at(72, 120, 120)];
        assert_eq!(follow(&before, 0, &after), Some(1));
    }

    #[test]
    fn a_shortened_note_is_followed_by_start_and_pitch() {
        let before = vec![note_at(60, 0, 960), note_at(60, 480, 480)];
        let after = vec![note_at(60, 0, 480), note_at(60, 480, 480)];
        assert_eq!(follow(&before, 0, &after), Some(0));
    }

    #[test]
    fn a_twin_or_a_vanished_note_is_never_followed() {
        let twin = note_at(60, 0, 480);
        assert_eq!(follow(&[twin, twin], 1, &[twin, twin]), None);
        assert_eq!(follow(&[twin, twin], 1, &[twin]), None);
        assert_eq!(follow(&[twin], 0, &[note_at(61, 0, 480)]), None);
    }

    #[test]
    fn a_change_nobody_accounted_for_follows_or_resets_and_says_which() {
        let mut keeper = CursorKeeper::default();
        let notes = vec![note_at(60, 0, 480), note_at(62, 480, 480)];
        keeper.set(CONTEXT, Some(1), &notes);
        let grown = vec![
            note_at(62, 480, 480),
            note_at(60, 0, 480),
            note_at(64, 0, 120),
        ];
        assert_eq!(keeper.reconcile(CONTEXT, &grown), Reconciled::Followed);
        assert_eq!(keeper.index(CONTEXT), Some(0));
        let twinned = vec![note_at(62, 480, 480), note_at(62, 480, 480)];
        assert_eq!(keeper.reconcile(CONTEXT, &twinned), Reconciled::Reset);
        assert_eq!(keeper.index(CONTEXT), None);
        assert_eq!(keeper.reconcile(CONTEXT, &notes), Reconciled::Unchanged);
    }

    #[test]
    fn another_pattern_under_the_same_part_resets_and_another_track_is_silent() {
        let notes = vec![note_at(60, 0, 480)];
        let part = SequenceContext {
            part: Some(0),
            pattern: Some(0),
            ..CONTEXT
        };
        let mut keeper = CursorKeeper::default();
        keeper.set(part, Some(0), &notes);
        let made_unique = SequenceContext {
            pattern: Some(1),
            ..part
        };
        assert_eq!(keeper.reconcile(made_unique, &notes), Reconciled::Reset);
        keeper.set(part, Some(0), &notes);
        let other_track = SequenceContext { track: 1, ..part };
        assert_eq!(keeper.reconcile(other_track, &notes), Reconciled::Unchanged);
        assert_eq!(keeper.index(other_track), None);
    }
}
