//! What the application says. Every string a screen reader, the status line or a later web
//! bridge reads about a note, the roll or the transport is made here, from the model and a
//! stated origin, so no surface words the same thing differently.

use evil_project::{Note, Project};

use crate::piano_roll::edits::{ChordModifier, Outcome, RollCommand};

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
}

pub fn unavailable_reason(reason: Unavailable) -> &'static str {
    match reason {
        Unavailable::NoNoteSelected => "Select a note first.",
        Unavailable::NothingToUndo => "Nothing to undo.",
        Unavailable::NothingToRedo => "Nothing to redo.",
        Unavailable::WhileRecording => "Not while recording.",
        Unavailable::NoTrack => "Add a track first.",
        Unavailable::NoOtherSound => "No other sound in this list.",
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

/// "Track 2, Bass, 12 unique notes, 3 parts, muted": an arranger lane.
pub fn describe_track(
    index: usize,
    name: &str,
    note_count: usize,
    part_count: usize,
    muted: bool,
) -> String {
    let notes = if note_count == 1 { "note" } else { "notes" };
    let parts = if part_count == 1 { "part" } else { "parts" };
    format!(
        "Track {}, {name}, {note_count} unique {notes}, {part_count} {parts}{}",
        index + 1,
        if muted { ", muted" } else { "" }
    )
}

/// "Part, Pattern 1, bar 3 beat 1, 4 beats": an arranger part, where it sits in the song.
pub fn describe_part(
    pattern_name: &str,
    start_tick: u64,
    length_ticks: u64,
    meter: Meter,
) -> String {
    format!(
        "Part, {pattern_name}, {}, {}",
        position(start_tick, meter, Origin::Song),
        length(length_ticks, meter)
    )
}

/// The roll's accessible description: every chord of its keyboard model, read once on focus.
/// `command_key` is what the platform calls the key that `Modifiers::command` means.
pub fn roll_key_help(command_key: &str) -> String {
    let chords: Vec<String> = RollCommand::ALL
        .into_iter()
        .map(|command| {
            let (modifier, key) = command.chord();
            let modifier = match modifier {
                ChordModifier::None => String::new(),
                ChordModifier::Shift => "Shift+".to_owned(),
                ChordModifier::Command => format!("{command_key}+"),
            };
            format!("{modifier}{} {}", key.name(), command_action(command))
        })
        .collect();
    format!(
        "{}. F6 and Shift+F6 move between the toolbar, the side panel and the editor; Tab leaves \
         the roll.",
        chords.join(", ")
    )
}

/// The name of the key that `Modifiers::command` stands for on this platform.
pub fn platform_command_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "Command"
    } else {
        "Control"
    }
}

fn command_action(command: RollCommand) -> &'static str {
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

/// A number with at most three decimals and no trailing zeros: 1, 2.5, 1.333.
fn decimal(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMON_TIME: Meter = Meter {
        ticks_per_quarter: 480,
        beats_per_bar: 4,
        beat_unit: 4,
    };

    const SIX_EIGHT: Meter = Meter {
        ticks_per_quarter: 480,
        beats_per_bar: 6,
        beat_unit: 8,
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
    fn pitch_names_cover_the_midi_range_with_middle_c_as_c_4() {
        assert_eq!(pitch_name(60), "C 4");
        assert_eq!(pitch_name(61), "C sharp 4");
        assert_eq!(pitch_name(0), "C minus 1");
        assert_eq!(pitch_name(127), "G 9");
    }

    #[test]
    fn a_note_in_the_song_is_described_by_bar_beat_length_and_velocity() {
        let described = describe_note(&note_at(64, 1_920 + 720, 240), COMMON_TIME, Origin::Song);
        assert_eq!(described, "E 4, bar 2 beat 2.5, 0.5 beats, velocity 100");
    }

    #[test]
    fn six_eight_counts_eighth_note_beats_in_six_beat_bars() {
        assert_eq!(SIX_EIGHT.bar_ticks(), 1_440);
        assert_eq!(
            position(1_440 + 240, SIX_EIGHT, Origin::Song),
            "bar 2 beat 2"
        );
        assert_eq!(position(1_200, SIX_EIGHT, Origin::Song), "bar 1 beat 6");
        assert_eq!(length(240, SIX_EIGHT), "1 beat");
        assert_eq!(length(480, SIX_EIGHT), "2 beats");
    }

    #[test]
    fn a_shared_pattern_counts_from_the_pattern_and_an_edit_says_how_far_it_reaches() {
        let origin = Origin::Pattern { sharing_parts: 3 };
        let note = note_at(62, 960, 480);
        assert_eq!(
            roll_name(Some(&note), 1, COMMON_TIME, origin),
            "Piano roll, D 4, pattern bar 1 beat 3, 1 beat, velocity 100"
        );
        assert_eq!(
            describe_outcome(Outcome::Deleted, origin).as_deref(),
            Some("Removed from 3 parts.")
        );
        assert_eq!(
            describe_outcome(Outcome::Edited, origin).as_deref(),
            Some("Changed in 3 parts.")
        );
        assert_eq!(
            describe_outcome(Outcome::Added, Origin::Pattern { sharing_parts: 1 }).as_deref(),
            Some("Added.")
        );
        assert_eq!(describe_outcome(Outcome::Edited, Origin::Song), None);
    }

    #[test]
    fn the_name_carries_state_only_and_never_an_outcome() {
        let note = note_at(60, 0, 480);
        assert_eq!(
            roll_name(None, 0, COMMON_TIME, Origin::Song),
            "Piano roll, empty"
        );
        assert_eq!(
            roll_name(None, 4, COMMON_TIME, Origin::Song),
            "Piano roll, no note selected"
        );
        let name = roll_name(Some(&note), 1, COMMON_TIME, Origin::Song).to_lowercase();
        for word in [
            "deleted", "removed", "already", "added", "cannot", "nothing",
        ] {
            assert!(!name.contains(word), "{word:?} in {name:?}");
        }
    }

    #[test]
    fn every_outcome_that_changes_no_note_has_words_of_its_own() {
        for outcome in [
            Outcome::AlreadyFirst,
            Outcome::AlreadyLast,
            Outcome::AlreadyHighest,
            Outcome::AlreadyLowest,
            Outcome::AlreadyAtStart,
            Outcome::AlreadyAtEnd,
            Outcome::AlreadyShortest,
            Outcome::AlreadyLongest,
            Outcome::NoRoom,
            Outcome::NoSelection,
            Outcome::Empty,
            Outcome::Cleared,
            Outcome::SelectionReset,
        ] {
            assert!(
                describe_outcome(outcome, Origin::Song).is_some(),
                "{outcome:?} would be silent"
            );
        }
        assert_eq!(
            describe_outcome(Outcome::Cleared, Origin::Song).as_deref(),
            Some("No note selected; Quantize, transpose and Copy apply to all notes.")
        );
    }

    #[test]
    fn a_lane_says_what_it_automates_and_how_many_points_it_holds() {
        assert_eq!(
            describe_lane("Automation lane", "Pitch bend", 1),
            "Automation lane, Pitch bend, 1 point"
        );
        assert_eq!(
            describe_lane("Mixer automation lane", "Channel gain", 0),
            "Mixer automation lane, Channel gain, 0 points"
        );
    }

    #[test]
    fn the_transport_says_its_state_tempo_meter_and_position_in_bars_and_in_time() {
        let mut transport = TransportState {
            playing: PlayingState::Stopped,
            tempo_beats_per_minute: 64.0,
            meter: COMMON_TIME,
            position_tick: 0,
            position_milliseconds: 0,
        };
        assert_eq!(
            describe_transport(&transport),
            "Transport, stopped, tempo 64 beats per minute, 4 4 time, bar 1 beat 1, \
             position 0 minutes, 0 seconds"
        );
        transport.playing = PlayingState::Playing;
        transport.tempo_beats_per_minute = 120.5;
        transport.position_tick = 1_920 + 480 + 300;
        transport.position_milliseconds = 61_900;
        assert_eq!(
            describe_transport(&transport),
            "Transport, playing, tempo 120.5 beats per minute, 4 4 time, bar 2 beat 2, \
             position 1 minute, 1 second"
        );
        transport.meter = SIX_EIGHT;
        transport.position_tick = 1_440 + 5 * 240;
        assert!(describe_transport(&transport).contains("6 8 time, bar 2 beat 6,"));
    }

    #[test]
    fn every_unavailable_control_has_a_reason() {
        for reason in [
            Unavailable::NoNoteSelected,
            Unavailable::NothingToUndo,
            Unavailable::NothingToRedo,
            Unavailable::WhileRecording,
            Unavailable::NoTrack,
            Unavailable::NoOtherSound,
        ] {
            assert!(!unavailable_reason(reason).is_empty(), "{reason:?}");
        }
        assert_eq!(
            Some(unavailable_reason(Unavailable::NoNoteSelected)),
            describe_outcome(Outcome::NoSelection, Origin::Song).as_deref(),
            "a disabled one-note button and its chord give the same reason"
        );
    }

    #[test]
    fn a_sound_says_where_it_is_in_the_list_and_an_arranger_lane_and_part_say_what_they_hold() {
        assert_eq!(
            describe_sound("Bass", Some(3), 12, "Synth"),
            "Bass, 4 of 12 synth sounds"
        );
        assert_eq!(
            describe_sound("Mine", None, 1, "Drum kit"),
            "Mine, not one of the 1 drum kit sound listed"
        );
        assert_eq!(
            describe_track(1, "Bass", 1, 3, true),
            "Track 2, Bass, 1 unique note, 3 parts, muted"
        );
        assert_eq!(
            describe_part("Pattern 1", 1_920 * 2, 1_920, COMMON_TIME),
            "Part, Pattern 1, bar 3 beat 1, 4 beats"
        );
    }

    #[test]
    fn the_key_help_names_every_chord_of_the_model_with_the_platform_command_key() {
        for command_key in ["Control", "Command"] {
            let help = roll_key_help(command_key);
            for command in RollCommand::ALL {
                let (modifier, key) = command.chord();
                let chord = match modifier {
                    ChordModifier::None => key.name().to_owned(),
                    ChordModifier::Shift => format!("Shift+{}", key.name()),
                    ChordModifier::Command => format!("{command_key}+{}", key.name()),
                };
                assert!(help.contains(&chord), "{chord:?} missing from {help:?}");
            }
            for chord in ["Home", "End", "Shift+Up", "Shift+Down", "Escape", "F6"] {
                assert!(help.contains(chord), "{chord:?} missing from {help:?}");
            }
        }
        assert!(!roll_key_help("Command").contains("Control"));
    }
}
