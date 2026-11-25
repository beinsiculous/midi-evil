//! The ordered key dispatch: which keys reach the roll, which reach the application, and which a
//! held key repeats.

use eframe::egui::accesskit;

use super::test_harness::*;
use super::*;

fn alt(key: egui::Key) -> egui::Event {
    press(key, egui::Modifiers::ALT)
}

fn instrument_source(harness: &Harness) -> InstrumentSource {
    harness.app.project.instruments[harness.app.project.tracks[0].instrument]
        .source
        .clone()
}

fn focus_a_text_field(harness: &mut Harness) {
    let field = harness.node_with_role(accesskit::Role::TextInput);
    harness.action(field, accesskit::Action::Focus, None);
    harness.run(vec![]);
    assert!(harness.context.text_edit_focused());
}

#[test]
fn with_the_roll_focused_space_and_the_alt_arrows_still_reach_the_application() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::Space)]);
    assert!(harness.transport_started_then_stopped(), "Space plays");
    let before = instrument_source(&harness);
    harness.run(vec![alt(egui::Key::ArrowRight)]);
    assert_ne!(
        instrument_source(&harness),
        before,
        "Alt+Right cycles the sound"
    );
    assert!(harness.roll_focused());
}

#[test]
fn a_text_field_being_edited_keeps_space_the_alt_arrows_and_its_own_undo() {
    let mut harness = Harness::new(project_with(&[&[]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::Enter)]);
    harness.run(vec![]);
    assert_eq!(harness.notes(0).len(), 1);
    focus_a_text_field(&mut harness);
    harness.run(vec![plain(egui::Key::Space)]);
    assert!(
        !harness.transport_started_then_stopped(),
        "Space is typed, not played"
    );
    let before = instrument_source(&harness);
    harness.run(vec![alt(egui::Key::ArrowRight)]);
    assert_eq!(instrument_source(&harness), before);
    for chord in [
        press(egui::Key::Z, CONTROL),
        press(egui::Key::Y, CONTROL),
        press(egui::Key::Z, CONTROL | egui::Modifiers::SHIFT),
    ] {
        harness.run(vec![chord]);
        assert_eq!(
            harness.notes(0).len(),
            1,
            "the project's history was not touched"
        );
    }
}

#[test]
fn a_held_delete_enter_or_space_acts_once_and_a_held_nudge_is_one_edit_to_undo() {
    let notes: Vec<_> = (0..6).map(|index| note_at(60, index * 480, 480)).collect();
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    for key in [egui::Key::Delete, egui::Key::Enter] {
        let before = harness.notes(0).len();
        harness.run_holding(vec![plain(key)]);
        for _ in 0..5 {
            harness.run(vec![repeat(key)]);
        }
        harness.run(vec![release(key)]);
        assert_eq!(
            harness.notes(0).len().abs_diff(before),
            1,
            "a held {key:?} acts once"
        );
    }
    harness.run_holding(vec![plain(egui::Key::Space)]);
    for _ in 0..3 {
        harness.run(vec![repeat(egui::Key::Space)]);
    }
    assert!(
        harness.transport_started_then_stopped(),
        "repeats did not toggle it back"
    );

    let cursor = harness.cursor().expect("a note under the cursor");
    let pitch = harness.notes(0)[cursor].pitch;
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    for _ in 0..4 {
        harness.run(vec![repeat(egui::Key::ArrowUp)]);
    }
    harness.run(vec![release(egui::Key::ArrowUp)]);
    harness.run(vec![]);
    assert_eq!(
        harness.notes(0)[cursor].pitch,
        pitch + 5,
        "each repeat nudges"
    );
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.notes(0)[cursor].pitch,
        pitch,
        "one undo takes the whole hold"
    );
}

#[test]
fn keys_after_a_tab_in_the_same_frame_never_edit_the_note_being_left() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
    ]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    let before = harness.notes(0).to_vec();
    harness.run(vec![
        plain(egui::Key::Tab),
        plain(egui::Key::ArrowUp),
        plain(egui::Key::Delete),
    ]);
    assert_eq!(harness.notes(0), before.as_slice());
    assert!(!harness.roll_focused(), "the Tab still moved focus on");
}

#[test]
fn behind_an_open_menu_the_roll_claims_nothing_and_escape_closes_the_menu() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
    ]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.click("Project");
    harness.run(vec![]);
    assert!(egui::Popup::is_any_open(&harness.context));
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::F6)]);
    harness.run(vec![plain(egui::Key::Escape)]);
    harness.run(vec![]);
    assert!(
        !egui::Popup::is_any_open(&harness.context),
        "Escape closed the menu"
    );
    assert_eq!(harness.cursor(), Some(0), "the cursor is untouched");
}

#[test]
fn an_edit_then_a_save_then_an_undo_in_one_frame_undoes_the_edit_only() {
    let directory = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.project_path = Some(directory.path().join("song.evil"));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![
        plain(egui::Key::ArrowUp),
        press(egui::Key::S, CONTROL),
        press(egui::Key::Z, CONTROL),
    ]);
    assert_eq!(harness.notes(0), &[note_at(60, 0, 480)]);
    assert!(
        harness.app.dirty,
        "the saved file holds the pitch the undo took back"
    );
}

#[test]
fn quantize_that_drops_the_cursors_note_clears_the_cursor_instead_of_moving_it() {
    let mut project = project_with(&[&[]]);
    let pattern = project
        .add_pattern(evil_project::Pattern {
            notes: vec![note_at(64, 1_900, 10), note_at(60, 0, 480)],
            ..evil_project::Pattern::default()
        })
        .unwrap();
    project
        .add_part(
            0,
            evil_project::Part {
                pattern,
                start_tick: 0,
                length_ticks: 1_920,
            },
        )
        .unwrap();
    let mut harness = Harness::new(project);
    harness.app.roll.select_part(0, Some(0));
    harness.run(vec![]);
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(
        harness.cursor(),
        Some(0),
        "on the note at the pattern's end"
    );
    harness.click("Quantize");
    assert_eq!(
        harness.app.project.patterns[pattern].notes,
        vec![note_at(60, 0, 480)]
    );
    assert_eq!(harness.cursor(), None, "never moved onto the other note");
    assert!(
        harness
            .announcements()
            .contains(&"The selection was reset.".to_owned())
    );
}

#[test]
fn an_edit_held_on_a_slider_or_in_the_roll_is_unsaved_before_the_key_comes_up() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.dirty = false;
    let gain = harness.node("Gain").0;
    harness.action(gain, accesskit::Action::Focus, None);
    let before = harness.app.project.instruments[0].gain;
    harness.run_holding(vec![plain(egui::Key::ArrowLeft)]);
    assert_ne!(
        harness.app.project.instruments[0].gain, before,
        "the slider moved"
    );
    assert!(harness.app.dirty, "dirty while the key is still down");
    assert!(harness.app.unsaved_changes(), "New and Open would ask");

    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.app.dirty = false;
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    assert!(harness.app.dirty);
    assert!(harness.app.unsaved_changes());
}

#[test]
fn two_keys_pressed_back_to_back_are_two_undo_entries() {
    let notes = [note_at(60, 0, 480), note_at(62, 480, 480)];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![
        release(egui::Key::ArrowUp),
        plain(egui::Key::ArrowRight),
    ]);
    harness.run_holding(vec![
        release(egui::Key::ArrowRight),
        plain(egui::Key::ArrowUp),
    ]);
    harness.run(vec![release(egui::Key::ArrowUp)]);
    assert_eq!(
        harness.notes(0),
        &[note_at(61, 0, 480), note_at(63, 480, 480)]
    );
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.notes(0),
        &[note_at(61, 0, 480), note_at(62, 480, 480)],
        "one undo takes back only the second note's raise"
    );
}

#[test]
fn redo_after_undoing_a_take_puts_the_cursor_back_on_its_note() {
    use crate::recording::{InputEvent, Recorder, RecordingTarget};

    let notes = [note_at(60, 480, 480), note_at(62, 960, 480)];
    let mut harness = Harness::new(project_with(&[&notes]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(harness.cursor(), Some(1), "on D");
    harness.app.recording_snapshot = Some(harness.app.editor_snapshot());
    let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 1_920, None);
    let project = harness.app.project.clone();
    recorder.input(
        InputEvent::NoteOn {
            pitch: 64,
            velocity: 100,
        },
        0,
        &project,
    );
    recorder.input(InputEvent::NoteOff { pitch: 64 }, 240, &project);
    harness.app.recorder = Some(recorder);
    harness.app.recording_started_at = Some(std::time::Instant::now());
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0), &notes, "the take was undone");
    harness.run(vec![press(egui::Key::Y, CONTROL)]);
    assert_eq!(harness.notes(0).len(), 3, "the take is back");
    let cursor = harness.cursor().expect("a cursor after redo");
    assert_eq!(
        harness.notes(0)[cursor].pitch,
        62,
        "the cursor is on D, not on C"
    );
}

#[test]
fn a_focused_automation_lane_is_named_and_leaves_space_to_the_transport() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.show_automation = true;
    harness.run(vec![]);
    harness.run(vec![]);
    let lane = harness.node("Automation lane, Pitch bend, 0 points").0;
    harness.action(lane, accesskit::Action::Focus, None);
    harness.run(vec![]);
    assert_eq!(
        harness.focused().map(|widget| widget.accesskit_id()),
        Some(lane)
    );
    harness.run(vec![plain(egui::Key::Space)]);
    assert!(
        harness.transport_started_then_stopped(),
        "Space plays from a lane"
    );
}

#[test]
fn a_slider_moved_while_a_nudge_is_held_is_an_undo_entry_of_its_own() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    let gain = harness.app.project.instruments[0].gain;
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    let slider = harness.node("Gain").0;
    harness.app.dirty = false;
    harness.action(
        slider,
        accesskit::Action::SetValue,
        Some(accesskit::ActionData::NumericValue(0.25)),
    );
    assert!(
        harness.app.dirty,
        "the slider's edit marks the project that frame"
    );
    harness.run(vec![release(egui::Key::ArrowUp)]);
    assert_eq!(harness.notes(0)[0].pitch, 62);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.app.project.instruments[0].gain, gain,
        "the slider undone alone"
    );
    assert_eq!(harness.notes(0)[0].pitch, 62);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.notes(0)[0].pitch,
        60,
        "then the held nudge, as one entry"
    );
}

#[test]
fn clean_overlaps_that_changes_nothing_keeps_the_cursor_on_its_twin() {
    let twin = note_at(60, 0, 480);
    let mut harness = Harness::new(project_with(&[&[twin, twin]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    assert_eq!(harness.cursor(), Some(1));
    harness.click("Clean overlaps");
    assert_eq!(harness.cursor(), Some(1));
    assert!(
        !harness
            .announcements()
            .contains(&"The selection was reset.".to_owned())
    );
}

#[test]
fn undo_leaves_the_view_alone_unless_the_restored_tracks_kind_rules_it_out() {
    let mut project = project_with(&[&[note_at(60, 0, 480)]]);
    let instrument = project
        .add_instrument(Instrument::drum_kit("Drums", "maxwell"))
        .unwrap();
    project
        .add_track(Track {
            name: "Drums".into(),
            instrument,
            ..Track::default()
        })
        .unwrap();
    let mut harness = Harness::new(project);
    harness.app.roll.select_track(1);
    let selection = harness.app.editor_selection();
    harness
        .app
        .history
        .record(harness.app.project.clone(), selection, None);
    harness.app.roll.select_track(0);
    harness.app.view = View::Synth;
    harness.app.project.name = "Edited".into();
    harness.app.undo();
    assert_eq!(harness.app.roll.selected_track, 1);
    assert!(harness.app.view == View::Pads, "a kit cannot stay in Synth");

    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![plain(egui::Key::ArrowUp)]);
    harness.app.view = View::Song;
    harness.run(vec![]);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0)[0].pitch, 60, "undone");
    assert!(
        harness.app.view == View::Song,
        "from Song, undo stays in Song"
    );
}

#[test]
fn the_toolbars_undo_during_a_held_nudge_takes_the_hold_back_as_one_keypress() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    assert_eq!(harness.notes(0)[0].pitch, 63);
    harness.click("Undo");
    assert_eq!(harness.notes(0)[0].pitch, 60, "the hold undone");
    harness.click("Redo");
    assert_eq!(harness.notes(0)[0].pitch, 63, "and reapplied");
    harness.click("Undo");
    assert_eq!(harness.notes(0)[0].pitch, 60, "and undone again");
    harness.run(vec![release(egui::Key::ArrowUp)]);
    assert!(
        !harness.app.history.can_undo(),
        "one entry, as one keypress"
    );
}

#[test]
fn save_as_during_a_held_nudge_keeps_the_undone_instrument_reference_resolving() {
    let directory = tempfile::tempdir().unwrap();
    let old_base = directory.path().join("old");
    let new_base = directory.path().join("new");
    std::fs::create_dir(&old_base).unwrap();
    std::fs::create_dir(&new_base).unwrap();
    std::fs::write(
        old_base.join("lead.json"),
        evil_synth::Patch::default().to_json_pretty().unwrap(),
    )
    .unwrap();
    let mut project = project_with(&[&[note_at(60, 0, 480)]]);
    project.instruments[0].source = InstrumentSource::Synth {
        patch: "lead.json".into(),
    };
    let mut harness = Harness::new(project);
    harness.app.project_path = Some(old_base.join("song.evil"));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    // The frame in which Save As lands, as `ui` runs it.
    harness.app.begin_project_frame();
    harness.app.save_to(new_base.join("song.evil"));
    if let Some(start) = harness.app.frame_start.take() {
        harness.app.finish_project_frame(start, &old_base);
    }
    harness.run(vec![release(egui::Key::ArrowUp)]);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0)[0].pitch, 60, "the hold undone");
    let patch = harness.app.project.instruments[0]
        .patch()
        .unwrap()
        .to_owned();
    assert!(
        evil_render::load_patch(&patch, &new_base).is_ok(),
        "{patch} resolves from the saved project's directory"
    );
}

#[test]
fn a_repeat_and_a_slider_in_one_frame_are_two_entries() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    let gain = harness.app.project.instruments[0].gain;
    let slider = harness.node("Gain").0;
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![
        repeat(egui::Key::ArrowUp),
        egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
            action: accesskit::Action::SetValue,
            target_tree: accesskit::TreeId::ROOT,
            target_node: slider,
            data: Some(accesskit::ActionData::NumericValue(0.25)),
        }),
    ]);
    harness.run(vec![release(egui::Key::ArrowUp)]);
    assert_eq!(harness.notes(0)[0].pitch, 62);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.app.project.instruments[0].gain, gain,
        "the slider undone alone"
    );
    assert_eq!(harness.notes(0)[0].pitch, 62);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0)[0].pitch, 60);
}

#[test]
fn a_final_repeat_arriving_with_its_release_stays_in_the_holds_one_entry() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run_holding(vec![plain(egui::Key::ArrowUp)]);
    harness.run_holding(vec![repeat(egui::Key::ArrowUp)]);
    harness.run(vec![
        repeat(egui::Key::ArrowUp),
        release(egui::Key::ArrowUp),
    ]);
    assert_eq!(harness.notes(0)[0].pitch, 63);
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(harness.notes(0)[0].pitch, 60);
    assert!(
        !harness.app.history.can_undo(),
        "one entry for the whole hold"
    );
}

#[test]
fn an_undo_that_takes_away_the_rolls_track_leaves_the_next_key_nothing_to_edit() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.click("Track actions");
    harness.click("Duplicate track");
    harness.focus_roll();
    assert_eq!(harness.app.roll.selected_track, 1);
    harness.run(vec![press(egui::Key::Z, CONTROL), plain(egui::Key::Enter)]);
    assert_eq!(
        harness.app.project.tracks.len(),
        1,
        "the duplicate is undone"
    );
    assert_eq!(
        harness.notes(0),
        &[note_at(60, 0, 480)],
        "Enter added nothing"
    );
}
