//! The arranger's commands and their edits. Every way to change the arrangement — a key, a
//! button, the part inspector, a pointer gesture — is one of these commands, and each is a
//! function of the project, the cursor and the transport that returns the new cursor and what
//! happened, so the cursor is never guessed after an edit the arranger made itself.

use eframe::egui;
use evil_project::{Part, Pattern, Project};

use super::cursor::{self, SongCursor, Walk};
use crate::key_help::ChordModifier;
use crate::speech::Meter;
use crate::transport::Transport;

/// One command of the arranger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SongCommand {
    Previous,
    Next,
    TrackAbove,
    TrackBelow,
    First,
    Last,
    Earlier,
    Later,
    Open,
    Delete,
    StartHere,
    ReadTransport,
    AddPatternPart,
    DuplicateLoop,
    UniqueTimeline,
    LoopPart,
    DuplicateShared,
    MakeUnique,
    MergeToTimeline,
    /// The pointer chose a track's timeline or one of its parts.
    Select(SongCursor),
    /// The part's start, from the inspector or a dropped drag, in ticks.
    MoveTo(u64),
    /// The part's length, from the inspector, in ticks.
    Resize(u64),
    /// The ruler was clicked on this bar, counted from 0.
    StartAtBar(u64),
}

impl SongCommand {
    /// The commands a key reaches, in the order the help lists them.
    pub const WITH_CHORDS: [Self; 12] = [
        Self::Previous,
        Self::Next,
        Self::TrackAbove,
        Self::TrackBelow,
        Self::First,
        Self::Last,
        Self::Earlier,
        Self::Later,
        Self::Open,
        Self::Delete,
        Self::StartHere,
        Self::ReadTransport,
    ];

    /// The one chord bound to this command, if a key reaches it.
    pub fn chord(self) -> Option<(ChordModifier, egui::Key)> {
        use ChordModifier::{Command, None};
        use egui::Key;
        Some(match self {
            Self::Previous => (None, Key::ArrowLeft),
            Self::Next => (None, Key::ArrowRight),
            Self::TrackAbove => (None, Key::ArrowUp),
            Self::TrackBelow => (None, Key::ArrowDown),
            Self::First => (None, Key::Home),
            Self::Last => (None, Key::End),
            Self::Earlier => (Command, Key::ArrowLeft),
            Self::Later => (Command, Key::ArrowRight),
            Self::Open => (None, Key::Enter),
            Self::Delete => (None, Key::Delete),
            // The ruler's click, from the keyboard: B for bar, a plain letter no browser keeps.
            Self::StartHere => (None, Key::B),
            Self::ReadTransport => (None, Key::I),
            _ => return Option::None,
        })
    }

    /// The command a key press means, if the arranger takes it. Modifiers must match exactly.
    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        let modifier = ChordModifier::of_editor_press(modifiers)?;
        Self::WITH_CHORDS
            .into_iter()
            .find(|command| command.chord() == Some((modifier, key)))
    }

    /// The moves and the nudges, which a held key repeats; a held Delete must not empty a track.
    pub fn repeats_when_held(self) -> bool {
        matches!(
            self,
            Self::Previous
                | Self::Next
                | Self::TrackAbove
                | Self::TrackBelow
                | Self::Earlier
                | Self::Later
        )
    }

    /// The commands that add, remove or move parts. A take holds its target by index, so these
    /// are the ones the command boundary refuses while recording.
    pub fn changes_structure(self) -> bool {
        matches!(
            self,
            Self::Earlier
                | Self::Later
                | Self::Delete
                | Self::AddPatternPart
                | Self::DuplicateLoop
                | Self::DuplicateShared
                | Self::MakeUnique
                | Self::MergeToTimeline
                | Self::MoveTo(_)
                | Self::Resize(_)
        )
    }
}

/// What a command did. Every outcome that leaves the cursor's name unchanged has words of its own
/// in `speech`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SongOutcome {
    Moved,
    Edited,
    AlreadyFirst,
    AlreadyLast,
    NoTrackAbove,
    NoTrackBelow,
    AlreadyAtStart,
    NoPart,
    Removed,
    Opened,
    PartAdded { pattern: String },
    SectionDuplicated { copied: usize },
    Looping { start_tick: u64, end_tick: u64 },
    PlaybackStartsAt { bar: u64 },
    PartDuplicated { sharing: usize, pattern: String },
    MadeUnique { pattern: String },
    Merged,
    Failed(String),
}

/// The cursor after a command, what happened, and whether the roll should now show the cursor's
/// timeline or part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongEdited {
    pub cursor: SongCursor,
    pub outcome: SongOutcome,
    pub open_in_roll: bool,
}

impl SongEdited {
    fn new(cursor: SongCursor, outcome: SongOutcome) -> Self {
        Self {
            cursor,
            outcome,
            open_in_roll: false,
        }
    }
}

/// Applies one command at the cursor. Reading the transport changes nothing and is the caller's
/// to announce.
pub fn apply(
    project: &mut Project,
    cursor: SongCursor,
    command: SongCommand,
    transport: &mut Transport,
) -> SongEdited {
    let walked = |walk: Walk| match walk {
        Walk::Moved(next) => SongEdited::new(next, SongOutcome::Moved),
        Walk::AlreadyFirst => SongEdited::new(cursor, SongOutcome::AlreadyFirst),
        Walk::AlreadyLast => SongEdited::new(cursor, SongOutcome::AlreadyLast),
        Walk::NoTrackAbove => SongEdited::new(cursor, SongOutcome::NoTrackAbove),
        Walk::NoTrackBelow => SongEdited::new(cursor, SongOutcome::NoTrackBelow),
    };
    let bar = Meter::of(project).bar_ticks();
    match command {
        SongCommand::Previous => walked(cursor::previous(project, cursor)),
        SongCommand::Next => walked(cursor::next(project, cursor)),
        SongCommand::TrackAbove => walked(cursor::above(project, cursor)),
        SongCommand::TrackBelow => walked(cursor::below(project, cursor)),
        SongCommand::First => walked(cursor::first(cursor)),
        SongCommand::Last => walked(cursor::last(project, cursor)),
        SongCommand::ReadTransport => SongEdited::new(cursor, SongOutcome::Moved),
        SongCommand::Select(chosen) => SongEdited::new(
            cursor::kept_inside(project, chosen).unwrap_or(cursor),
            SongOutcome::Moved,
        ),
        SongCommand::Open => SongEdited {
            open_in_roll: true,
            ..SongEdited::new(cursor, SongOutcome::Opened)
        },
        SongCommand::UniqueTimeline => SongEdited {
            open_in_roll: true,
            ..SongEdited::new(
                SongCursor {
                    part: None,
                    ..cursor
                },
                SongOutcome::Opened,
            )
        },
        SongCommand::StartAtBar(bar_index) => start_at_bar(cursor, bar_index, transport, bar),
        SongCommand::AddPatternPart => add_pattern_part(project, cursor, transport.start_tick),
        SongCommand::DuplicateLoop => {
            match project.duplicate_section(transport.start_tick, transport.end_tick) {
                Ok(copied) => SongEdited::new(cursor, SongOutcome::SectionDuplicated { copied }),
                Err(error) => failed(cursor, "duplicate the loop", error),
            }
        }
        _ => match cursor.part {
            Some(part) => part_command(project, cursor, part, command, transport, bar),
            None => SongEdited::new(cursor, SongOutcome::NoPart),
        },
    }
}

/// A command that acts on the part under the cursor.
fn part_command(
    project: &mut Project,
    cursor: SongCursor,
    part_index: usize,
    command: SongCommand,
    transport: &mut Transport,
    bar: u64,
) -> SongEdited {
    let track = cursor.track;
    let part = project.tracks[track].parts[part_index];
    let moved = |project: &mut Project, start_tick: u64| match project
        .move_part(track, part_index, start_tick)
    {
        Ok(()) => SongEdited::new(cursor, SongOutcome::Edited),
        Err(error) => failed(cursor, "move the part", error),
    };
    match command {
        SongCommand::Earlier if part.start_tick == 0 => {
            SongEdited::new(cursor, SongOutcome::AlreadyAtStart)
        }
        SongCommand::Earlier => moved(project, part.start_tick.saturating_sub(bar)),
        SongCommand::Later => match part.start_tick.checked_add(bar) {
            Some(start_tick) => moved(project, start_tick),
            None => failed(cursor, "move the part", "it would end past the song's end"),
        },
        SongCommand::MoveTo(start_tick) if start_tick == part.start_tick => {
            SongEdited::new(cursor, SongOutcome::Moved)
        }
        SongCommand::MoveTo(start_tick) => moved(project, start_tick),
        SongCommand::Resize(length_ticks) => {
            match project.resize_part(track, part_index, length_ticks) {
                Ok(()) => SongEdited::new(cursor, SongOutcome::Edited),
                Err(error) => failed(cursor, "resize the part", error),
            }
        }
        SongCommand::Delete => {
            let next = cursor::after_removal(&project.tracks[track], part_index);
            match project.remove_part(track, part_index) {
                Ok(_) => SongEdited::new(SongCursor { track, part: next }, SongOutcome::Removed),
                Err(error) => failed(cursor, "remove the part", error),
            }
        }
        SongCommand::StartHere => start_at_bar(cursor, part.start_tick / bar, transport, bar),
        SongCommand::LoopPart => {
            transport.start_tick = part.start_tick;
            transport.end_tick = part.start_tick + part.length_ticks;
            transport.repeat = true;
            SongEdited::new(
                cursor,
                SongOutcome::Looping {
                    start_tick: transport.start_tick,
                    end_tick: transport.end_tick,
                },
            )
        }
        SongCommand::DuplicateShared => match project.duplicate_part(track, part_index) {
            Ok(index) => SongEdited::new(
                SongCursor {
                    track,
                    part: Some(index),
                },
                SongOutcome::PartDuplicated {
                    sharing: cursor::parts_sharing(project, part.pattern),
                    pattern: project.patterns[part.pattern].name.clone(),
                },
            ),
            Err(error) => failed(cursor, "duplicate the part", error),
        },
        SongCommand::MakeUnique => match project.make_part_unique(track, part_index) {
            Ok(pattern) => SongEdited::new(
                cursor,
                SongOutcome::MadeUnique {
                    pattern: project.patterns[pattern].name.clone(),
                },
            ),
            Err(error) => failed(cursor, "make the part unique", error),
        },
        SongCommand::MergeToTimeline => match project.merge_part_into_track(track, part_index) {
            Ok(()) => SongEdited::new(SongCursor { track, part: None }, SongOutcome::Merged),
            Err(error) => failed(cursor, "merge the part", error),
        },
        _ => SongEdited::new(cursor, SongOutcome::Moved),
    }
}

/// The ruler's click: playback starts at the bar and loops the bar after it, wherever the
/// cursor is. The key reaches it through the cursor's part.
fn start_at_bar(
    cursor: SongCursor,
    bar_index: u64,
    transport: &mut Transport,
    bar: u64,
) -> SongEdited {
    transport.set_bars(bar_index + 1, bar_index + 2, bar);
    SongEdited::new(cursor, SongOutcome::PlaybackStartsAt { bar: bar_index + 1 })
}

fn add_pattern_part(project: &mut Project, cursor: SongCursor, start_tick: u64) -> SongEdited {
    let bar = Meter::of(project).bar_ticks();
    let name = format!("Pattern {}", project.patterns.len() + 1);
    let pattern = Pattern {
        name: name.clone(),
        length_ticks: bar,
        ..Pattern::default()
    };
    let added = project.add_pattern(pattern).and_then(|pattern| {
        project.add_part(
            cursor.track,
            Part {
                pattern,
                start_tick,
                length_ticks: bar,
            },
        )
    });
    match added {
        Ok(part) => SongEdited::new(
            SongCursor {
                track: cursor.track,
                part: Some(part),
            },
            SongOutcome::PartAdded { pattern: name },
        ),
        Err(error) => failed(cursor, "add a pattern part", error),
    }
}

fn failed(cursor: SongCursor, what: &str, error: impl std::fmt::Display) -> SongEdited {
    SongEdited::new(
        cursor,
        SongOutcome::Failed(format!("Could not {what}: {error}.")),
    )
}

#[cfg(test)]
mod tests;
