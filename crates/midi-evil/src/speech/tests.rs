use super::*;
use crate::piano_roll::edits::ChordModifier;

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
        describe_track(1, 3, "Bass", true, "Sub"),
        "track 2 of 3, Bass, muted, sound Sub"
    );
    let part = PartPlace {
        place: 1,
        part_count: 3,
        pattern: "Pattern 1",
        start_tick: 1_920 * 2,
        length_ticks: 1_920,
        sharing: 2,
    };
    assert_eq!(
        describe_part(part, COMMON_TIME),
        "part 2 of 3, Pattern 1, bar 3 beat 1, 1 bar, shared by 2 parts"
    );
    assert_eq!(
        describe_part(
            PartPlace {
                length_ticks: 960,
                sharing: 1,
                ..part
            },
            COMMON_TIME
        ),
        "part 2 of 3, Pattern 1, bar 3 beat 1, 2 beats, the only part playing it"
    );
}

#[test]
fn the_song_name_says_the_track_its_sound_and_the_part_or_the_timeline_under_the_cursor() {
    use evil_project::{Part, Pattern, Track};
    let mut project = Project::new("Song");
    for name in ["Lead", "Bass"] {
        project
            .add_track(Track {
                name: name.into(),
                ..Track::default()
            })
            .unwrap();
    }
    project.tracks[0].notes.push(Note::default());
    let pattern = project
        .add_pattern(Pattern {
            name: "Riff".into(),
            length_ticks: 1_920,
            ..Pattern::default()
        })
        .unwrap();
    for start_tick in [1_920, 0] {
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick,
                    length_ticks: 1_920,
                },
            )
            .unwrap();
    }
    let at = |track, part| Some(SongCursor { track, part });
    assert_eq!(
        song_name(&project, at(0, Some(0)), "Lead"),
        "Song, track 1 of 2, Lead, sound Lead, part 2 of 2, Riff, bar 2 beat 1, 1 bar, \
         shared by 2 parts; the unique timeline has 1 note"
    );
    assert_eq!(
        song_name(&project, at(0, None), "Lead"),
        "Song, track 1 of 2, Lead, sound Lead, unique timeline, 1 note, 2 parts"
    );
    project.tracks[1].muted = true;
    assert_eq!(
        song_name(&project, at(1, None), "Kit"),
        "Song, track 2 of 2, Bass, muted, sound Kit, unique timeline, empty, no parts"
    );
    let name = song_name(&project, at(0, Some(1)), "Lead").to_lowercase();
    for word in ["removed", "added", "cannot", "nothing", "already"] {
        assert!(!name.contains(word), "{word:?} in {name:?}");
    }
}

#[test]
fn every_song_outcome_that_leaves_the_name_unchanged_has_words_of_its_own() {
    for outcome in [
        SongOutcome::AlreadyFirst,
        SongOutcome::AlreadyLast,
        SongOutcome::NoTrackAbove,
        SongOutcome::NoTrackBelow,
        SongOutcome::AlreadyAtStart,
        SongOutcome::NoPart,
        SongOutcome::Removed,
        SongOutcome::SectionDuplicated { copied: 0 },
        SongOutcome::PlaybackStartsAt { bar: 3 },
        SongOutcome::Merged,
    ] {
        assert!(
            describe_song_outcome(&outcome, COMMON_TIME).is_some(),
            "{outcome:?} would be silent"
        );
    }
    assert_eq!(
        describe_song_outcome(&SongOutcome::Moved, COMMON_TIME),
        None
    );
    assert_eq!(
        describe_song_outcome(&SongOutcome::NoPart, COMMON_TIME).as_deref(),
        Some("Select a part first.")
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
                ChordModifier::CommandShift | ChordModifier::Alt => {
                    unreachable!("the roll binds no {modifier:?} chord")
                }
            };
            assert!(help.contains(&chord), "{chord:?} missing from {help:?}");
        }
        for chord in ["Home", "End", "Shift+Up", "Shift+Down", "Escape", "F6"] {
            assert!(help.contains(chord), "{chord:?} missing from {help:?}");
        }
    }
    assert!(!roll_key_help("Command").contains("Control"));
}
