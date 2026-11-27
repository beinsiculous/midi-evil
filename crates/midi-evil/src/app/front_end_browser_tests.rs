//! The browser's build through whole frames of the application, run natively: it binds no chord
//! a browser keeps and names none, its unavailable controls stay reachable and say why, and a
//! field whose edit ended keeps its place without an open edit.

use super::front_end_tests::*;
use super::test_harness::*;
use super::*;

#[test]
fn in_the_browser_a_chord_with_an_extra_modifier_does_nothing() {
    let mut harness = browser(three_notes());
    focus_roll_in_the_browser(&mut harness);
    let control_shift = egui::Modifiers {
        shift: true,
        ..CONTROL
    };
    let control_alt = egui::Modifiers {
        alt: true,
        ..CONTROL
    };
    for (key, modifiers) in [
        (egui::Key::S, control_shift),
        (egui::Key::O, control_alt),
        (egui::Key::N, control_shift),
        (egui::Key::Space, egui::Modifiers::SHIFT),
    ] {
        let frame = spoken(&mut harness, vec![press(key, modifiers)]);
        assert_eq!(frame.happened.outcome, None, "{key:?} with {modifiers:?}");
        assert!(harness.dialogs.take_asked().is_empty());
        assert!(!harness.transport_started_then_stopped());
    }
}

#[test]
fn the_browser_build_shows_and_says_no_chord_a_browser_keeps() {
    const KEPT_BY_A_BROWSER: [&str; 10] = [
        "F6",
        "Alt+Left",
        "Alt+Right",
        "Ctrl+N",
        "Ctrl+O",
        "Control+N",
        "Control+O",
        "Alt and Left",
        "Control and N",
        "Control and O",
    ];
    let mut harness = browser(three_notes());
    focus_roll_in_the_browser(&mut harness);
    let mut said = everything_said(&mut harness);
    harness.click("Project");
    said.push_str(&everything_said(&mut harness));
    harness.app.show_key_help = true;
    harness.app.select_view(View::Song);
    said.push_str(&everything_said(&mut harness));
    said.push_str(&everything_said(&mut harness));
    assert!(said.contains("Keyboard notes"), "the window was open");
    assert!(said.contains("Open…"), "the menu was open");
    for chord in KEPT_BY_A_BROWSER {
        assert!(!said.contains(chord), "{chord:?} is shown or said");
    }
    assert!(said.contains("Alt+Page Down"), "the region chord is named");
}

#[test]
fn the_browser_build_binds_no_chord_a_browser_keeps_and_moves_regions_on_alt_page_down() {
    let mut harness = browser(three_notes());
    focus_roll_in_the_browser(&mut harness);
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    let project = harness.app.project.clone();
    let alt = egui::Modifiers::ALT;
    for (key, modifiers) in [
        (egui::Key::N, CONTROL),
        (egui::Key::O, CONTROL),
        (egui::Key::ArrowLeft, alt),
        (egui::Key::ArrowRight, alt),
        (egui::Key::F6, egui::Modifiers::NONE),
    ] {
        let frame = spoken(&mut harness, vec![press(key, modifiers)]);
        assert_eq!(frame.announcement, None, "{key:?} said something");
        assert!(harness.roll_focused(), "{key:?} moved focus");
        assert_eq!(harness.app.project, project, "{key:?} changed the project");
        assert!(harness.dialogs.take_asked().is_empty());
        assert!(
            !frame.keys.holds(key, modifiers),
            "{key:?} is the browser's"
        );
    }
    spoken(&mut harness, vec![press(egui::Key::PageDown, alt)]);
    assert!(!harness.roll_focused(), "Alt+Page Down leaves the roll");
    spoken(&mut harness, vec![press(egui::Key::PageUp, alt)]);
    assert!(harness.roll_focused(), "Alt+Page Up comes back");
}

#[test]
fn in_the_browser_play_and_undo_are_reachable_say_why_and_do_nothing() {
    let mut harness = browser(three_notes());
    let mut reached = Vec::new();
    for _ in 0..12 {
        let frame = spoken(&mut harness, vec![plain(egui::Key::Tab)]);
        if let Some(control) = frame.focused {
            reached.push((control.name.clone(), frame.current.clone()));
        }
    }
    let play = reached
        .iter()
        .find(|(name, _)| name == "PLAY")
        .expect("Play is a Tab stop");
    assert!(
        play.1
            .contains("unavailable. Playback is not in the browser build yet."),
        "{}",
        play.1
    );
    let pressed = spoken_click(&mut harness, "PLAY");
    assert_eq!(
        pressed.happened.outcome.as_deref(),
        Some("Playback is not in the browser build yet.")
    );
    assert!(!harness.transport_started_then_stopped());
    spoken(&mut harness, vec![plain(egui::Key::Escape)]);
    let space = spoken(&mut harness, vec![plain(egui::Key::Space)]);
    assert_eq!(
        space.happened.outcome.as_deref(),
        Some("Playback is not in the browser build yet.")
    );
    let project = harness.app.project.clone();
    let undo = spoken_click(&mut harness, "Undo");
    assert_eq!(undo.happened.outcome.as_deref(), Some("Nothing to undo."));
    assert_eq!(harness.app.project, project);
}

#[test]
fn in_the_browser_b_says_why_and_moves_nothing() {
    let mut harness = browser(three_notes());
    harness.app.select_view(View::Song);
    harness.run(vec![]);
    for _ in 0..3 {
        if harness.focused() == harness.app.song.widget_id {
            break;
        }
        harness.run(vec![press(egui::Key::PageDown, egui::Modifiers::ALT)]);
    }
    assert_eq!(
        harness.focused(),
        harness.app.song.widget_id,
        "the arranger"
    );
    let transport = harness.app.transport;
    let frame = spoken(&mut harness, vec![plain(egui::Key::B)]);
    assert_eq!(
        frame.happened.outcome.as_deref(),
        Some("Playback is not in the browser build yet.")
    );
    assert_eq!(harness.app.transport, transport);
}

#[test]
fn in_the_browser_save_and_open_choose_nothing_and_say_so() {
    let mut harness = browser(three_notes());
    harness.dialogs.answer(Answer::Nothing);
    let saved = spoken(&mut harness, vec![press(egui::Key::S, CONTROL)]);
    assert_eq!(saved.happened.outcome.as_deref(), Some("Not saved."));
    harness.click("Project");
    spoken(&mut harness, vec![]);
    harness.dialogs.answer(Answer::Nothing);
    let opened = spoken_click(&mut harness, "Open…");
    assert_eq!(opened.happened.outcome.as_deref(), Some("Nothing opened."));
}

#[test]
fn in_the_browser_a_control_refused_while_recording_is_reached_and_says_why() {
    let mut harness = browser(three_notes());
    harness.app.recorder = Some(crate::recording::Recorder::new(
        crate::recording::RecordingTarget::Track(0),
        0,
        1_920,
        None,
    ));
    let mut reached = None;
    for _ in 0..80 {
        let frame = spoken(&mut harness, vec![plain(egui::Key::Tab)]);
        if frame
            .focused
            .as_ref()
            .is_some_and(|control| control.name == "Add a synth track")
        {
            reached = Some(frame.current);
            break;
        }
    }
    let current = reached.expect("Tab reaches the track rack's + while recording");
    assert!(
        current.ends_with("unavailable. Not while recording."),
        "{current}"
    );
    let tracks = harness.app.project.tracks.len();
    let pressed = spoken(&mut harness, vec![plain(egui::Key::Enter)]);
    assert_eq!(
        pressed.happened.outcome.as_deref(),
        Some("Not while recording.")
    );
    assert_eq!(harness.app.project.tracks.len(), tracks);
    harness.app.recorder = None;
}

#[test]
fn in_the_browser_a_field_left_by_enter_or_escape_is_not_being_edited_and_tab_moves_on() {
    for ending in [egui::Key::Enter, egui::Key::Escape] {
        let mut harness = browser(three_notes());
        let tempo = harness.node("Tempo in beats per minute").0;
        harness.action(tempo, eframe::egui::accesskit::Action::Focus, None);
        spoken(&mut harness, vec![]);
        let ended = spoken(&mut harness, vec![plain(ending)]);
        spoken(&mut harness, vec![]);
        let before = harness.app.project.tempo_bpm;
        let typed = spoken(&mut harness, vec![egui::Event::Text("5".to_owned())]);
        spoken(&mut harness, vec![]);
        assert_eq!(
            harness.app.project.tempo_bpm, before,
            "{ending:?}: a digit edited it"
        );
        let kept = typed.focused.expect("the field keeps its place");
        assert_eq!(kept.name, "Tempo in beats per minute");
        assert!(!kept.is_text_entry, "{ending:?}: still an open edit");
        assert!(ended.focused.is_some_and(|control| !control.is_text_entry));
        let moved = spoken(&mut harness, vec![plain(egui::Key::Tab)]);
        let next = spoken(&mut harness, vec![]);
        let name = next.focused.or(moved.focused).map(|control| control.name);
        assert!(
            name.as_deref()
                .is_some_and(|name| name.starts_with("Meter")),
            "{ending:?}: Tab went to {name:?}"
        );
        assert_eq!(harness.app.project.tempo_bpm, before);
    }
}

#[test]
fn in_the_browser_tab_from_a_kept_project_name_reaches_the_tempo_box_and_shift_tab_the_readout() {
    for (shift, wanted) in [(false, "Tempo in beats per minute"), (true, "Transport,")] {
        let mut harness = browser(three_notes());
        let name = harness.node("Project name").0;
        harness.action(name, eframe::egui::accesskit::Action::Focus, None);
        spoken(&mut harness, vec![]);
        spoken(&mut harness, vec![plain(egui::Key::Enter)]);
        spoken(&mut harness, vec![]);
        let modifiers = if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        };
        spoken(&mut harness, vec![press(egui::Key::Tab, modifiers)]);
        let reached = spoken(&mut harness, vec![])
            .focused
            .map(|control| control.name);
        assert!(
            reached
                .as_deref()
                .is_some_and(|name| name.starts_with(wanted)),
            "shift {shift}: reached {reached:?}"
        );
    }
}

#[test]
fn in_the_browser_a_text_that_can_only_be_read_keeps_its_selection_chords() {
    let mut harness = browser(three_notes());
    harness.app.show_about = false;
    harness.app.show_key_help = true;
    spoken(&mut harness, vec![]);
    let mut on_the_notes = None;
    for _ in 0..80 {
        let frame = spoken(&mut harness, vec![plain(egui::Key::Tab)]);
        if frame
            .focused
            .as_ref()
            .is_some_and(|control| control.holds_selectable_text && !control.is_text_entry)
        {
            on_the_notes = Some(frame);
            break;
        }
    }
    let frame = on_the_notes.expect("Tab reaches the keyboard notes' text");
    assert!(
        frame.keys.holds(egui::Key::A, CONTROL),
        "select all is the text's, not the browser's"
    );
    assert!(
        !frame.keys.holds(egui::Key::O, CONTROL),
        "and the browser keeps what it kept"
    );
}
