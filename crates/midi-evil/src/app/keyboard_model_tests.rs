//! The keyboard model through whole frames of the application: the key dispatch, the roll, the
//! announcer, the regions and undo together, read back from the accessibility tree egui builds.

use eframe::egui::accesskit;

use super::test_harness::*;
use super::*;
use crate::recording::{Recorder, RecordingTarget};

#[test]
fn an_inspector_edit_over_several_frames_keeps_the_cursor_off_the_twin() {
    let twin = note_at(60, 0, 480);
    let mut harness = Harness::new(project_with(&[&[twin, twin]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(harness.cursor(), Some(0));
    for velocity in [99, 98, 97] {
        let velocity_box = harness.node_with_value(f64::from(velocity + 1));
        harness.action(
            velocity_box,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(f64::from(velocity))),
        );
        harness.run(vec![]);
        assert_eq!(harness.cursor(), Some(0), "the drag went on editing note 0");
        assert_eq!(harness.notes(0)[0].velocity, velocity);
        assert_eq!(
            harness.notes(0)[1].velocity,
            100,
            "the twin was never touched"
        );
    }
}

#[test]
fn an_arrow_in_the_frame_after_f6_moves_the_cursor_and_focus_stays() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
    ]]));
    harness.run(vec![plain(egui::Key::F6)]);
    harness.run(vec![plain(egui::Key::F6)]);
    harness.run(vec![plain(egui::Key::F6), plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![]);
    assert!(
        harness.roll_focused(),
        "the arrow must not carry focus off the roll"
    );
    assert_eq!(harness.cursor(), Some(1));
}

#[test]
fn f6_from_a_bass_part_reaches_the_toolbar_and_shift_f6_returns() {
    let bass = [note_at(36, 0, 480), note_at(31, 480, 480)];
    let mut harness = Harness::new(project_with(&[&bass]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::F6)]);
    let focused = harness.focused();
    assert!(focused.is_some() && !harness.roll_focused());
    assert_eq!(
        focused.and_then(|widget| crate::regions::region_of(&harness.context, widget)),
        Some(crate::regions::Region::Toolbar)
    );
    harness.run(vec![press(egui::Key::F6, egui::Modifiers::SHIFT)]);
    assert!(harness.roll_focused());
}

#[test]
fn delete_then_tab_away_and_back_leaves_no_outcome_in_the_name() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
    ]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::Delete)]);
    assert!(harness.announcements().contains(&"Removed.".to_owned()));
    harness.run(vec![]);
    assert_eq!(
        harness.roll_name(),
        "Piano roll, D 4, bar 1 beat 2, 1 beat, velocity 100"
    );
    harness.run(vec![plain(egui::Key::Tab)]);
    harness.run(vec![press(egui::Key::Tab, egui::Modifiers::SHIFT)]);
    let name = harness.roll_name().to_lowercase();
    assert!(
        !name.contains("deleted") && !name.contains("removed"),
        "{name}"
    );
}

#[test]
fn undoing_the_add_that_made_the_cursors_note_says_the_selection_was_reset() {
    let mut harness = Harness::new(project_with(&[&[]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::Enter)]);
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert!(harness.notes(0).is_empty());
    assert_eq!(harness.cursor(), None);
    assert!(
        harness
            .announcements()
            .contains(&"The selection was reset.".to_owned())
    );
}

#[test]
fn a_held_arrow_publishes_one_name_at_rest_and_a_held_outcome_once_per_hold() {
    let notes: Vec<_> = (0..25).map(|index| note_at(60, index * 120, 120)).collect();
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    let right = egui::Key::ArrowRight;
    harness.run_holding(vec![plain(right)]);
    let first = harness.roll_name();
    assert!(first.contains("bar 1 beat 1,"), "{first}");
    for _ in 0..19 {
        harness.run(vec![repeat(right)]);
        assert_eq!(
            harness.roll_name(),
            first,
            "a held key must not republish the name"
        );
    }
    assert_eq!(harness.cursor(), Some(19));
    harness.run(vec![release(right)]);
    assert!(
        harness.roll_name().contains("bar 2 beat 1.75,"),
        "{}",
        harness.roll_name()
    );

    for _ in 0..6 {
        harness.run(vec![press(egui::Key::ArrowUp, egui::Modifiers::SHIFT)]);
    }
    assert_eq!(harness.notes(0)[19].pitch, 127);
    let up = egui::Key::ArrowUp;
    let mut heard = Vec::new();
    for event in [
        plain(up),
        repeat(up),
        repeat(up),
        repeat(up),
        release(up),
        plain(up),
    ] {
        harness.run_holding(vec![event]);
        heard.extend(harness.announcements());
    }
    let refusals = heard
        .iter()
        .filter(|text| *text == "Cannot go higher.")
        .count();
    assert_eq!(refusals, 2, "once in the hold and once after it: {heard:?}");
}

#[test]
fn with_the_last_track_deleted_f6s_editor_stop_is_the_add_a_track_button() {
    let mut harness = Harness::new(project_with(&[
        &[note_at(60, 0, 480)],
        &[note_at(62, 0, 480)],
    ]));
    harness.focus_roll();
    harness.app.remove_selected_track();
    harness.run(vec![]);
    harness.app.remove_selected_track();
    harness.run(vec![]);
    harness.run(vec![]);
    harness.run(vec![press(egui::Key::F6, egui::Modifiers::SHIFT)]);
    let add = harness.node("+ Add a track to begin composing").0;
    assert_eq!(
        harness.focused().map(|widget| widget.accesskit_id()),
        Some(add)
    );
}

#[test]
fn undoing_a_twins_addition_puts_the_cursor_back_on_the_first_twin() {
    let twin = note_at(60, 0, 480);
    let mut harness = Harness::new(project_with(&[&[twin, twin]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![plain(egui::Key::Enter)]);
    assert_eq!(harness.cursor(), Some(2));
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0).len(), 2);
    assert_eq!(
        harness.cursor(),
        Some(0),
        "back on the first twin, not on either by value"
    );
}

#[test]
fn the_one_note_buttons_edit_the_cursors_note_and_are_disabled_without_one() {
    let notes = [
        note_at(60, 0, 120),
        note_at(62, 120, 120),
        note_at(64, 960, 120),
    ];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.click("Later");
    harness.click("Later");
    assert_eq!(harness.cursor(), Some(0), "the same note moves twice");
    assert_eq!(
        harness.notes(0)[0].start_tick,
        240,
        "past its neighbour at 120"
    );
    harness.click("Longer");
    assert_eq!(harness.notes(0)[0].duration_ticks, 240);
    harness.click("Shorter");
    harness.click("Earlier");
    assert_eq!(harness.notes(0)[0], note_at(60, 120, 120));
    harness.click("Delete");
    assert_eq!(harness.notes(0).len(), 2);
    assert!(harness.announcements().contains(&"Removed.".to_owned()));
    harness.run(vec![plain(egui::Key::Escape)]);
    harness.run(vec![]);
    for label in ["Earlier", "Later", "Shorter", "Longer", "Delete"] {
        assert!(!harness.enabled(label), "{label} acts on no note");
    }
    assert!(harness.enabled("Add"));
}

#[test]
fn undoing_a_duplicated_track_never_leaves_the_cursor_on_another_tracks_note() {
    let mut harness = Harness::new(project_with(&[
        &[note_at(60, 0, 480)],
        &[note_at(64, 0, 480)],
    ]));
    harness.click("Track actions");
    harness.click("Duplicate track");
    assert_eq!(harness.app.roll.selected_track, 1);
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.app.project.tracks.len(), 2);
    assert_eq!(harness.app.roll.selected_track, 0);
    assert_eq!(harness.cursor(), None);
    harness.run(vec![plain(egui::Key::Delete)]);
    assert_eq!(harness.notes(1), &[note_at(64, 0, 480)]);
}

#[test]
fn undoing_a_move_past_a_neighbour_puts_the_cursor_on_the_moved_note() {
    let notes = [
        note_at(60, 0, 240),
        note_at(62, 60, 240),
        note_at(64, 960, 240),
    ];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![press(egui::Key::ArrowRight, CONTROL)]);
    assert_eq!(harness.notes(0)[0].start_tick, 120);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0)[0].start_tick, 0);
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![]);
    assert!(
        harness
            .roll_name()
            .starts_with("Piano roll, C 4, bar 1 beat 1,")
    );
}

#[test]
fn clean_overlaps_carries_the_cursor_with_its_note_through_a_re_sort_and_a_shortening() {
    let notes = [note_at(72, 192, 96), note_at(71, 288, 96)];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![press(egui::Key::ArrowLeft, CONTROL)]);
    harness.run(vec![press(egui::Key::ArrowLeft, CONTROL)]);
    assert_eq!(harness.cursor(), Some(1));
    harness.click("Clean overlaps");
    assert_eq!(harness.notes(0)[0].pitch, 71, "the clean re-sorted the two");
    assert_eq!(harness.cursor(), Some(0), "the cursor followed B 4");

    let overlap = [note_at(60, 0, 960), note_at(60, 480, 480)];
    let mut harness = Harness::new(project_with(&[&overlap]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.click("Clean overlaps");
    assert_eq!(harness.notes(0)[0].duration_ticks, 480);
    assert_eq!(harness.cursor(), Some(0), "followed by start and pitch");
}

#[test]
fn two_keys_in_one_frame_act_in_the_order_they_arrived() {
    let notes = [
        note_at(60, 0, 480),
        note_at(62, 480, 480),
        note_at(64, 960, 480),
    ];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::Delete), press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0).len(), 3, "the delete was undone");
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![plain(egui::Key::ArrowUp)]);
    harness.run(vec![press(egui::Key::Z, CONTROL), plain(egui::Key::Delete)]);
    assert_eq!(
        harness.notes(0),
        &notes[1..],
        "the pitch was undone, then the note deleted"
    );
    harness.run(vec![plain(egui::Key::ArrowRight), plain(egui::Key::Delete)]);
    assert_eq!(
        harness.notes(0),
        &[notes[1]],
        "the note moved to was deleted"
    );
}

#[test]
fn escape_clears_the_cursor_so_quantize_reaches_every_note_and_again_leaves() {
    let notes = [note_at(60, 10, 470), note_at(62, 500, 470)];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::Escape)]);
    assert_eq!(harness.cursor(), None);
    assert!(
        harness.roll_focused(),
        "with a cursor, Escape is the roll's"
    );
    assert!(harness.announcements().contains(
        &"No note selected; Quantize, transpose and Copy apply to all notes.".to_owned()
    ));
    harness.click("Quantize");
    assert_eq!(
        harness.notes(0),
        &[note_at(60, 0, 480), note_at(62, 480, 480)]
    );
    harness.run(vec![plain(egui::Key::Escape)]);
    assert_eq!(
        harness.focused(),
        None,
        "without a cursor, Escape is egui's"
    );
}

#[test]
fn undo_during_a_take_restores_the_cursor_from_when_the_take_began() {
    let notes = [note_at(60, 0, 480), note_at(62, 480, 480)];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.app.recording_snapshot = Some(harness.app.editor_snapshot());
    harness.app.recorder = Some(Recorder::new(RecordingTarget::Track(0), 0, 1_920, None));
    harness.app.recording_started_at = Some(std::time::Instant::now());
    harness.run(vec![plain(egui::Key::ArrowLeft)]);
    assert_eq!(harness.cursor(), Some(0));
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert!(harness.app.recorder.is_none());
    assert_eq!(
        harness.cursor(),
        Some(1),
        "where it was when the take began"
    );
}
