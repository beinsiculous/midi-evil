//! The arranger by keyboard, its one command boundary, and every command saying what happened.

use super::test_harness::*;
use super::*;

/// Two tracks; the first has parts at bars 3 and 1 (stored in that order) playing one pattern.
fn arrangement() -> Project {
    let mut project = project_with(&[&[note_at(60, 0, 480)], &[]]);
    let pattern = project
        .add_pattern(evil_project::Pattern {
            name: "Riff".into(),
            length_ticks: 1_920,
            ..evil_project::Pattern::default()
        })
        .unwrap();
    for bar in [2, 0] {
        project
            .add_part(
                0,
                evil_project::Part {
                    pattern,
                    start_tick: bar * 1_920,
                    length_ticks: 1_920,
                },
            )
            .unwrap();
    }
    project
}

pub(super) fn song_harness() -> Harness {
    let mut harness = Harness::new(arrangement());
    harness.app.view = View::Song;
    harness.run(vec![]);
    harness.run(vec![]);
    harness
}

impl Harness {
    fn song_focused(&self) -> bool {
        self.app.song.widget_id.is_some() && self.focused() == self.app.song.widget_id
    }

    /// F6 until the arranger has focus, as a person reaches it.
    fn focus_song(&mut self) {
        for _ in 0..3 {
            if self.song_focused() {
                break;
            }
            self.run(vec![plain(egui::Key::F6)]);
        }
        assert!(self.song_focused(), "F6 reaches the arranger");
    }

    fn song_name(&self) -> String {
        self.app
            .song
            .widget_id
            .and_then(|song| self.node_of(song))
            .and_then(|node| node.label().map(str::to_owned))
            .unwrap_or_default()
    }
}

#[test]
fn the_arranger_is_one_editor_whose_name_is_the_track_and_the_part_under_its_cursor() {
    let mut harness = song_harness();
    harness.focus_song();
    assert!(
        harness.song_name().starts_with(
            "Song, track 1 of 2, Track 1, sound Lead, unique timeline, 1 note, 2 parts"
        ),
        "{}",
        harness.song_name()
    );
    harness.key(plain(egui::Key::ArrowRight));
    assert!(
        harness.song_name().contains(
            "part 1 of 2, Riff, bar 1 beat 1, 1 bar, shared by 2 parts; the unique timeline \
             has 1 note"
        ),
        "{}",
        harness.song_name()
    );
    harness.key(plain(egui::Key::End));
    assert!(
        harness
            .song_name()
            .contains("part 2 of 2, Riff, bar 3 beat 1")
    );
    harness.key(plain(egui::Key::ArrowDown));
    assert!(
        harness
            .song_name()
            .starts_with("Song, track 2 of 2, Track 2, sound Lead, unique timeline, empty"),
        "{}",
        harness.song_name()
    );
    harness.run(vec![plain(egui::Key::ArrowDown)]);
    assert_eq!(harness.announcements(), ["No track below."]);
    harness.key(plain(egui::Key::Home));
    harness.run(vec![plain(egui::Key::ArrowLeft)]);
    assert_eq!(harness.announcements(), ["Nothing before this one."]);
    assert!(harness.song_focused(), "the arrows never move focus off it");
    let focusable_parts = harness
        .nodes()
        .into_iter()
        .filter(|node| {
            node.label()
                .is_some_and(|label| label.starts_with("Part, "))
        })
        .count();
    assert_eq!(focusable_parts, 0, "one way to reach a part: the cursor");
}

#[test]
fn control_arrows_move_the_part_a_bar_and_a_held_nudge_is_one_undo_entry() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.key(plain(egui::Key::ArrowRight));
    harness.run_holding(vec![press(egui::Key::ArrowRight, CONTROL)]);
    for _ in 0..2 {
        harness.run_holding(vec![egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: CONTROL,
        }]);
    }
    harness.key(release(egui::Key::ArrowRight));
    assert_eq!(harness.app.project.tracks[0].parts[1].start_tick, 3 * 1_920);
    assert!(harness.song_name().contains("bar 4 beat 1"));
    harness.run(vec![press(egui::Key::Z, CONTROL)]);
    assert_eq!(
        harness.app.project.tracks[0].parts[1].start_tick, 0,
        "the whole hold undone at once"
    );
    assert_eq!(
        harness.app.roll.selected_part,
        Some(1),
        "undo keeps the cursor's part"
    );
}

#[test]
fn enter_opens_the_cursors_part_in_the_roll_and_says_what_is_open_once_focus_is_there() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.key(plain(egui::Key::ArrowRight));
    harness.run(vec![plain(egui::Key::Enter)]);
    assert!(harness.app.view == View::Notes);
    assert!(harness.roll_focused(), "focus follows into the roll");
    assert!(harness.roll_name().starts_with("Piano roll, "));
    assert!(harness.announcements().is_empty(), "the roll's name first");
    harness.run(vec![]);
    assert_eq!(
        harness.announcements(),
        ["Riff of Track 1, in the piano roll."]
    );
    assert_eq!(harness.app.roll.selected_part, Some(1));
}

#[test]
fn delete_removes_the_part_and_says_so_and_the_name_follows_to_the_next_part() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.key(plain(egui::Key::ArrowRight));
    harness.run(vec![plain(egui::Key::Delete)]);
    assert_eq!(harness.announcements(), ["Removed."]);
    harness.run(vec![]);
    assert_eq!(harness.app.project.tracks[0].parts.len(), 1);
    assert!(
        harness
            .song_name()
            .contains("part 1 of 1, Riff, bar 3 beat 1"),
        "{}",
        harness.song_name()
    );
    assert!(!harness.song_name().to_lowercase().contains("removed"));
}

#[test]
fn during_a_take_delete_on_a_part_removes_nothing_says_why_and_leaves_the_takes_target() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.key(plain(egui::Key::ArrowRight));
    harness.click("Record");
    assert!(harness.app.recorder.is_some());
    let target = harness
        .app
        .recorder
        .as_ref()
        .map(crate::recording::Recorder::target_track);
    harness.focus_song();
    harness.run(vec![plain(egui::Key::Delete)]);
    assert_eq!(harness.announcements(), ["Not while recording."]);
    assert_eq!(
        harness.app.project.tracks[0].parts.len(),
        2,
        "nothing removed"
    );
    assert_eq!(
        harness
            .app
            .recorder
            .as_ref()
            .map(crate::recording::Recorder::target_track),
        target
    );
    harness.run(vec![press(egui::Key::ArrowRight, CONTROL)]);
    assert_eq!(harness.announcements(), ["Not while recording."]);
    assert_eq!(harness.app.project.tracks[0].parts[1].start_tick, 0);
    harness.app.run_song_command(SongCommand::DuplicateShared);
    assert_eq!(
        harness.app.project.tracks[0].parts.len(),
        2,
        "a button's command passes the same boundary"
    );
    harness.app.stop_preview();
}

use crate::arranger::edits::SongCommand;

#[test]
fn every_arranger_button_says_what_it_did() {
    let mut harness = song_harness();
    harness.app.roll.select_part(0, Some(1));
    harness.run(vec![]);
    for (button, heard) in [
        ("Loop part", "Looping from bar 1 beat 1 to bar 2 beat 1."),
        (
            "Duplicate shared",
            "Duplicated the part; 3 parts now play Riff.",
        ),
        (
            "Make unique",
            "This part now plays its own pattern, Riff Unique.",
        ),
        ("Merge to timeline", "Merged into the unique timeline."),
        ("+ Pattern part", "Added a part playing the new Pattern 3."),
        ("Delete part", "Removed."),
        ("Duplicate loop", "Duplicated "),
    ] {
        harness.click(button);
        let announced = harness.announcements();
        assert!(
            announced.iter().any(|text| text.starts_with(heard)),
            "{button}: {announced:?}"
        );
        assert!(
            harness.app.status.starts_with(heard),
            "{button}: the status line too"
        );
        harness.run(vec![]);
    }
    harness.click("Unique timeline");
    let mut heard = Vec::new();
    for _ in 0..4 {
        harness.run(vec![]);
        heard.extend(harness.announcements());
    }
    assert_eq!(
        heard,
        ["The unique timeline of Track 1, in the piano roll."]
    );
}

#[test]
fn the_rulers_bar_has_a_key_and_reading_the_transport_works_from_the_arranger() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.key(plain(egui::Key::End));
    harness.run(vec![plain(egui::Key::B)]);
    assert_eq!(harness.announcements(), ["Playback starts at bar 3."]);
    assert_eq!(harness.app.transport.start_tick, 2 * 1_920);
    harness.run(vec![plain(egui::Key::I)]);
    assert!(harness.announcements()[0].starts_with("Transport, stopped, "));
}

#[test]
fn the_arranger_describes_every_chord_of_its_table_and_the_help_window_lists_each_surface() {
    let mut harness = song_harness();
    harness.focus_song();
    let description = harness
        .app
        .song
        .widget_id
        .and_then(|song| harness.node_of(song))
        .and_then(|node| node.description().map(str::to_owned))
        .unwrap_or_default();
    for chord in [
        "Left ",
        "Up ",
        "Home ",
        "End ",
        "Control+Right ",
        "Enter ",
        "Delete ",
        "B ",
        "I ",
    ] {
        assert!(description.contains(chord), "{chord:?} in {description:?}");
    }
    harness.app.show_key_help = true;
    harness.run(vec![]);
    let (_, notes) = harness.node("Keyboard notes, one chord a line");
    let text = notes.value().unwrap_or_default().to_owned();
    for title in ["Anywhere", "Song (the arranger)", "Piano roll"] {
        assert!(text.contains(title), "{title:?}");
    }
    assert!(text.contains("Control+Shift+Z: redoes it"));
}

/// What the frames say until a background render finishes, or ten seconds pass.
#[test]
fn during_a_take_the_arranger_cannot_change_the_track_and_says_why() {
    let mut harness = song_harness();
    harness.focus_song();
    harness.click("Record");
    assert!(harness.app.recorder.is_some());
    harness.focus_song();
    harness.run(vec![plain(egui::Key::ArrowDown)]);
    assert_eq!(harness.announcements(), ["Not while recording."]);
    assert_eq!(harness.app.roll.selected_track, 0, "the selection stays");
    harness
        .app
        .run_song_command(SongCommand::Select(crate::arranger::cursor::SongCursor {
            track: 1,
            part: None,
        }));
    assert_eq!(harness.app.roll.selected_track, 0, "the pointer's too");
    harness.app.stop_preview();
}

#[test]
fn a_track_change_from_the_arranger_keeps_the_left_tracks_sound_draft_as_the_track_list_does() {
    let mut harness = song_harness();
    assert!(harness.app.open_patch_lab());
    harness.app.patch_lab.as_mut().unwrap().patch.gain = 0.123;
    harness.app.run_song_command(SongCommand::TrackBelow);
    assert_eq!(harness.app.roll.selected_track, 1);
    // A sound editor opened for the new track before the frame ends.
    assert!(harness.app.open_patch_lab());
    harness.app.run_song_command(SongCommand::TrackAbove);
    assert_eq!(harness.app.roll.selected_track, 0);
    assert_eq!(harness.app.patch_lab.as_ref().unwrap().patch.gain, 0.123);
}

#[test]
fn a_part_opened_by_the_pointer_is_named_afresh_in_the_roll_and_its_outcome_follows() {
    let mut project = project_with(&[&[], &[]]);
    for (track, name, pitch) in [(0, "Alpha", 60), (1, "Beta", 64)] {
        let pattern = project
            .add_pattern(evil_project::Pattern {
                name: name.into(),
                length_ticks: 1_920,
                notes: vec![note_at(pitch, 0, 480)],
                ..evil_project::Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                track,
                evil_project::Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 1_920,
                },
            )
            .unwrap();
    }
    let mut harness = Harness::new(project);
    harness.app.roll.select_part(0, Some(0));
    harness.run(vec![]);
    harness.focus_roll();
    harness.key(plain(egui::Key::ArrowRight));
    let roll_name = |harness: &Harness| {
        harness
            .nodes()
            .into_iter()
            .filter(|node| node.role() == egui::accesskit::Role::Canvas)
            .filter_map(|node| node.label().map(str::to_owned))
            .find(|label| label.starts_with("Piano roll, "))
    };
    assert!(
        roll_name(&harness).is_some_and(|name| name.contains("C 4")),
        "{:?} {:?}",
        roll_name(&harness),
        harness.app.roll.selected_part
    );
    harness.app.view = View::Song;
    harness.run(vec![]);
    harness.run(vec![]);
    let board = harness
        .app
        .song
        .widget_id
        .and_then(|song| harness.node_of(song))
        .and_then(|node| node.bounds())
        .expect("the arranger is drawn");
    let on_second_part = egui::pos2(board.x0 as f32 + 170.0, board.y0 as f32 + 94.0 + 40.0);
    harness.run(vec![egui::Event::PointerMoved(on_second_part)]);
    for _ in 0..2 {
        harness.run(vec![
            egui::Event::PointerButton {
                pos: on_second_part,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: on_second_part,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    let mut first_name = None;
    let mut heard_after_the_name = false;
    for _ in 0..5 {
        if first_name.is_some()
            && harness.announcements() == ["Beta of Track 2, in the piano roll."]
        {
            heard_after_the_name = true;
        }
        if first_name.is_none() {
            assert!(
                harness.announcements().is_empty(),
                "the outcome comes after the roll's name"
            );
            first_name = roll_name(&harness).filter(|_| harness.app.view == View::Notes);
        }
        harness.run(vec![]);
    }
    assert_eq!(
        harness.app.roll.selected_track, 1,
        "the double-click opened it"
    );
    assert_eq!(first_name.as_deref(), Some("Piano roll, no note selected"));
    assert!(heard_after_the_name);
}
