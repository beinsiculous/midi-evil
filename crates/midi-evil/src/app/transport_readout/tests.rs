//! The transport readout: reached by F6, true when read, still between reads.

use eframe::egui::accesskit;

use super::super::test_harness::*;
use super::*;

fn readout(harness: &Harness) -> accesskit::Node {
    harness
        .node_of(readout_id())
        .unwrap_or_else(|| panic!("no transport readout"))
}

fn readout_name(harness: &Harness) -> String {
    readout(harness).label().unwrap_or_default().to_owned()
}

fn focus_readout(harness: &mut Harness) {
    harness.run(vec![plain(egui::Key::F6)]);
    assert_eq!(
        harness.focused(),
        Some(readout_id()),
        "F6 reaches the readout"
    );
}

#[test]
fn the_readout_is_the_toolbars_f6_stop_a_named_status_with_fixed_help() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    focus_readout(&mut harness);
    let node = readout(&harness);
    assert_eq!(node.role(), accesskit::Role::Status);
    assert_eq!(
        node.label(),
        Some(
            "Transport, stopped, tempo 120 beats per minute, 4 4 time, bar 1 beat 1, \
             position 0 minutes, 0 seconds"
        )
    );
    assert_eq!(node.description(), Some(speech::TRANSPORT_READOUT_HELP));
}

#[test]
fn the_readout_is_present_at_the_narrowest_window_and_the_widest() {
    for width in [900.0, 1_920.0] {
        let mut harness = Harness::new(project_with(&[&[]]));
        harness.width = width;
        harness.run(vec![]);
        assert!(
            readout_name(&harness).starts_with("Transport, stopped"),
            "at {width} points"
        );
    }
}

#[test]
fn a_focused_readout_rests_while_the_transport_changes_and_enter_reads_it_again() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    focus_readout(&mut harness);
    let first = readout_name(&harness);
    harness.app.project.tempo_bpm = 90.0;
    for _ in 0..5 {
        harness.run(vec![]);
        assert_eq!(readout_name(&harness), first, "nothing changes by itself");
    }
    assert_eq!(
        readout(&harness).description(),
        Some(speech::TRANSPORT_READOUT_HELP)
    );
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(readout_name(&harness).contains("tempo 90 beats per minute"));
    assert!(
        harness.announcements().is_empty(),
        "the name's change is the reading"
    );
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(
        harness
            .announcements()
            .iter()
            .any(|announcement| announcement.contains("tempo 90 beats per minute")),
        "a reading that changes nothing is announced: {:?}",
        harness.announcements()
    );
    assert_eq!(harness.focused(), Some(readout_id()));
}

#[test]
fn an_unfocused_readout_follows_the_transport_so_focus_arriving_finds_it_true() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.project.tempo_bpm = 75.0;
    harness.run(vec![]);
    assert!(readout_name(&harness).contains("tempo 75"));
    focus_readout(&mut harness);
    harness.run(vec![plain(egui::Key::Tab)]);
    assert_ne!(harness.focused(), Some(readout_id()));
    harness.app.project.tempo_bpm = 76.0;
    harness.run(vec![]);
    assert!(readout_name(&harness).contains("tempo 76"));
}

#[test]
fn with_the_readout_focused_a_new_project_reads_as_the_new_project() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    focus_readout(&mut harness);
    harness.app.project.tempo_bpm = 100.0;
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(readout_name(&harness).contains("tempo 100"));
    harness.app.dirty = false;
    harness.run(vec![press(egui::Key::N, CONTROL)]);
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(readout_name(&harness).contains("tempo 120"));
    assert_eq!(harness.focused(), Some(readout_id()), "focus never moved");
}

#[test]
fn space_on_the_readout_plays_and_stops_while_enter_only_reads() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    focus_readout(&mut harness);
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(
        !harness.transport_started_then_stopped(),
        "Enter is not play"
    );
    harness.run(vec![plain(egui::Key::Space)]);
    assert!(harness.transport_started_then_stopped(), "Space plays");
    assert_eq!(harness.focused(), Some(readout_id()));
}

#[test]
fn the_rolls_transport_chord_and_the_readout_say_the_same_thing() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.run(vec![]);
    let shown = readout_name(&harness);
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::I)]);
    assert_eq!(harness.announcements(), vec![shown]);
}

#[test]
fn during_a_take_with_no_preview_the_bar_and_the_time_come_from_one_clock() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.transport.set_bars(1, 9, 1_920);
    harness.click("Record");
    assert!(harness.app.recorder.is_some());
    harness.app.render_job = None;
    harness.app.pending_render = None;
    assert!(harness.app.preview.is_none(), "no device, so no preview");
    harness.app.recording_started_at =
        Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    harness.run(vec![]);
    let name = readout_name(&harness);
    assert!(
        name.contains("bar 2 beat 3, position 0 minutes, 3 seconds"),
        "three seconds at 120 beats a minute is bar 2 beat 3: {name}"
    );
}
