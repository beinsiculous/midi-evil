use super::*;
use evil_project::Track;

const BAR: u64 = 1_920;

/// One track with parts at bars 3 and 1, stored in that order, both playing one pattern, and a
/// second track with nothing on it.
fn arrangement() -> Project {
    let mut project = Project::new("Edits");
    for name in ["Lead", "Bass"] {
        project
            .add_track(Track {
                name: name.into(),
                ..Track::default()
            })
            .unwrap();
    }
    let pattern = project
        .add_pattern(Pattern {
            name: "Riff".into(),
            length_ticks: BAR,
            ..Pattern::default()
        })
        .unwrap();
    for bar in [2, 0] {
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: bar * BAR,
                    length_ticks: BAR,
                },
            )
            .unwrap();
    }
    project
}

fn at(track: usize, part: Option<usize>) -> SongCursor {
    SongCursor { track, part }
}

fn run(project: &mut Project, cursor: SongCursor, command: SongCommand) -> SongEdited {
    apply(project, cursor, command, &mut Transport::new(BAR))
}

#[test]
fn a_chord_means_one_command_and_its_modifiers_must_match_exactly() {
    let right = egui::Key::ArrowRight;
    assert_eq!(
        SongCommand::from_key(right, egui::Modifiers::NONE),
        Some(SongCommand::Next)
    );
    assert_eq!(
        SongCommand::from_key(right, egui::Modifiers::COMMAND),
        Some(SongCommand::Later)
    );
    assert_eq!(SongCommand::from_key(right, egui::Modifiers::SHIFT), None);
    assert_eq!(SongCommand::from_key(right, egui::Modifiers::ALT), None);
    for command in SongCommand::WITH_CHORDS {
        let (modifier, key) = command.chord().unwrap();
        let modifiers = match modifier {
            ChordModifier::None => egui::Modifiers::NONE,
            ChordModifier::Command => egui::Modifiers::COMMAND,
            other => unreachable!("the arranger binds no {other:?} chord"),
        };
        assert_eq!(SongCommand::from_key(key, modifiers), Some(command));
    }
    assert!(SongCommand::Later.repeats_when_held());
    assert!(!SongCommand::Delete.repeats_when_held());
    assert!(!SongCommand::Open.repeats_when_held());
}

#[test]
fn only_the_commands_that_change_parts_are_the_ones_a_take_refuses() {
    for command in [
        SongCommand::Earlier,
        SongCommand::Later,
        SongCommand::Delete,
        SongCommand::AddPatternPart,
        SongCommand::DuplicateLoop,
        SongCommand::DuplicateShared,
        SongCommand::MakeUnique,
        SongCommand::MergeToTimeline,
        SongCommand::MoveTo(0),
        SongCommand::Resize(1),
    ] {
        assert!(command.changes_structure(), "{command:?}");
    }
    for command in [
        SongCommand::Next,
        SongCommand::TrackBelow,
        SongCommand::Open,
        SongCommand::UniqueTimeline,
        SongCommand::LoopPart,
        SongCommand::StartHere,
        SongCommand::StartAtBar(2),
        SongCommand::ReadTransport,
        SongCommand::Select(at(0, None)),
    ] {
        assert!(!command.changes_structure(), "{command:?}");
    }
}

#[test]
fn a_part_moves_a_bar_at_a_time_and_says_when_it_cannot_go_earlier() {
    let mut project = arrangement();
    let later = run(&mut project, at(0, Some(1)), SongCommand::Later);
    assert_eq!(later.outcome, SongOutcome::Edited);
    assert_eq!(
        later.cursor,
        at(0, Some(1)),
        "the cursor stays on the moved part"
    );
    assert_eq!(project.tracks[0].parts[1].start_tick, BAR);
    run(&mut project, at(0, Some(1)), SongCommand::Earlier);
    let refused = run(&mut project, at(0, Some(1)), SongCommand::Earlier);
    assert_eq!(refused.outcome, SongOutcome::AlreadyAtStart);
    assert_eq!(project.tracks[0].parts[1].start_tick, 0);
}

#[test]
fn delete_removes_the_cursors_part_and_hands_the_cursor_to_the_part_that_plays_next() {
    let mut project = arrangement();
    let removed = run(&mut project, at(0, Some(1)), SongCommand::Delete);
    assert_eq!(removed.outcome, SongOutcome::Removed);
    assert_eq!(project.tracks[0].parts.len(), 1);
    assert_eq!(removed.cursor, at(0, Some(0)));
    assert_eq!(
        project.tracks[0].parts[0].start_tick,
        2 * BAR,
        "bar 1's part went"
    );
    let last = run(&mut project, removed.cursor, SongCommand::Delete);
    assert_eq!(last.cursor, at(0, None), "the timeline is left");
    let nothing = run(&mut project, last.cursor, SongCommand::Delete);
    assert_eq!(nothing.outcome, SongOutcome::NoPart);
}

#[test]
fn the_buttons_say_where_the_cursor_goes() {
    let mut project = arrangement();
    let duplicated = run(&mut project, at(0, Some(1)), SongCommand::DuplicateShared);
    assert_eq!(duplicated.cursor, at(0, Some(2)));
    assert_eq!(
        duplicated.outcome,
        SongOutcome::PartDuplicated {
            sharing: 3,
            pattern: "Riff".into()
        }
    );
    let unique = run(&mut project, at(0, Some(2)), SongCommand::MakeUnique);
    assert_eq!(unique.cursor, at(0, Some(2)));
    assert_eq!(
        unique.outcome,
        SongOutcome::MadeUnique {
            pattern: "Riff Unique".into()
        }
    );
    let merged = run(&mut project, at(0, Some(2)), SongCommand::MergeToTimeline);
    assert_eq!(merged.cursor, at(0, None));
    assert_eq!(merged.outcome, SongOutcome::Merged);
    let added = run(&mut project, at(1, None), SongCommand::AddPatternPart);
    assert_eq!(added.cursor, at(1, Some(0)));
    assert!(matches!(added.outcome, SongOutcome::PartAdded { .. }));
}

#[test]
fn the_rulers_bar_and_the_parts_first_bar_start_playback_the_same_way() {
    let mut project = arrangement();
    let mut by_key = Transport::new(BAR);
    let keyed = apply(
        &mut project,
        at(0, Some(0)),
        SongCommand::StartHere,
        &mut by_key,
    );
    let mut by_ruler = Transport::new(BAR);
    let clicked = apply(
        &mut project,
        at(1, None),
        SongCommand::StartAtBar(2),
        &mut by_ruler,
    );
    assert_eq!(by_key, by_ruler);
    assert_eq!(keyed.outcome, SongOutcome::PlaybackStartsAt { bar: 3 });
    assert_eq!(clicked.outcome, SongOutcome::PlaybackStartsAt { bar: 3 });
    assert_eq!(by_key.start_tick, 2 * BAR);
    let on_timeline = apply(
        &mut project,
        at(0, None),
        SongCommand::StartHere,
        &mut by_key,
    );
    assert_eq!(on_timeline.outcome, SongOutcome::NoPart);
}

#[test]
fn opening_and_the_unique_timeline_ask_for_the_roll() {
    let mut project = arrangement();
    let opened = run(&mut project, at(0, Some(0)), SongCommand::Open);
    assert!(opened.open_in_roll);
    assert_eq!(opened.cursor, at(0, Some(0)));
    let timeline = run(&mut project, at(0, Some(0)), SongCommand::UniqueTimeline);
    assert!(timeline.open_in_roll);
    assert_eq!(timeline.cursor, at(0, None));
    let moved = run(&mut project, at(0, None), SongCommand::Next);
    assert!(!moved.open_in_roll);
}
