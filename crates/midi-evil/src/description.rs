//! What the piano roll says: one function per thing described, read by the roll's accessible
//! name, its notice, and the status line alike, so no surface words a note differently.

use evil_project::Note;

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

/// What one key in the roll did. Every outcome that changes no note carries a notice, because
/// an unchanged name is not spoken again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Moved,
    Edited,
    Added,
    Deleted(Note),
    AlreadyFirst,
    AlreadyLast,
    AlreadyHighest,
    AlreadyLowest,
    AlreadyAtStart,
    AlreadyAtEnd,
    AlreadyShortest,
    NoRoom,
    NoSelection,
    Empty,
    SelectionLost,
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
    let beat_ticks = meter.beat_ticks();
    let bar = tick / bar_ticks + 1;
    let beat = 1.0 + (tick % bar_ticks) as f64 / beat_ticks as f64;
    let prefix = match origin {
        Origin::Song => "bar",
        Origin::Pattern { .. } => "pattern bar",
    };
    format!("{prefix} {bar} beat {}", decimal(beat))
}

/// "1 beat", "0.5 beats".
pub fn length(duration_ticks: u64, meter: Meter) -> String {
    let beats = duration_ticks as f64 / meter.beat_ticks() as f64;
    let text = decimal(beats);
    if text == "1" {
        "1 beat".to_owned()
    } else {
        format!("{text} beats")
    }
}

/// "E 4, bar 2 beat 1, 0.5 beats, velocity 90".
pub fn note(note: &Note, meter: Meter, origin: Origin) -> String {
    format!(
        "{}, {}, {}, velocity {}",
        pitch_name(note.pitch),
        position(note.start_tick, meter, origin),
        length(note.duration_ticks, meter),
        note.velocity
    )
}

/// How far an edit to a shared pattern reaches; a listener cannot see the repeated blocks.
pub fn sharing(sharing_parts: usize) -> String {
    if sharing_parts == 1 {
        "used by 1 part".to_owned()
    } else {
        format!("shared by {sharing_parts} parts")
    }
}

/// The roll's accessible name: "Piano roll", then what the last key did when the note alone
/// does not say it, then the cursor's note.
pub fn roll_name(
    cursor: Option<&Note>,
    note_count: usize,
    outcome: Option<Outcome>,
    meter: Meter,
    origin: Origin,
) -> String {
    let sharing_parts = match origin {
        Origin::Song => None,
        Origin::Pattern { sharing_parts } => Some(sharing_parts),
    };
    let mut parts = vec!["Piano roll".to_owned()];
    let outcome_text = outcome.and_then(|outcome| match outcome {
        Outcome::Moved | Outcome::Edited | Outcome::NoSelection | Outcome::Empty => None,
        Outcome::Added => Some("added".to_owned()),
        Outcome::Deleted(deleted) => {
            let mut text = format!(
                "deleted {} at {}",
                pitch_name(deleted.pitch),
                position(deleted.start_tick, meter, origin)
            );
            if let Some(sharing_parts) = sharing_parts {
                text.push_str("; ");
                text.push_str(&sharing(sharing_parts));
            }
            Some(text)
        }
        Outcome::AlreadyFirst => Some("already the first note".to_owned()),
        Outcome::AlreadyLast => Some("already the last note".to_owned()),
        Outcome::AlreadyHighest => Some("already at the top".to_owned()),
        Outcome::AlreadyLowest => Some("already at the bottom".to_owned()),
        Outcome::AlreadyAtStart => Some("already at the start".to_owned()),
        Outcome::AlreadyAtEnd => Some("already at the end".to_owned()),
        Outcome::AlreadyShortest => Some("already the shortest".to_owned()),
        Outcome::NoRoom => Some("no room".to_owned()),
        Outcome::SelectionLost => Some("selection lost".to_owned()),
    });
    parts.extend(outcome_text);
    let mut body = match cursor {
        Some(cursor) => note(cursor, meter, origin),
        None if note_count == 0 => "empty".to_owned(),
        None => "no note selected".to_owned(),
    };
    if cursor.is_some()
        && matches!(outcome, Some(Outcome::Edited | Outcome::Added))
        && let Some(sharing_parts) = sharing_parts
    {
        body.push_str("; ");
        body.push_str(&sharing(sharing_parts));
    }
    parts.push(body);
    parts.join(", ")
}

/// The roll's polite notice for an outcome the name may not carry. Its words never appear in
/// the name, so a listener can tell which of the two was spoken.
pub fn notice(outcome: Outcome, origin: Origin) -> Option<String> {
    let text = match outcome {
        Outcome::Moved | Outcome::Edited | Outcome::Added => return None,
        Outcome::Deleted(_) => match origin {
            Origin::Pattern { sharing_parts } if sharing_parts > 1 => {
                format!("Removed from {sharing_parts} parts.")
            }
            _ => "Removed.".to_owned(),
        },
        Outcome::AlreadyFirst => "Nothing before this one.".to_owned(),
        Outcome::AlreadyLast => "Nothing after this one.".to_owned(),
        Outcome::AlreadyHighest => "Cannot go higher.".to_owned(),
        Outcome::AlreadyLowest => "Cannot go lower.".to_owned(),
        Outcome::AlreadyAtStart => "Cannot move earlier.".to_owned(),
        Outcome::AlreadyAtEnd => "Cannot move later.".to_owned(),
        Outcome::AlreadyShortest => "Cannot be shorter.".to_owned(),
        Outcome::NoRoom => "Nothing added: the pattern ends here.".to_owned(),
        Outcome::NoSelection => "Select a note first.".to_owned(),
        Outcome::Empty => "Nothing here yet.".to_owned(),
        Outcome::SelectionLost => "The selection was reset.".to_owned(),
    };
    Some(text)
}

/// The roll's accessible description: how to use it, read once when it takes focus.
pub const ROLL_KEYS: &str = "Left and Right move between notes, Up and Down change pitch, \
    Control with Left or Right moves a note, Shift with Left or Right changes its length, \
    Enter adds a note, Delete removes one";

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
        let described = note(&note_at(64, 1_920 + 720, 240), COMMON_TIME, Origin::Song);
        assert_eq!(described, "E 4, bar 2 beat 2.5, 0.5 beats, velocity 100");
    }

    #[test]
    fn beats_follow_the_meter_not_the_quarter_note() {
        let six_eight = Meter {
            ticks_per_quarter: 480,
            beats_per_bar: 6,
            beat_unit: 8,
        };
        assert_eq!(
            position(1_440 + 240, six_eight, Origin::Song),
            "bar 2 beat 2"
        );
        assert_eq!(length(240, six_eight), "1 beat");
    }

    #[test]
    fn a_shared_pattern_counts_from_the_pattern_and_says_how_far_an_edit_reaches() {
        let origin = Origin::Pattern { sharing_parts: 3 };
        let edited = note_at(62, 960, 480);
        assert_eq!(
            roll_name(Some(&edited), 1, Some(Outcome::Edited), COMMON_TIME, origin),
            "Piano roll, D 4, pattern bar 1 beat 3, 1 beat, velocity 100; shared by 3 parts"
        );
        assert_eq!(
            roll_name(None, 0, Some(Outcome::Deleted(edited)), COMMON_TIME, origin),
            "Piano roll, deleted D 4 at pattern bar 1 beat 3; shared by 3 parts, empty"
        );
        assert_eq!(
            notice(Outcome::Deleted(edited), origin).as_deref(),
            Some("Removed from 3 parts.")
        );
    }

    #[test]
    fn an_outcome_that_changes_no_note_is_folded_into_the_name() {
        let last = note_at(60, 0, 480);
        assert_eq!(
            roll_name(
                Some(&last),
                1,
                Some(Outcome::AlreadyLast),
                COMMON_TIME,
                Origin::Song
            ),
            "Piano roll, already the last note, C 4, bar 1 beat 1, 1 beat, velocity 100"
        );
        assert_eq!(
            roll_name(None, 0, None, COMMON_TIME, Origin::Song),
            "Piano roll, empty"
        );
        assert_eq!(
            roll_name(
                None,
                4,
                Some(Outcome::NoSelection),
                COMMON_TIME,
                Origin::Song
            ),
            "Piano roll, no note selected"
        );
    }

    #[test]
    fn no_notice_repeats_words_from_the_name() {
        let note = note_at(60, 0, 480);
        for outcome in [
            Outcome::Deleted(note),
            Outcome::AlreadyFirst,
            Outcome::AlreadyLast,
            Outcome::AlreadyHighest,
            Outcome::AlreadyLowest,
            Outcome::AlreadyAtStart,
            Outcome::AlreadyAtEnd,
            Outcome::AlreadyShortest,
            Outcome::NoRoom,
            Outcome::NoSelection,
            Outcome::Empty,
            Outcome::SelectionLost,
        ] {
            let origin = Origin::Pattern { sharing_parts: 2 };
            let notice = notice(outcome, origin).expect("every outcome that changes no note");
            let name = roll_name(Some(&note), 1, Some(outcome), COMMON_TIME, origin);
            let notice_words = notice.trim_end_matches('.').to_lowercase();
            assert!(
                !name.to_lowercase().contains(&notice_words),
                "{notice:?} is inside {name:?}"
            );
        }
    }
}
