//! What a front end that writes one string a frame is told, through whole frames of the
//! application: one action is one announcement, the name as it now is in it, and the frame after
//! says nothing; a held key says nothing until it comes up; and the browser's build binds no
//! chord a browser keeps, names none, and keeps its unavailable controls reachable.

use super::test_harness::*;
use super::*;
use crate::platform::{self, Platform};
use crate::spoken::SpokenFrame;

/// One frame, with what a front end is told about it asked inside the frame, as a front end asks.
pub(super) fn spoken(harness: &mut Harness, events: Vec<egui::Event>) -> SpokenFrame {
    let releases: Vec<_> = events
        .iter()
        .filter_map(|event| match *event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } => Some(egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            }),
            _ => None,
        })
        .collect();
    let mut events = events;
    events.extend(releases);
    spoken_holding(harness, events)
}

pub(super) fn spoken_holding(harness: &mut Harness, events: Vec<egui::Event>) -> SpokenFrame {
    harness.time += 0.1;
    let mut spoken = None;
    let Harness {
        context,
        app,
        frame,
        time,
        width,
        ..
    } = harness;
    harness.output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(*width, 900.0),
            )),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ui| {
            eframe::App::ui(app, ui, frame);
            spoken = Some(app.spoken_frame(ui.ctx()));
        },
    );
    spoken.expect("the frame ran")
}

/// Clicks the one node with this name, as a screen reader's click, in a frame that is asked.
pub(super) fn spoken_click(harness: &mut Harness, label: &str) -> SpokenFrame {
    let node = harness.node(label).0;
    spoken(
        harness,
        vec![egui::Event::AccessKitActionRequest(
            eframe::egui::accesskit::ActionRequest {
                action: eframe::egui::accesskit::Action::Click,
                target_tree: eframe::egui::accesskit::TreeId::ROOT,
                target_node: node,
                data: None,
            },
        )],
    )
}

/// The browser's region chord until the roll has focus.
pub(super) fn focus_roll_in_the_browser(harness: &mut Harness) {
    for _ in 0..3 {
        if harness.roll_focused() {
            break;
        }
        harness.run(vec![press(egui::Key::PageDown, egui::Modifiers::ALT)]);
    }
    assert!(harness.roll_focused(), "Alt+Page Down reaches the roll");
}

pub(super) fn three_notes() -> Project {
    project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
        note_at(64, 960, 480),
    ]])
}

pub(super) fn browser(project: Project) -> Harness {
    let mut harness = Harness::new(project);
    platform::install(&harness.context, Platform::Browser);
    harness.run(vec![]);
    harness
}

/// Every string the build shows or says in the next frame: every name, description and value in
/// its tree, and the frame's announcement.
pub(super) fn everything_said(harness: &mut Harness) -> String {
    let frame = spoken(harness, vec![]);
    let mut said: Vec<String> = harness
        .nodes()
        .iter()
        .flat_map(|node| [node.label(), node.description(), node.value()])
        .flatten()
        .map(str::to_owned)
        .collect();
    said.extend(frame.announcement.clone());
    said.push(frame.current.clone());
    said.push(harness.app.status.clone());
    said.join("\n")
}

#[test]
fn a_delete_is_one_announcement_with_the_next_note_and_the_frame_after_says_nothing() {
    let mut harness = Harness::new(three_notes());
    harness.focus_roll();
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(&mut harness, vec![]);
    let deleted = spoken(&mut harness, vec![plain(egui::Key::Delete)]);
    let announcement = deleted.announcement.expect("the delete is announced");
    assert!(
        announcement.starts_with("Removed. Piano roll, D 4"),
        "{announcement}"
    );
    assert_eq!(deleted.happened.outcome.as_deref(), Some("Removed."));
    let after = spoken(&mut harness, vec![]);
    assert_eq!(
        after.announcement, None,
        "the held-back name is not said again"
    );
    assert!(
        after.current.starts_with("Piano roll, D 4"),
        "{}",
        after.current
    );
}

#[test]
fn focus_arriving_says_the_name_role_and_state_once() {
    let mut harness = Harness::new(three_notes());
    let mut frames = Vec::new();
    for _ in 0..3 {
        frames.push(spoken(&mut harness, vec![plain(egui::Key::Tab)]));
    }
    let first = &frames[0];
    assert!(first.happened.focus_moved);
    assert_eq!(
        first.announcement.as_deref(),
        Some("About MIDI Evil, button")
    );
    assert!(first.at_first_stop, "the About button is Tab's first stop");
    assert_eq!(spoken(&mut harness, vec![]).announcement, None);
}

#[test]
fn a_held_key_says_nothing_until_it_comes_up_and_then_says_where_it_rests() {
    let mut harness = Harness::new(three_notes());
    harness.focus_roll();
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(&mut harness, vec![plain(egui::Key::Home)]);
    spoken_holding(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    for _ in 0..3 {
        let held = spoken_holding(&mut harness, vec![repeat(egui::Key::ArrowRight)]);
        assert_eq!(held.announcement, None, "nothing while the key repeats");
        assert!(held.current.starts_with("Piano roll, "), "{}", held.current);
    }
    let released = spoken(&mut harness, vec![release(egui::Key::ArrowRight)]);
    let announcement = released.announcement.expect("one announcement at rest");
    assert!(announcement.contains("E 4"), "{announcement}");
    assert_eq!(spoken(&mut harness, vec![]).announcement, None);
}

#[test]
fn three_identical_notices_are_three_announcements() {
    let mut harness = Harness::new(project_with(&[&[note_at(127, 0, 480)]]));
    harness.focus_roll();
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(&mut harness, vec![]);
    for _ in 0..3 {
        let frame = spoken(&mut harness, vec![plain(egui::Key::ArrowUp)]);
        assert_eq!(frame.announcement.as_deref(), Some("Cannot go higher."));
        spoken(&mut harness, vec![]);
    }
}

#[test]
fn an_undo_is_one_announcement_with_the_selection_it_restored() {
    let mut harness = Harness::new(three_notes());
    harness.focus_roll();
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    spoken(&mut harness, vec![plain(egui::Key::ArrowUp)]);
    spoken(&mut harness, vec![]);
    let undone = spoken(&mut harness, vec![press(egui::Key::Z, CONTROL)]);
    let announcement = undone.announcement.expect("the undo is announced");
    assert!(
        announcement.starts_with("Undid the last edit. Piano roll, C 4"),
        "{announcement}"
    );
    for _ in 0..3 {
        assert_eq!(spoken(&mut harness, vec![]).announcement, None);
    }
}

#[test]
fn a_value_typed_is_not_announced_an_arrow_is_and_enter_commits_it_once() {
    let mut harness = browser(three_notes());
    let tempo = harness.node("Tempo in beats per minute").0;
    harness.action(tempo, eframe::egui::accesskit::Action::Focus, None);
    let arrived = spoken(&mut harness, vec![]);
    let control = arrived.focused.clone().expect("the tempo box has focus");
    assert!(control.is_text_entry);
    assert_eq!(control.name, "Tempo in beats per minute");
    let typed = spoken(&mut harness, vec![egui::Event::Text("9".to_owned())]);
    assert_eq!(
        typed.announcement, None,
        "a screen reader echoes what is typed"
    );
    assert!(typed.current.contains('9'), "{}", typed.current);
    let raised = spoken(&mut harness, vec![plain(egui::Key::ArrowUp)]);
    assert!(raised.announcement.is_some(), "an arrow's change is said");
    let committed = spoken(&mut harness, vec![plain(egui::Key::Enter)]);
    assert!(committed.happened.text_committed);
    let announcement = committed.announcement.expect("the commit is said");
    assert!(
        announcement.starts_with("Tempo in beats per minute, "),
        "{announcement}"
    );
    let after = spoken(&mut harness, vec![]);
    assert_eq!(after.announcement, None, "said once");
    assert_eq!(
        after.focused.map(|control| control.name).as_deref(),
        Some("Tempo in beats per minute"),
        "in the browser the field keeps its place"
    );
}

#[test]
fn the_desktop_leaves_nothing_focused_after_a_field_is_committed() {
    let mut harness = Harness::new(three_notes());
    let tempo = harness.node("Tempo in beats per minute").0;
    harness.action(tempo, eframe::egui::accesskit::Action::Focus, None);
    spoken(&mut harness, vec![]);
    spoken(&mut harness, vec![plain(egui::Key::Enter)]);
    spoken(&mut harness, vec![]);
    assert_eq!(harness.focused(), None, "as egui always has");
}

#[test]
fn the_ends_are_where_egui_wraps_and_the_last_moves_when_a_window_opens() {
    let mut harness = Harness::new(three_notes());
    let closed = spoken(&mut harness, vec![]);
    let (first, last) = (closed.first_stop, closed.last_stop.expect("a last stop"));
    harness
        .context
        .memory_mut(|memory| memory.request_focus(last));
    assert!(spoken(&mut harness, vec![]).at_last_stop);
    spoken(&mut harness, vec![plain(egui::Key::Tab)]);
    let wrapped = spoken(&mut harness, vec![]);
    assert_eq!(
        harness.focused(),
        first,
        "Tab from the last stop wraps to the first"
    );
    assert!(wrapped.at_first_stop);
    harness.app.show_key_help = true;
    spoken(&mut harness, vec![]);
    let opened = spoken(&mut harness, vec![]);
    assert_ne!(
        opened.last_stop,
        Some(last),
        "the window's controls come last"
    );
    harness
        .context
        .memory_mut(|memory| memory.request_focus(last));
    assert!(!spoken(&mut harness, vec![]).at_last_stop);
}

#[test]
fn an_edit_leaves_the_document_unsaved() {
    let mut harness = Harness::new(three_notes());
    harness.focus_roll();
    assert!(!spoken(&mut harness, vec![]).unsaved);
    spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    assert!(spoken(&mut harness, vec![plain(egui::Key::ArrowUp)]).unsaved);
}

#[test]
fn on_the_desktop_a_chord_with_an_extra_modifier_does_what_it_always_did() {
    let mut harness = Harness::new(three_notes());
    harness.focus_roll();
    let control_shift = egui::Modifiers {
        shift: true,
        ..CONTROL
    };
    harness.dialogs.answer(Answer::Nothing);
    let saved = spoken(&mut harness, vec![press(egui::Key::S, control_shift)]);
    assert_eq!(saved.happened.outcome.as_deref(), Some("Not saved."));
    assert_eq!(
        harness.dialogs.take_asked().len(),
        1,
        "Control+Shift+S saves"
    );
    spoken(
        &mut harness,
        vec![press(egui::Key::Space, egui::Modifiers::SHIFT)],
    );
    assert!(
        harness.transport_started_then_stopped(),
        "Shift+Space plays"
    );
}

#[test]
fn on_the_desktop_play_is_a_button_and_undo_with_nothing_to_undo_is_disabled() {
    let harness = Harness::new(three_notes());
    assert!(harness.enabled("PLAY"));
    assert!(!harness.enabled("Undo"));
}

/// Tab from the reported first stop until the reported last, then once more: egui wraps to the
/// first exactly when the reported last is the last it reaches. And Shift+Tab, the other way.
fn walk_to_the_ends(harness: &mut Harness) {
    let frame = spoken(harness, vec![]);
    let (first, last) = (frame.first_stop, frame.last_stop);
    harness
        .context
        .memory_mut(|memory| memory.request_focus(first.expect("a first stop")));
    spoken(harness, vec![]);
    let mut stops = 1;
    while harness.focused() != last && stops < 200 {
        spoken(harness, vec![plain(egui::Key::Tab)]);
        stops += 1;
    }
    assert_eq!(
        harness.focused(),
        last,
        "Tab reached the reported last stop"
    );
    assert!(spoken(harness, vec![]).at_last_stop);
    spoken(harness, vec![plain(egui::Key::Tab)]);
    spoken(harness, vec![]);
    assert_eq!(
        harness.focused(),
        first,
        "Tab from the reported last wraps to the first"
    );
    let mut back = 1;
    spoken(harness, vec![press(egui::Key::Tab, egui::Modifiers::SHIFT)]);
    spoken(harness, vec![]);
    assert_eq!(
        harness.focused(),
        last,
        "Shift+Tab from the first wraps to the last"
    );
    while harness.focused() != first && back < 200 {
        spoken(harness, vec![press(egui::Key::Tab, egui::Modifiers::SHIFT)]);
        back += 1;
    }
    assert_eq!(
        harness.focused(),
        first,
        "Shift+Tab walks back to the first stop"
    );
}

#[test]
fn with_two_windows_open_the_ends_are_where_tab_walks() {
    // The About window, then the Keyboard notes; and the Keyboard notes with the WAV recovery
    // window, a pair whose layer ids sort the other way round from the order they are drawn.
    for (about, recovery) in [(true, false), (false, true)] {
        let mut harness = Harness::new(three_notes());
        harness.app.show_about = about;
        spoken(&mut harness, vec![]);
        harness.app.show_key_help = true;
        harness.app.recovery.open = recovery;
        spoken(&mut harness, vec![]);
        walk_to_the_ends(&mut harness);
    }
}

#[test]
fn the_rolls_zoom_slider_says_its_value_and_its_change() {
    let mut harness = Harness::new(three_notes());
    let mut zoom = None;
    for _ in 0..80 {
        let frame = spoken(&mut harness, vec![plain(egui::Key::Tab)]);
        if frame
            .focused
            .as_ref()
            .is_some_and(|control| control.name == "Zoom")
        {
            zoom = frame.focused;
            break;
        }
    }
    let zoom = zoom.expect("Tab reaches the Zoom slider");
    assert_eq!(zoom.role, "slider");
    assert!(!zoom.value.is_empty(), "the slider's value is read");
    let moved = spoken(&mut harness, vec![plain(egui::Key::ArrowRight)]);
    let value = moved.focused.expect("still on the slider").value;
    assert_ne!(value, zoom.value, "the slider moved");
    assert_eq!(moved.announcement.as_deref(), Some(value.as_str()));
}
