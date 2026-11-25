use super::*;

const GRID: EditGrid = EditGrid {
    snap: 120,
    beat: 480,
    pattern_length: None,
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
fn the_cursor_walks_by_start_then_pitch_then_index() {
    let mut notes = vec![
        note_at(64, 480, 120),
        note_at(60, 0, 120),
        note_at(62, 0, 120),
    ];
    assert_eq!(walk_order(&notes), vec![1, 2, 0]);
    let first = apply(&mut notes, None, RollCommand::Next, GRID);
    assert_eq!(first, Edited::new(Some(1), Outcome::Moved));
    assert_eq!(
        apply(&mut notes, first.cursor, RollCommand::Next, GRID).cursor,
        Some(2)
    );
    assert_eq!(
        apply(&mut notes, None, RollCommand::Previous, GRID).cursor,
        Some(0)
    );
    assert_eq!(
        apply(&mut notes, Some(1), RollCommand::Last, GRID).cursor,
        Some(0)
    );
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::First, GRID).cursor,
        Some(1)
    );
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Next, GRID),
        Edited::new(Some(0), Outcome::AlreadyLast)
    );
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Last, GRID),
        Edited::new(Some(0), Outcome::AlreadyLast)
    );
    assert_eq!(
        apply(&mut notes, Some(1), RollCommand::Previous, GRID),
        Edited::new(Some(1), Outcome::AlreadyFirst)
    );
    assert_eq!(
        apply(&mut notes, Some(1), RollCommand::First, GRID),
        Edited::new(Some(1), Outcome::AlreadyFirst)
    );
}

#[test]
fn pitch_is_clamped_at_127_and_at_0() {
    let mut notes = vec![note_at(120, 0, 120)];
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::OctaveUp, GRID).outcome,
        Outcome::Edited
    );
    assert_eq!(notes[0].pitch, MAX_PITCH);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::PitchUp, GRID),
        Edited::new(Some(0), Outcome::AlreadyHighest)
    );
    assert_eq!(notes[0].pitch, MAX_PITCH);
    notes[0].pitch = 5;
    apply(&mut notes, Some(0), RollCommand::OctaveDown, GRID);
    assert_eq!(notes[0].pitch, 0);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::PitchDown, GRID).outcome,
        Outcome::AlreadyLowest
    );
}

#[test]
fn a_move_saturates_at_tick_0_and_at_the_patterns_last_snap() {
    let mut notes = vec![note_at(60, 60, 120)];
    apply(&mut notes, Some(0), RollCommand::Earlier, GRID);
    assert_eq!(notes[0].start_tick, 0);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Earlier, GRID).outcome,
        Outcome::AlreadyAtStart
    );
    let pattern = EditGrid {
        pattern_length: Some(480),
        ..GRID
    };
    notes[0].start_tick = 240;
    apply(&mut notes, Some(0), RollCommand::Later, pattern);
    assert_eq!(notes[0].start_tick, 360);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Later, pattern).outcome,
        Outcome::AlreadyAtEnd
    );
    assert_eq!(notes[0].start_tick, 360);
}

#[test]
fn a_note_is_never_shorter_than_one_snap() {
    let mut notes = vec![note_at(60, 0, 250)];
    apply(&mut notes, Some(0), RollCommand::Shorter, GRID);
    assert_eq!(notes[0].duration_ticks, 130);
    apply(&mut notes, Some(0), RollCommand::Shorter, GRID);
    assert_eq!(notes[0].duration_ticks, 120);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Shorter, GRID).outcome,
        Outcome::AlreadyShortest
    );
    apply(&mut notes, Some(0), RollCommand::Longer, GRID);
    assert_eq!(notes[0].duration_ticks, 240);
}

#[test]
fn enter_adds_a_beat_after_the_cursor_or_middle_c_at_tick_0() {
    let mut notes = Vec::new();
    let added = apply(&mut notes, None, RollCommand::Add, GRID);
    assert_eq!(added, Edited::new(Some(0), Outcome::Added));
    assert_eq!(notes[0], note_at(MIDDLE_C, 0, 480));
    notes[0].pitch = 64;
    let after = apply(&mut notes, Some(0), RollCommand::Add, GRID);
    assert_eq!(after, Edited::new(Some(1), Outcome::Added));
    assert_eq!(notes[1], note_at(64, 480, 480));
}

#[test]
fn enter_with_no_room_in_a_shared_pattern_adds_nothing() {
    let pattern = EditGrid {
        pattern_length: Some(1_920),
        ..GRID
    };
    let mut notes = vec![note_at(60, 1_440, 480)];
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Add, pattern),
        Edited::new(Some(0), Outcome::NoRoom)
    );
    assert_eq!(notes.len(), 1);
}

#[test]
fn delete_moves_to_the_next_note_else_the_previous_else_none() {
    let mut notes = vec![
        note_at(60, 0, 120),
        note_at(62, 480, 120),
        note_at(64, 960, 120),
    ];
    assert_eq!(
        apply(&mut notes, Some(1), RollCommand::Delete, GRID),
        Edited::new(Some(1), Outcome::Deleted)
    );
    assert_eq!(notes[1].pitch, 64);
    assert_eq!(
        apply(&mut notes, Some(1), RollCommand::Delete, GRID).cursor,
        Some(0)
    );
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::Delete, GRID),
        Edited::new(None, Outcome::Deleted)
    );
    assert_eq!(
        apply(&mut notes, None, RollCommand::Delete, GRID),
        Edited::new(None, Outcome::Empty)
    );
}

#[test]
fn an_edit_with_no_cursor_says_so_and_changes_nothing() {
    let mut notes = vec![note_at(60, 0, 120)];
    let before = notes.clone();
    for command in [
        RollCommand::PitchUp,
        RollCommand::Earlier,
        RollCommand::Later,
        RollCommand::Shorter,
        RollCommand::Longer,
        RollCommand::Delete,
        RollCommand::ClearSelection,
    ] {
        assert_eq!(
            apply(&mut notes, None, command, GRID),
            Edited::new(None, Outcome::NoSelection),
            "{command:?}"
        );
    }
    assert_eq!(notes, before);
    assert_eq!(
        apply(&mut notes, Some(0), RollCommand::ClearSelection, GRID),
        Edited::new(None, Outcome::Cleared)
    );
}

#[test]
fn chords_are_told_apart_by_their_modifiers() {
    let command = egui::Modifiers::COMMAND;
    let shift = egui::Modifiers::SHIFT;
    let none = egui::Modifiers::NONE;
    let right = egui::Key::ArrowRight;
    assert_eq!(RollCommand::from_key(right, none), Some(RollCommand::Next));
    assert_eq!(
        RollCommand::from_key(right, command),
        Some(RollCommand::Later)
    );
    assert_eq!(
        RollCommand::from_key(right, shift),
        Some(RollCommand::Longer)
    );
    assert_eq!(RollCommand::from_key(right, egui::Modifiers::ALT), None);
    assert_eq!(RollCommand::from_key(egui::Key::Tab, none), None);
    assert_eq!(RollCommand::from_key(egui::Key::Z, command), None);
    for command in RollCommand::ALL {
        let (modifier, key) = command.chord();
        let modifiers = match modifier {
            ChordModifier::None => none,
            ChordModifier::Shift => shift,
            ChordModifier::Command => egui::Modifiers::COMMAND,
            ChordModifier::CommandShift | ChordModifier::Alt => {
                unreachable!("the roll binds no {modifier:?} chord")
            }
        };
        assert_eq!(RollCommand::from_key(key, modifiers), Some(command));
    }
}

#[test]
fn the_pointer_edits_stay_inside_a_pattern_and_keep_a_snap_of_length() {
    let pattern = EditGrid {
        pattern_length: Some(1_920),
        ..GRID
    };
    let mut notes = Vec::new();
    assert_eq!(
        add_at(&mut notes, 60, 5_000, pattern),
        Edited::new(Some(0), Outcome::Added)
    );
    assert_eq!(notes[0].start_tick, 1_800);
    assert_eq!(
        move_to(&mut notes, 0, 62, 9_999, pattern).outcome,
        Outcome::Edited
    );
    assert_eq!((notes[0].pitch, notes[0].start_tick), (62, 1_800));
    assert_eq!(
        resize_to(&mut notes, 0, 0, pattern).outcome,
        Outcome::Edited
    );
    assert_eq!(notes[0].duration_ticks, 120);
    assert_eq!(select(&notes, Some(3)).cursor, None);
}
