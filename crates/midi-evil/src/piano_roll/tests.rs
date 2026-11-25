use eframe::egui::{Pos2, Rect};
use evil_project::Track;

use super::drawing::{is_black_key, octave_label, snap_tick};
use super::*;

#[test]
fn bar_lengths_follow_the_imported_time_signature() {
    let mut project = Project::new("Meters");
    assert_eq!(bar_ticks(&project), 1_920);
    project.beats_per_bar = 6;
    project.beat_unit = 8;
    assert_eq!(bar_ticks(&project), 1_440);
    project.ticks_per_quarter = 384;
    project.beats_per_bar = 3;
    project.beat_unit = 4;
    assert_eq!(bar_ticks(&project), 1_152);
}

#[test]
fn sticky_keyboard_and_ruler_do_not_create_notes_but_the_grid_does() {
    let context = egui::Context::default();
    theme::install(&context);
    let mut project = Project::new("Hit testing");
    project.add_track(Track::default()).unwrap();
    let mut state = RollState::default();
    let mut announcer = Announcer::default();
    let mut time = 0.0;
    let mut frame = |events| {
        time += 0.1;
        let _ = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                let edited = EditedSequence::new(&project, sequence_context(&project, &state, 0));
                show(
                    ui,
                    &edited,
                    &mut project.tracks[0],
                    &mut state,
                    &mut announcer,
                );
            },
        );
        project.tracks[0].notes.len()
    };
    frame(vec![]);
    frame(vec![]);
    for (position, expected) in [
        (Pos2::new(24.0, 140.0), 0), // fixed keyboard, after initial vertical scroll
        (Pos2::new(180.0, 48.0), 0), // fixed bar ruler
        (Pos2::new(180.0, 140.0), 1),
    ] {
        frame(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let count = frame(vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(count, expected, "unexpected note creation at {position:?}");
    }
    assert_eq!(project.tracks[0].notes[0].start_tick, 840);
}

#[test]
fn snaps_to_the_nearest_grid_point_without_underflow() {
    assert_eq!(snap_tick(0, 32), 0);
    assert_eq!(snap_tick(15, 32), 0);
    assert_eq!(snap_tick(16, 32), 32);
    assert_eq!(snap_tick(47, 32), 32);
    assert_eq!(snap_tick(48, 32), 64);
}

#[test]
fn octave_labels_cover_the_full_midi_range() {
    for pitch in 0..=MAX_PITCH {
        let _ = octave_label(pitch);
    }
    assert_eq!(octave_label(0), "C-1");
    assert_eq!(octave_label(60), "C4");
    assert_eq!(octave_label(120), "C9");
}

#[test]
fn timeline_extends_to_cover_automation() {
    let track = Track {
        pitch_bends: vec![evil_project::PitchBend {
            tick: 28 * 480,
            value: 0,
        }],
        ..Track::default()
    };
    assert_eq!(timeline_ticks(&track, 480), 32 * 480);
}

#[test]
fn black_keys_match_a_piano_keyboard() {
    assert!(!is_black_key(60));
    assert!(is_black_key(61));
    assert!(!is_black_key(62));
    assert!(is_black_key(63));
}

#[test]
fn a_refusal_leaves_the_view_alone_and_a_move_or_an_edit_brings_the_note_into_view() {
    let mut project = Project::new("Scroll");
    project
        .add_track(Track {
            notes: vec![
                Note {
                    pitch: 127,
                    velocity: 100,
                    start_tick: 0,
                    duration_ticks: 120,
                },
                Note {
                    pitch: 60,
                    velocity: 100,
                    start_tick: 480,
                    duration_ticks: 120,
                },
            ],
            ..Track::default()
        })
        .unwrap();
    let context = sequence_context(&project, &RollState::default(), 0);
    let edited = EditedSequence::new(&project, context);
    let mut state = RollState::default();
    let mut announcer = Announcer::default();
    let track = &mut project.tracks[0];
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::Next,
        &mut announcer,
    );
    assert!(
        std::mem::take(&mut state.scroll_to_cursor),
        "moved to a note"
    );
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::PitchUp,
        &mut announcer,
    );
    assert!(!state.scroll_to_cursor, "a refusal does not scroll");
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::Previous,
        &mut announcer,
    );
    assert!(!state.scroll_to_cursor, "already the first: no scroll");
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::Later,
        &mut announcer,
    );
    assert!(
        state.scroll_to_cursor,
        "an edit may have moved the note out of view"
    );
}

#[test]
fn deleting_a_note_brings_the_note_that_takes_its_place_into_view() {
    let mut project = Project::new("Delete");
    let note = |start_tick| Note {
        pitch: 60,
        velocity: 100,
        start_tick,
        duration_ticks: 120,
    };
    project
        .add_track(Track {
            notes: vec![note(0), note(48_000)],
            ..Track::default()
        })
        .unwrap();
    let context = sequence_context(&project, &RollState::default(), 0);
    let edited = EditedSequence::new(&project, context);
    let mut state = RollState::default();
    let mut announcer = Announcer::default();
    let track = &mut project.tracks[0];
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::Next,
        &mut announcer,
    );
    state.scroll_to_cursor = false;
    run_command(
        track,
        &mut state,
        &edited,
        RollCommand::Delete,
        &mut announcer,
    );
    assert_eq!(track.notes, vec![note(48_000)]);
    assert!(
        state.scroll_to_cursor,
        "the far note took the deleted note's index and must be brought into view"
    );
}

#[test]
fn the_scroll_target_clears_the_fixed_ruler_and_keyboard() {
    let note_rect = Rect::from_min_size(Pos2::new(500.0, 300.0), Vec2::new(40.0, 16.0));
    let target = drawing::scroll_target(note_rect);
    assert_eq!(target.max, note_rect.max);
    assert_eq!(target.left(), note_rect.left() - drawing::KEYBOARD_WIDTH);
    assert_eq!(target.top(), note_rect.top() - drawing::RULER_HEIGHT);
}

#[test]
fn a_restored_cursor_is_one_scroll_not_two() {
    let mut project = Project::new("Restore");
    for _ in 0..2 {
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
    }
    let mut state = RollState::default();
    assert!(
        state.scroll_to_middle,
        "a fresh roll asks to scroll to the middle"
    );
    state.restore(&project, 1, None, 0, Some(0));
    assert!(state.scroll_to_cursor && !state.scroll_to_middle);
    state.scroll_to_cursor = false;
    state.restore(&project, 0, None, 0, None);
    assert!(state.scroll_to_middle && !state.scroll_to_cursor);
}
