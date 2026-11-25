//! What the application says. Every string a screen reader, the status line or a later web
//! bridge reads about a note, the roll or the transport is made here, from the model and a
//! stated origin, so no surface words the same thing differently.

use evil_project::{Note, Project};

use crate::arranger::cursor::{self as arranger_cursor, SongCursor};
use crate::arranger::edits::{SongCommand, SongOutcome};
use crate::key_help::{self, AppCommand};
use crate::piano_roll::edits::{Outcome, RollCommand};

const PITCH_CLASS_NAMES: [&str; 12] = [
    "C", "C sharp", "D", "D sharp", "E", "F", "F sharp", "G", "G sharp", "A", "A sharp", "B",
];

/// The project's meter, which turns ticks into bars and beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    pub ticks_per_quarter: u16,
    pub beats_per_bar: u32,
    pub beat_unit: u32,
}

impl Meter {
    pub fn of(project: &Project) -> Self {
        Self {
            ticks_per_quarter: project.ticks_per_quarter,
            beats_per_bar: project.beats_per_bar,
            beat_unit: project.beat_unit,
        }
    }

    /// One beat of the meter: an eighth in 6/8, a quarter in 4/4.
    pub fn beat_ticks(self) -> u64 {
        (u64::from(self.ticks_per_quarter.max(1)) * 4 / u64::from(self.beat_unit.max(1))).max(1)
    }

    pub fn bar_ticks(self) -> u64 {
        (u64::from(self.ticks_per_quarter.max(1)) * 4 * u64::from(self.beats_per_bar.max(1))
            / u64::from(self.beat_unit.max(1)))
        .max(1)
    }
}

/// Where a note's position is counted from. A shared pattern sounds at several song positions,
/// so its notes are described within the pattern, never at one of those positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Song,
    Pattern { sharing_parts: usize },
}

/// "C sharp 4": middle C is C 4.
pub fn pitch_name(pitch: u8) -> String {
    let octave = i32::from(pitch) / 12 - 1;
    let octave_text = if octave < 0 {
        format!("minus {}", -octave)
    } else {
        octave.to_string()
    };
    format!(
        "{} {octave_text}",
        PITCH_CLASS_NAMES[usize::from(pitch % 12)]
    )
}

/// "bar 3 beat 2.5", or "pattern bar 1 beat 3" in a shared pattern.
pub fn position(tick: u64, meter: Meter, origin: Origin) -> String {
    let bar_ticks = meter.bar_ticks();
    let bar = tick / bar_ticks + 1;
    let beat = 1.0 + (tick % bar_ticks) as f64 / meter.beat_ticks() as f64;
    let prefix = match origin {
        Origin::Song => "bar",
        Origin::Pattern { .. } => "pattern bar",
    };
    format!("{prefix} {bar} beat {}", decimal(beat))
}

/// "1 beat", "0.5 beats".
pub fn length(duration_ticks: u64, meter: Meter) -> String {
    let text = decimal(duration_ticks as f64 / meter.beat_ticks() as f64);
    if text == "1" {
        "1 beat".to_owned()
    } else {
        format!("{text} beats")
    }
}

/// "E 4, bar 2 beat 1, 0.5 beats, velocity 90".
pub fn describe_note(note: &Note, meter: Meter, origin: Origin) -> String {
    format!(
        "{}, {}, {}, velocity {}",
        pitch_name(note.pitch),
        position(note.start_tick, meter, origin),
        length(note.duration_ticks, meter),
        note.velocity
    )
}

/// The roll's accessible name: what the cursor is on now, and nothing else. What a key did is
/// an outcome, announced separately, so the name never says "deleted" or "already".
pub fn roll_name(cursor: Option<&Note>, note_count: usize, meter: Meter, origin: Origin) -> String {
    let state = match cursor {
        Some(note) => describe_note(note, meter, origin),
        None if note_count == 0 => "empty".to_owned(),
        None => "no note selected".to_owned(),
    };
    format!("Piano roll, {state}")
}

/// What one edit did, for the polite announcement. `None` when the roll's name alone says it.
/// An edit to a pattern that several parts share says how far it reached, because a listener
/// cannot see the repeated blocks.
pub fn describe_outcome(outcome: Outcome, origin: Origin) -> Option<String> {
    let reach = match origin {
        Origin::Pattern { sharing_parts } if sharing_parts > 1 => Some(sharing_parts),
        _ => None,
    };
    let text = match (outcome, reach) {
        (Outcome::Moved, _) | (Outcome::Edited, None) => return None,
        (Outcome::Edited, Some(parts)) => format!("Changed in {parts} parts."),
        (Outcome::Added, None) => "Added.".to_owned(),
        (Outcome::Added, Some(parts)) => format!("Added to {parts} parts."),
        (Outcome::Deleted, None) => "Removed.".to_owned(),
        (Outcome::Deleted, Some(parts)) => format!("Removed from {parts} parts."),
        (Outcome::AlreadyFirst, _) => "Nothing before this one.".to_owned(),
        (Outcome::AlreadyLast, _) => "Nothing after this one.".to_owned(),
        (Outcome::AlreadyHighest, _) => "Cannot go higher.".to_owned(),
        (Outcome::AlreadyLowest, _) => "Cannot go lower.".to_owned(),
        (Outcome::AlreadyAtStart, _) => "Cannot move earlier.".to_owned(),
        (Outcome::AlreadyAtEnd, _) => "Cannot move later.".to_owned(),
        (Outcome::AlreadyShortest, _) => "Cannot be shorter.".to_owned(),
        (Outcome::AlreadyLongest, _) => "Cannot be longer.".to_owned(),
        (Outcome::NoRoom, _) => "Nothing added: the pattern ends here.".to_owned(),
        (Outcome::NoSelection, _) => unavailable_reason(Unavailable::NoNoteSelected).to_owned(),
        (Outcome::Empty, _) => "Nothing here yet.".to_owned(),
        (Outcome::Cleared, _) => {
            "No note selected; Quantize, transpose and Copy apply to all notes.".to_owned()
        }
        (Outcome::SelectionReset, _) => "The selection was reset.".to_owned(),
    };
    Some(text)
}

/// "Automation lane, Pitch bend, 3 points": a lane's accessible name, what it automates and how
/// many points it holds, so a point removed or kept can be heard.
pub fn describe_lane(kind: &str, automates: &str, point_count: usize) -> String {
    let points = if point_count == 1 { "point" } else { "points" };
    format!("{kind}, {automates}, {point_count} {points}")
}

/// What the transport is doing, for the readout and for a person asking from inside an editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayingState {
    Stopped,
    Rendering,
    Playing,
    Recording,
}

impl PlayingState {
    pub fn word(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Rendering => "rendering",
            Self::Playing => "playing",
            Self::Recording => "recording",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportState {
    pub playing: PlayingState,
    pub tempo_beats_per_minute: f32,
    pub meter: Meter,
    /// Where the playhead is in the song.
    pub position_tick: u64,
    /// How long the song has been playing, as the clock beside the readout shows it.
    pub position_milliseconds: u128,
}

/// "bar 3 beat 2": the whole beat the playhead is in, which is what a listener counts.
pub fn bar_and_beat(tick: u64, meter: Meter) -> (u64, u64) {
    let bar_ticks = meter.bar_ticks();
    (
        tick / bar_ticks + 1,
        tick % bar_ticks / meter.beat_ticks() + 1,
    )
}

/// "Transport, playing, tempo 120 beats per minute, 4 4 time, bar 3 beat 2, position 1 minute,
/// 5 seconds".
pub fn describe_transport(transport: &TransportState) -> String {
    let total_seconds = transport.position_milliseconds / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let (bar, beat) = bar_and_beat(transport.position_tick, transport.meter);
    format!(
        "Transport, {}, tempo {} beats per minute, {} {} time, bar {bar} beat {beat}, \
         position {minutes} {}, {seconds} {}",
        transport.playing.word(),
        decimal(f64::from(transport.tempo_beats_per_minute)),
        transport.meter.beats_per_bar,
        transport.meter.beat_unit,
        if minutes == 1 { "minute" } else { "minutes" },
        if seconds == 1 { "second" } else { "seconds" },
    )
}

/// The transport readout's description: fixed help, never state, because a focused node whose
/// description changes is spoken at every change.
pub const TRANSPORT_READOUT_HELP: &str = "Enter reads the transport again";

/// Why a control is unavailable, said by its description and its hover text while it is: a
/// screen reader is never told that a button is disabled, so the reason is how it learns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    NoNoteSelected,
    NothingToUndo,
    NothingToRedo,
    WhileRecording,
    NoTrack,
    NoOtherSound,
    NoPartSelected,
}

pub fn unavailable_reason(reason: Unavailable) -> &'static str {
    match reason {
        Unavailable::NoNoteSelected => "Select a note first.",
        Unavailable::NothingToUndo => "Nothing to undo.",
        Unavailable::NothingToRedo => "Nothing to redo.",
        Unavailable::WhileRecording => "Not while recording.",
        Unavailable::NoTrack => "Add a track first.",
        Unavailable::NoOtherSound => "No other sound in this list.",
        Unavailable::NoPartSelected => "Select a part first.",
    }
}

/// "Bass, 4 of 12 synth sounds": which sound a track has and where it is in the list that
/// Previous and Next walk. A sound the list does not hold says so.
pub fn describe_sound(name: &str, position: Option<usize>, count: usize, kind: &str) -> String {
    let kind = kind.to_lowercase();
    let noun = if count == 1 { "sound" } else { "sounds" };
    match position {
        Some(index) => format!("{name}, {} of {count} {kind} {noun}", index + 1),
        None => format!("{name}, not one of the {count} {kind} {noun} listed"),
    }
}

/// "track 1 of 2, Lead, muted, sound Lead": which track the arranger's cursor is on.
pub fn describe_track(index: usize, count: usize, name: &str, muted: bool, sound: &str) -> String {
    format!(
        "track {} of {count}, {name}{}, sound {sound}",
        index + 1,
        if muted { ", muted" } else { "" }
    )
}

/// "Added Track 2, a synth track with the Lead sound."
pub fn track_added(name: &str, kind: &str, sound: &str) -> String {
    format!("Added {name}, a {kind} track with the {sound} sound.")
}

/// "Removed Track 2; 1 track left."
pub fn track_removed(name: &str, remaining: usize) -> String {
    let tracks = if remaining == 1 { "track" } else { "tracks" };
    format!("Removed {name}; {remaining} {tracks} left.")
}

/// One part as the arranger's cursor finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartPlace<'a> {
    /// Its place among the track's parts in the order they play, from 0.
    pub place: usize,
    pub part_count: usize,
    pub pattern: &'a str,
    pub start_tick: u64,
    pub length_ticks: u64,
    /// How many parts, on any track, play its pattern.
    pub sharing: usize,
}

/// "part 2 of 3, Pattern 1, bar 3 beat 1, 1 bar, shared by 2 parts": a part, where it sits in
/// the song and how far an edit of its pattern reaches.
pub fn describe_part(part: PartPlace<'_>, meter: Meter) -> String {
    let bars = meter.bar_ticks();
    let span = if part.length_ticks.is_multiple_of(bars) {
        let count = part.length_ticks / bars;
        format!("{count} {}", if count == 1 { "bar" } else { "bars" })
    } else {
        length(part.length_ticks, meter)
    };
    let sharing = if part.sharing > 1 {
        format!("shared by {} parts", part.sharing)
    } else {
        "the only part playing it".to_owned()
    };
    format!(
        "part {} of {}, {}, {}, {span}, {sharing}",
        part.place + 1,
        part.part_count,
        part.pattern,
        position(part.start_tick, meter, Origin::Song),
    )
}

/// The arranger's accessible name: the track the cursor is on and, on it, the part or the
/// unique timeline. A track whose unique timeline holds notes says so wherever the cursor is.
pub fn song_name(project: &Project, cursor: Option<SongCursor>, sound: &str) -> String {
    let Some(cursor) = cursor else {
        return "Song, no tracks".to_owned();
    };
    let track = &project.tracks[cursor.track];
    let notes = track.notes.len();
    let note_count = format!("{notes} {}", if notes == 1 { "note" } else { "notes" });
    let parts = track.parts.len();
    let part_count = match parts {
        0 => "no parts".to_owned(),
        1 => "1 part".to_owned(),
        count => format!("{count} parts"),
    };
    let place = match cursor.part {
        None if notes == 0 => format!("unique timeline, empty, {part_count}"),
        None => format!("unique timeline, {note_count}, {part_count}"),
        Some(index) => {
            let part = track.parts[index];
            let described = describe_part(
                PartPlace {
                    place: arranger_cursor::parts_in_time_order(track)
                        .iter()
                        .position(|&walked| walked == index)
                        .unwrap_or(0),
                    part_count: parts,
                    pattern: &project.patterns[part.pattern].name,
                    start_tick: part.start_tick,
                    length_ticks: part.length_ticks,
                    sharing: arranger_cursor::parts_sharing(project, part.pattern),
                },
                Meter::of(project),
            );
            if notes == 0 {
                described
            } else {
                format!("{described}; the unique timeline has {note_count}")
            }
        }
    };
    format!(
        "Song, {}, {place}",
        describe_track(
            cursor.track,
            project.tracks.len(),
            &track.name,
            track.muted,
            sound
        )
    )
}

/// What one arranger command did, for the polite announcement. `None` when the arranger's name
/// alone says it: a move of the cursor, or a part moved to where its name now places it.
pub fn describe_song_outcome(outcome: &SongOutcome, meter: Meter) -> Option<String> {
    let text = match outcome {
        SongOutcome::Moved | SongOutcome::Edited | SongOutcome::Opened => return None,
        SongOutcome::AlreadyFirst => return describe_outcome(Outcome::AlreadyFirst, Origin::Song),
        SongOutcome::AlreadyLast => return describe_outcome(Outcome::AlreadyLast, Origin::Song),
        SongOutcome::AlreadyAtStart => {
            return describe_outcome(Outcome::AlreadyAtStart, Origin::Song);
        }
        SongOutcome::NoTrackAbove => "No track above.".to_owned(),
        SongOutcome::NoTrackBelow => "No track below.".to_owned(),
        SongOutcome::NoPart => unavailable_reason(Unavailable::NoPartSelected).to_owned(),
        SongOutcome::Removed => return describe_outcome(Outcome::Deleted, Origin::Song),
        SongOutcome::PartAdded { pattern } => {
            format!("Added a part playing the new {pattern}.")
        }
        SongOutcome::SectionDuplicated { copied: 0 } => {
            "Nothing in the loop to duplicate.".to_owned()
        }
        SongOutcome::SectionDuplicated { copied } => format!(
            "Duplicated {copied} {} after the loop.",
            if *copied == 1 {
                "event or part"
            } else {
                "events and parts"
            }
        ),
        SongOutcome::Looping {
            start_tick,
            end_tick,
        } => format!(
            "Looping from {} to {}.",
            position(*start_tick, meter, Origin::Song),
            position(*end_tick, meter, Origin::Song)
        ),
        SongOutcome::PlaybackStartsAt { bar } => format!("Playback starts at bar {bar}."),
        SongOutcome::PartDuplicated { sharing, pattern } => {
            format!("Duplicated the part; {sharing} parts now play {pattern}.")
        }
        SongOutcome::MadeUnique { pattern } => {
            format!("This part now plays its own pattern, {pattern}.")
        }
        SongOutcome::Merged => "Merged into the unique timeline.".to_owned(),
        SongOutcome::Failed(error) => error.clone(),
    };
    Some(text)
}

/// "Pattern 1 of Track 1, in the piano roll": what the roll was opened on, said once focus has
/// arrived there.
pub fn opened_in_roll(track: &str, pattern: Option<&str>) -> String {
    match pattern {
        Some(pattern) => format!("{pattern} of {track}, in the piano roll."),
        None => format!("The unique timeline of {track}, in the piano roll."),
    }
}

/// The roll's accessible description: every chord of its keyboard model, read once on focus.
/// `command_key` is what the platform calls the key that `Modifiers::command` means.
pub fn roll_key_help(command_key: &str) -> String {
    key_help::editor_description(key_help::Surface::Roll, command_key)
}

/// The arranger's accessible description, from the same kind of table.
pub fn song_key_help(command_key: &str) -> String {
    key_help::editor_description(key_help::Surface::Song, command_key)
}

/// The name of the key that `Modifiers::command` stands for on this platform.
pub fn platform_command_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "Command"
    } else {
        "Control"
    }
}

pub fn roll_command_action(command: RollCommand) -> &'static str {
    match command {
        RollCommand::Previous => "previous note",
        RollCommand::Next => "next note",
        RollCommand::First => "first note",
        RollCommand::Last => "last note",
        RollCommand::PitchUp => "a semitone higher",
        RollCommand::PitchDown => "a semitone lower",
        RollCommand::OctaveUp => "an octave higher",
        RollCommand::OctaveDown => "an octave lower",
        RollCommand::Earlier => "moves the note earlier",
        RollCommand::Later => "moves the note later",
        RollCommand::Shorter => "shorter",
        RollCommand::Longer => "longer",
        RollCommand::Add => "adds a note",
        RollCommand::Delete => "removes the note",
        RollCommand::ClearSelection => "clears the selection",
        RollCommand::ReadTransport => "reads the transport",
    }
}

pub fn song_command_action(command: SongCommand) -> &'static str {
    match command {
        SongCommand::Previous => "previous part, then the track's unique timeline",
        SongCommand::Next => "next part",
        SongCommand::TrackAbove => "track above, at the same point in the song",
        SongCommand::TrackBelow => "track below, at the same point in the song",
        SongCommand::First => "the track's unique timeline",
        SongCommand::Last => "the track's last part",
        SongCommand::Earlier => "moves the part a bar earlier",
        SongCommand::Later => "moves the part a bar later",
        SongCommand::Open => "opens it in the piano roll",
        SongCommand::Delete => "removes the part",
        SongCommand::StartHere => "starts playback at the part's first bar",
        SongCommand::ReadTransport => "reads the transport",
        SongCommand::AddPatternPart => "adds a part with a new pattern",
        SongCommand::DuplicateLoop => "duplicates the loop",
        SongCommand::UniqueTimeline => "opens the track's unique timeline",
        SongCommand::LoopPart => "loops the part",
        SongCommand::DuplicateShared => "duplicates the part",
        SongCommand::MakeUnique => "gives the part its own pattern",
        SongCommand::MergeToTimeline => "merges the part into the unique timeline",
        SongCommand::Select(_) => "selects",
        SongCommand::MoveTo(_) => "moves the part",
        SongCommand::Resize(_) => "resizes the part",
        SongCommand::StartAtBar(_) => "starts playback at the bar",
    }
}

pub fn app_command_action(command: AppCommand) -> &'static str {
    match command {
        AppCommand::NewProject => "a new project",
        AppCommand::OpenProject => "opens a project",
        AppCommand::SaveProject => "saves the project",
        AppCommand::Undo => "undoes the last edit",
        AppCommand::Redo => "redoes it",
        AppCommand::PreviousSound => "the previous sound for the selected track",
        AppCommand::NextSound => "the next sound for the selected track",
        AppCommand::PlayOrStop => {
            "plays or stops the song, where the focused control does not take Space"
        }
        AppCommand::NextRegion => "the next region: the toolbar, the side panel, the editor",
        AppCommand::PreviousRegion => "the region before",
    }
}

/// A number with at most three decimals and no trailing zeros: 1, 2.5, 1.333.
fn decimal(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests;
