//! One action is one write in the browser: an outcome is told with the controls as its action
//! left them — the control now unavailable, the focus gone with the window that closed — and the
//! frame after says nothing more of it. The desktop runs each frame once, as it always has.

use super::front_end_tests::*;
use super::test_harness::*;
use super::*;

/// Gives egui's focus to the control named `label`, as Tab would, and lets its name be told.
fn focus(harness: &mut Harness, label: &str) {
    let node = harness.node(label).0;
    harness.action(node, egui::accesskit::Action::Focus, None);
    spoken(harness, vec![]);
}

fn one_note_edited(harness: &mut Harness) {
    focus_roll_in_the_browser(harness);
    spoken(harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(harness, vec![plain(egui::Key::ArrowUp)]);
    spoken(harness, vec![]);
}

#[test]
fn undo_that_leaves_nothing_to_undo_is_told_with_its_new_state_in_one_write() {
    let mut harness = browser(three_notes());
    one_note_edited(&mut harness);
    focus(&mut harness, "Undo");
    let undone = spoken(&mut harness, vec![plain(egui::Key::Enter)]);
    let said = undone.announcement.unwrap_or_default();
    assert!(said.starts_with("Undid the last edit."), "{said:?}");
    assert!(said.ends_with("unavailable"), "{said:?}");
    assert_eq!(spoken(&mut harness, vec![]).announcement, None);
}

#[test]
fn a_button_whose_window_closes_is_told_with_where_focus_now_is_in_one_write() {
    let mut harness = browser(three_notes());
    harness.app.show_about = true;
    spoken(&mut harness, vec![]);
    focus(&mut harness, "Start composing");
    let started = spoken(&mut harness, vec![plain(egui::Key::Enter)]);
    let said = started.announcement.unwrap_or_default();
    assert!(said.starts_with("The piano roll is open."), "{said:?}");
    assert!(said.contains("Nothing in the editor has focus"), "{said:?}");
    assert_eq!(spoken(&mut harness, vec![]).announcement, None);
}

#[test]
fn the_desktop_runs_an_action_in_one_pass_and_the_browser_in_two() {
    for (mut harness, passes) in [
        (Harness::new(three_notes()), 1),
        (browser(three_notes()), 2),
    ] {
        if passes == 2 {
            one_note_edited(&mut harness);
        } else {
            harness.focus_roll();
            harness.key(plain(egui::Key::ArrowRight));
            harness.key(plain(egui::Key::ArrowUp));
        }
        focus(&mut harness, "Undo");
        spoken(&mut harness, vec![plain(egui::Key::Enter)]);
        assert_eq!(harness.output.platform_output.num_completed_passes, passes);
    }
}

/// The region chord's frame, then the next frame holding the chord's last press and an arrow:
/// the arrow moves the roll's cursor once, and the focus stays on the roll.
#[test]
fn in_the_browser_an_arrow_in_the_frame_after_the_region_chord_moves_the_cursor_once() {
    let mut harness = browser(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
        note_at(64, 960, 480),
    ]]));
    let region = press(egui::Key::PageDown, egui::Modifiers::ALT);
    spoken(&mut harness, vec![region.clone()]);
    spoken(&mut harness, vec![region.clone()]);
    spoken(&mut harness, vec![region, plain(egui::Key::ArrowRight)]);
    let moved = spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(&mut harness, vec![]);
    assert!(
        harness.roll_focused(),
        "the arrow carried focus off the roll"
    );
    assert_eq!(
        harness.cursor(),
        Some(1),
        "each arrow moved the cursor once"
    );
    assert!(
        moved.current.starts_with("Piano roll, D 4, bar 1 beat 2"),
        "{}",
        moved.current
    );
}

/// Two keys in one frame, three orders, each acting once and in the order it arrived, through
/// the pass the browser discards and runs again.
#[test]
fn in_the_browser_two_keys_in_one_frame_each_act_once_in_the_order_they_arrived() {
    let notes = [
        note_at(60, 0, 480),
        note_at(62, 480, 480),
        note_at(64, 960, 480),
    ];
    let mut harness = browser(project_with(&[&notes]));
    focus_roll_in_the_browser(&mut harness);
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    let delete_then_undo = spoken(
        &mut harness,
        vec![plain(egui::Key::Delete), press(egui::Key::Z, CONTROL)],
    );
    let passes = |harness: &Harness| harness.output.platform_output.num_completed_passes;
    assert_eq!(
        passes(&harness),
        2,
        "the frame went through the pass run again"
    );
    assert_eq!(harness.notes(0), &notes, "the delete was undone, once");
    assert_eq!(harness.cursor(), Some(0));
    assert!(harness.app.history.can_redo(), "the undo is there to redo");
    spoken(&mut harness, vec![plain(egui::Key::ArrowUp)]);
    let undo_then_delete = spoken(
        &mut harness,
        vec![press(egui::Key::Z, CONTROL), plain(egui::Key::Delete)],
    );
    assert_eq!(
        harness.notes(0),
        &notes[1..],
        "the pitch was undone, then the note deleted, each once"
    );
    assert_eq!(passes(&harness), 2);
    let right_then_delete = spoken(
        &mut harness,
        vec![plain(egui::Key::ArrowRight), plain(egui::Key::Delete)],
    );
    assert_eq!(
        harness.notes(0),
        &[notes[1]],
        "the note moved to was deleted, once"
    );
    assert_eq!(passes(&harness), 2);
    assert_eq!(harness.cursor(), Some(0));
    assert!(harness.roll_focused());
    for frame in [delete_then_undo, undo_then_delete, right_then_delete] {
        let said = frame.announcement.unwrap_or_default();
        assert_eq!(said.matches("Removed.").count(), 1, "{said:?}");
    }
    assert_eq!(spoken(&mut harness, vec![]).announcement, None);
}
