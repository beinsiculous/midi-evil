//! The roll's keyboard model and its edits. Every edit is a pure function of the notes, the
//! cursor and the command, returning the new cursor and what happened; keys, the toolbar's
//! buttons, the pointer and the inspector all call these, so the cursor is never guessed after
//! an edit the roll made itself.

use eframe::egui;
use evil_project::Note;

pub const MAX_PITCH: u8 = 127;
pub const MIDDLE_C: u8 = 60;
pub const DEFAULT_NOTE_VELOCITY: u8 = 100;

/// One command of the roll's keyboard model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollCommand {
    Previous,
    Next,
    First,
    Last,
    PitchUp,
    PitchDown,
    OctaveUp,
    OctaveDown,
    Earlier,
    Later,
    Shorter,
    Longer,
    Add,
    Delete,
    ClearSelection,
    ReadTransport,
}

/// The modifier half of a chord. `Command` is Control, or Command on macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordModifier {
    None,
    Shift,
    Command,
}

impl RollCommand {
    pub const ALL: [Self; 16] = [
        Self::Previous,
        Self::Next,
        Self::First,
        Self::Last,
        Self::PitchUp,
        Self::PitchDown,
        Self::OctaveUp,
        Self::OctaveDown,
        Self::Earlier,
        Self::Later,
        Self::Shorter,
        Self::Longer,
        Self::Add,
        Self::Delete,
        Self::ClearSelection,
        Self::ReadTransport,
    ];

    /// The one chord bound to this command: the key help and the key dispatch both read it.
    pub fn chord(self) -> (ChordModifier, egui::Key) {
        use ChordModifier::{Command, None, Shift};
        use egui::Key;
        match self {
            Self::Previous => (None, Key::ArrowLeft),
            Self::Next => (None, Key::ArrowRight),
            Self::First => (None, Key::Home),
            Self::Last => (None, Key::End),
            Self::PitchUp => (None, Key::ArrowUp),
            Self::PitchDown => (None, Key::ArrowDown),
            Self::OctaveUp => (Shift, Key::ArrowUp),
            Self::OctaveDown => (Shift, Key::ArrowDown),
            Self::Earlier => (Command, Key::ArrowLeft),
            Self::Later => (Command, Key::ArrowRight),
            Self::Shorter => (Shift, Key::ArrowLeft),
            Self::Longer => (Shift, Key::ArrowRight),
            Self::Add => (None, Key::Enter),
            Self::Delete => (None, Key::Delete),
            Self::ClearSelection => (None, Key::Escape),
            // The transport's position, asked for from inside the roll. A plain letter: no
            // browser keeps it, and I is not among the keys the piano or the pads take.
            Self::ReadTransport => (None, Key::I),
        }
    }

    /// The command a key press means, if the roll takes it. Modifiers must match exactly, so
    /// Right and Control+Right are never confused.
    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        let modifier = if modifiers.is_none() {
            ChordModifier::None
        } else if modifiers.shift_only() {
            ChordModifier::Shift
        } else if modifiers.command_only() {
            ChordModifier::Command
        } else {
            return None;
        };
        Self::ALL
            .into_iter()
            .find(|command| command.chord() == (modifier, key))
    }

    /// The moves and nudges, which a held key repeats. Everything else acts on the first
    /// press only: a held Delete or Enter must not empty the roll or fill it.
    pub fn repeats_when_held(self) -> bool {
        matches!(
            self,
            Self::Previous
                | Self::Next
                | Self::PitchUp
                | Self::PitchDown
                | Self::OctaveUp
                | Self::OctaveDown
                | Self::Earlier
                | Self::Later
                | Self::Shorter
                | Self::Longer
        )
    }

    /// The commands that edit the cursor's note: without a cursor their buttons are disabled.
    pub fn edits_one_note(self) -> bool {
        matches!(
            self,
            Self::Earlier | Self::Later | Self::Shorter | Self::Longer | Self::Delete
        )
    }
}

/// What an edit did. Every outcome that changes no note has words of its own in `speech`,
/// because an unchanged name is not spoken again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Moved,
    Edited,
    Added,
    Deleted,
    AlreadyFirst,
    AlreadyLast,
    AlreadyHighest,
    AlreadyLowest,
    AlreadyAtStart,
    AlreadyAtEnd,
    AlreadyShortest,
    AlreadyLongest,
    NoRoom,
    NoSelection,
    Empty,
    Cleared,
    SelectionReset,
}

/// The grid the edits move on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditGrid {
    pub snap: u64,
    pub beat: u64,
    /// A shared pattern's length; every note must start inside it or the project cannot save.
    pub pattern_length: Option<u64>,
}

/// The new cursor, an index into the notes as they are after the edit, and what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edited {
    pub cursor: Option<usize>,
    pub outcome: Outcome,
}

impl Edited {
    fn new(cursor: Option<usize>, outcome: Outcome) -> Self {
        Self { cursor, outcome }
    }
}

/// Note indices in the order the cursor walks them: by start, then pitch, then index.
pub fn walk_order(notes: &[Note]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by_key(|&index| (notes[index].start_tick, notes[index].pitch, index));
    order
}

/// Applies one command. A cursor out of range is treated as no cursor. Clearing the selection
/// and reading the transport change no note and are the caller's to announce.
pub fn apply(
    notes: &mut Vec<Note>,
    cursor: Option<usize>,
    command: RollCommand,
    grid: EditGrid,
) -> Edited {
    let cursor = cursor.filter(|&index| index < notes.len());
    match command {
        RollCommand::ClearSelection => {
            let outcome = if cursor.is_some() {
                Outcome::Cleared
            } else {
                Outcome::NoSelection
            };
            return Edited::new(None, outcome);
        }
        RollCommand::ReadTransport => return Edited::new(cursor, Outcome::Moved),
        RollCommand::Add => return add_after(notes, cursor, grid),
        _ => {}
    }
    if notes.is_empty() {
        return Edited::new(None, Outcome::Empty);
    }
    let order = walk_order(notes);
    let place = cursor.and_then(|index| order.iter().position(|&walked| walked == index));
    let last_place = order.len() - 1;
    match (command, place) {
        (RollCommand::Next, None) | (RollCommand::First, None) => {
            Edited::new(Some(order[0]), Outcome::Moved)
        }
        (RollCommand::Previous, None) | (RollCommand::Last, None) => {
            Edited::new(Some(order[last_place]), Outcome::Moved)
        }
        (RollCommand::Next, Some(place)) if place < last_place => {
            Edited::new(Some(order[place + 1]), Outcome::Moved)
        }
        (RollCommand::Next | RollCommand::Last, Some(place)) if place == last_place => {
            Edited::new(cursor, Outcome::AlreadyLast)
        }
        (RollCommand::Last, Some(_)) => Edited::new(Some(order[last_place]), Outcome::Moved),
        (RollCommand::Previous | RollCommand::First, Some(0)) => {
            Edited::new(cursor, Outcome::AlreadyFirst)
        }
        (RollCommand::Previous, Some(place)) => Edited::new(Some(order[place - 1]), Outcome::Moved),
        (RollCommand::First, Some(_)) => Edited::new(Some(order[0]), Outcome::Moved),
        (RollCommand::Delete, Some(place)) => delete_at(notes, &order, place),
        (_, Some(place)) => {
            let index = order[place];
            Edited::new(Some(index), edit_note(&mut notes[index], command, grid))
        }
        (_, None) => Edited::new(None, Outcome::NoSelection),
    }
}

/// Removes the note at `place` in walk order; the cursor goes to the next note, else the
/// previous, else nowhere.
fn delete_at(notes: &mut Vec<Note>, order: &[usize], place: usize) -> Edited {
    let index = order[place];
    let neighbour = order
        .get(place + 1)
        .or_else(|| place.checked_sub(1).and_then(|before| order.get(before)))
        .map(|&other| if other > index { other - 1 } else { other });
    notes.remove(index);
    Edited::new(neighbour, Outcome::Deleted)
}

/// A one-beat note after the cursor's note at its pitch, or middle C at tick 0 with no cursor.
fn add_after(notes: &mut Vec<Note>, cursor: Option<usize>, grid: EditGrid) -> Edited {
    let (pitch, start_tick) = match cursor {
        Some(index) => {
            let after = notes[index];
            match after.start_tick.checked_add(after.duration_ticks) {
                Some(end) => (after.pitch, end),
                None => return Edited::new(cursor, Outcome::NoRoom),
            }
        }
        None => (MIDDLE_C, 0),
    };
    match insert(notes, pitch, start_tick, grid.beat, grid) {
        Some(index) => Edited::new(Some(index), Outcome::Added),
        None => Edited::new(cursor, Outcome::NoRoom),
    }
}

/// A one-beat note where the pointer clicked, kept inside a shared pattern.
pub fn add_at(notes: &mut Vec<Note>, pitch: u8, start_tick: u64, grid: EditGrid) -> Edited {
    let start_tick =
        latest_start(grid, grid.beat).map_or(start_tick, |latest| start_tick.min(latest));
    match insert(notes, pitch.min(MAX_PITCH), start_tick, grid.beat, grid) {
        Some(index) => Edited::new(Some(index), Outcome::Added),
        None => Edited::new(None, Outcome::NoRoom),
    }
}

fn insert(
    notes: &mut Vec<Note>,
    pitch: u8,
    start_tick: u64,
    duration_ticks: u64,
    grid: EditGrid,
) -> Option<usize> {
    let beyond_pattern = grid
        .pattern_length
        .is_some_and(|length| start_tick >= length);
    if beyond_pattern || start_tick.checked_add(duration_ticks).is_none() {
        return None;
    }
    notes.push(Note {
        pitch,
        velocity: DEFAULT_NOTE_VELOCITY,
        start_tick,
        duration_ticks,
    });
    Some(notes.len() - 1)
}

/// The pointer selecting a note, or clicking empty space with nothing to add.
pub fn select(notes: &[Note], index: Option<usize>) -> Edited {
    Edited::new(index.filter(|&index| index < notes.len()), Outcome::Moved)
}

/// The pointer dragging a note to a pitch and a snapped start.
pub fn move_to(
    notes: &mut [Note],
    index: usize,
    pitch: u8,
    start_tick: u64,
    grid: EditGrid,
) -> Edited {
    let Some(note) = notes.get_mut(index) else {
        return Edited::new(None, Outcome::NoSelection);
    };
    let start_tick =
        latest_start(grid, note.duration_ticks).map_or(start_tick, |latest| start_tick.min(latest));
    let pitch = pitch.min(MAX_PITCH);
    if note.pitch == pitch && note.start_tick == start_tick {
        return Edited::new(Some(index), Outcome::Moved);
    }
    note.pitch = pitch;
    note.start_tick = start_tick;
    Edited::new(Some(index), Outcome::Edited)
}

/// The pointer dragging a note's end to a snapped tick; never shorter than one snap.
pub fn resize_to(notes: &mut [Note], index: usize, end_tick: u64, grid: EditGrid) -> Edited {
    let Some(note) = notes.get_mut(index) else {
        return Edited::new(None, Outcome::NoSelection);
    };
    let duration = end_tick
        .saturating_sub(note.start_tick)
        .max(grid.snap.max(1));
    if note.duration_ticks == duration {
        return Edited::new(Some(index), Outcome::Moved);
    }
    note.duration_ticks = duration;
    Edited::new(Some(index), Outcome::Edited)
}

/// The last start a note may have: the pattern's last snap, or where its end would overflow.
fn latest_start(grid: EditGrid, duration_ticks: u64) -> Option<u64> {
    let snap = grid.snap.max(1);
    match grid.pattern_length {
        Some(length) => Some((length.saturating_sub(1) / snap) * snap),
        None => Some(u64::MAX - duration_ticks),
    }
}

fn edit_note(note: &mut Note, command: RollCommand, grid: EditGrid) -> Outcome {
    let snap = grid.snap.max(1);
    match command {
        RollCommand::PitchUp | RollCommand::OctaveUp => {
            if note.pitch >= MAX_PITCH {
                return Outcome::AlreadyHighest;
            }
            let step = if command == RollCommand::OctaveUp {
                12
            } else {
                1
            };
            note.pitch = note.pitch.saturating_add(step).min(MAX_PITCH);
        }
        RollCommand::PitchDown | RollCommand::OctaveDown => {
            if note.pitch == 0 {
                return Outcome::AlreadyLowest;
            }
            let step = if command == RollCommand::OctaveDown {
                12
            } else {
                1
            };
            note.pitch = note.pitch.saturating_sub(step);
        }
        RollCommand::Earlier => {
            if note.start_tick == 0 {
                return Outcome::AlreadyAtStart;
            }
            note.start_tick = note.start_tick.saturating_sub(snap);
        }
        RollCommand::Later => {
            let latest = latest_start(grid, note.duration_ticks).unwrap_or(note.start_tick);
            if note.start_tick >= latest {
                return Outcome::AlreadyAtEnd;
            }
            note.start_tick = note.start_tick.saturating_add(snap).min(latest);
        }
        RollCommand::Shorter => {
            if note.duration_ticks <= snap {
                return Outcome::AlreadyShortest;
            }
            note.duration_ticks = (note.duration_ticks - snap).max(snap);
        }
        RollCommand::Longer => {
            let Some(longer) = note
                .duration_ticks
                .checked_add(snap)
                .filter(|longer| note.start_tick.checked_add(*longer).is_some())
            else {
                return Outcome::AlreadyLongest;
            };
            note.duration_ticks = longer;
        }
        RollCommand::Previous
        | RollCommand::Next
        | RollCommand::First
        | RollCommand::Last
        | RollCommand::Add
        | RollCommand::Delete
        | RollCommand::ClearSelection
        | RollCommand::ReadTransport => return Outcome::Moved,
    }
    Outcome::Edited
}

#[cfg(test)]
mod tests;
