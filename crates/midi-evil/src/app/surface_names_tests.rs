//! Every control on a shown surface — the Song and Notes views, the toolbar and the side panel —
//! has a name, and every disabled button there says why.

use eframe::egui::accesskit;

use super::test_harness::*;
use super::*;

fn focusable_nodes_without_a_name(harness: &Harness) -> Vec<String> {
    harness
        .nodes()
        .into_iter()
        .filter(|node| {
            node.supports_action(accesskit::Action::Focus)
                || node.role() == accesskit::Role::ScrollBar
        })
        // A slider's value box is named by the slider's caption through a relation.
        .filter(|node| node.label().is_none_or(str::is_empty) && node.labelled_by().is_empty())
        .map(|node| format!("{:?} with value {:?}", node.role(), node.value()))
        .collect()
}

/// Disabled buttons with the description each carries.
fn disabled_buttons(harness: &Harness) -> Vec<(String, Option<String>)> {
    harness
        .nodes()
        .into_iter()
        .filter(|node| node.role() == accesskit::Role::Button && node.is_disabled())
        .map(|node| {
            (
                node.label().unwrap_or_default().to_owned(),
                node.description().map(str::to_owned),
            )
        })
        .collect()
}

fn description(harness: &Harness, label: &str) -> Option<String> {
    harness.node(label).1.description().map(str::to_owned)
}

/// A project with a shared pattern part, so the Song view shows its part inspector.
fn project_with_a_part() -> Project {
    let mut project = project_with(&[&[note_at(60, 0, 480)], &[note_at(64, 0, 480)]]);
    let pattern = project
        .add_pattern(evil_project::Pattern {
            name: "Riff".into(),
            length_ticks: 1_920,
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
    project
}

#[test]
fn every_focusable_control_on_the_shown_surfaces_is_named() {
    let mut harness = Harness::new(project_with_a_part());
    harness.app.view = View::Song;
    harness.app.roll.select_part(0, Some(0));
    harness.run(vec![]);
    harness.run(vec![]);
    assert!(
        harness
            .node_of(egui::Id::new("tracks").with("__resize"))
            .is_some()
    );
    assert_eq!(
        focusable_nodes_without_a_name(&harness),
        Vec::<String>::new(),
        "Song"
    );
    harness.app.view = View::Notes;
    harness.app.roll.select_part(0, None);
    harness.run(vec![]);
    harness.run(vec![]);
    assert_eq!(
        focusable_nodes_without_a_name(&harness),
        Vec::<String>::new(),
        "Notes"
    );
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![]);
    assert!(harness.node("Velocity").1.numeric_value().is_some());
    assert_eq!(
        focusable_nodes_without_a_name(&harness),
        Vec::<String>::new(),
        "Notes, with the note inspector"
    );
    assert_eq!(
        harness
            .node_of(egui::accesskit_root_id())
            .and_then(|root| root.label().map(str::to_owned)),
        Some(crate::accessibility::WINDOW_TITLE.to_owned())
    );
}

#[test]
fn every_disabled_button_on_the_shown_surfaces_says_why_and_loses_it_when_enabled() {
    let mut harness = Harness::new(project_with_a_part());
    harness.app.view = View::Song;
    harness.app.roll.select_part(0, Some(0));
    harness.run(vec![]);
    harness.run(vec![]);
    assert_eq!(
        description(&harness, "Undo").as_deref(),
        Some("Nothing to undo.")
    );
    assert_eq!(
        description(&harness, "Redo").as_deref(),
        Some("Nothing to redo.")
    );
    harness.click("Record");
    harness.run(vec![]);
    assert!(harness.app.recorder.is_some());
    let disabled = disabled_buttons(&harness);
    for label in [
        "+ Pattern part",
        "Duplicate loop",
        "Duplicate shared",
        "Make unique",
        "Merge to timeline",
        "Delete part",
        "Add a synth track",
        "Add a Maxwell drum-kit track",
        "Track actions",
        "Previous sound",
        "Next sound",
        "Load files…",
    ] {
        assert!(
            disabled
                .iter()
                .any(|(name, reason)| name == label
                    && reason.as_deref() == Some("Not while recording.")),
            "{label} while recording: {disabled:?}"
        );
    }
    harness.click("Stop take");
    harness.run(vec![]);
    for label in [
        "+ Pattern part",
        "Delete part",
        "Track actions",
        "Next sound",
    ] {
        assert_eq!(
            description(&harness, label),
            None,
            "{label} once the take stops"
        );
    }
    harness.app.view = View::Notes;
    harness.app.roll.select_part(0, None);
    harness.run(vec![]);
    for label in ["Earlier", "Later", "Shorter", "Longer", "Delete"] {
        assert_eq!(
            description(&harness, label).as_deref(),
            Some("Select a note first."),
            "{label} without a cursor"
        );
    }
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![press(egui::Key::ArrowRight, CONTROL)]);
    harness.run(vec![]);
    for label in ["Earlier", "Delete", "Undo"] {
        assert_eq!(
            description(&harness, label),
            None,
            "{label} once it can act"
        );
    }
    for track in (0..harness.app.project.tracks.len()).rev() {
        harness.app.project.remove_track(track).unwrap();
    }
    harness.run(vec![]);
    for label in ["SYNTH", "KEYBOARD"] {
        assert_eq!(
            description(&harness, label).as_deref(),
            Some("Add a track first.")
        );
    }
    assert!(
        disabled_buttons(&harness)
            .iter()
            .all(|(_, reason)| reason.as_deref().is_some_and(|reason| !reason.is_empty())),
        "{:?}",
        disabled_buttons(&harness)
    );
}

#[test]
fn alt_arrows_announce_the_sound_a_track_now_has_and_where_it_is_in_the_list() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    let named = |harness: &Harness| {
        harness
            .nodes()
            .into_iter()
            .filter_map(|node| node.label().map(str::to_owned))
            .find(|label| label.starts_with("Sound: "))
            .unwrap_or_default()
    };
    let before = named(&harness);
    assert!(
        before.contains(" of ") && before.contains("synth sounds"),
        "{before}"
    );
    harness.run(vec![press(egui::Key::ArrowRight, egui::Modifiers::ALT)]);
    let announced = harness.announcements();
    assert_eq!(announced.len(), 1, "{announced:?}");
    assert!(announced[0].contains(" of ") && announced[0].contains("synth sounds"));
    harness.run(vec![]);
    assert!(
        named(&harness).contains(&announced[0]),
        "the bar says what was announced"
    );
    assert_ne!(named(&harness), before);
}

#[test]
fn every_open_menu_and_window_on_the_shown_surfaces_is_named_too() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.view = View::Song;
    harness.run(vec![]);
    harness.click("Project");
    harness.run(vec![]);
    harness.node("Project menu");
    assert_eq!(
        focusable_nodes_without_a_name(&harness),
        Vec::<String>::new(),
        "the Project menu open"
    );
    harness.run(vec![plain(egui::Key::Escape)]);
    harness.app.sounds.open = true;
    harness.app.show_key_help = true;
    harness.app.recovery.open = true;
    harness.app.show_about = true;
    harness.run(vec![]);
    harness.run(vec![]);
    harness.node("Dr. Maxwell's studio");
    for title in ["Sound library", "Keyboard notes", "Recover rhythm from WAV"] {
        harness.node(&format!(
            "{title}, title bar, dragged with a pointer to move it"
        ));
    }
    assert_eq!(
        focusable_nodes_without_a_name(&harness),
        Vec::<String>::new(),
        "the windows open"
    );
}
