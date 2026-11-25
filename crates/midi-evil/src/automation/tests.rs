use super::*;
use evil_project::ControllerEvent;
use evil_project::Track;

#[test]
fn edits_keep_combined_aftertouch_lanes_ordered_and_scoped() {
    let mut track = Track::default();
    let mut state = AutomationState {
        lane: Lane::PolyAftertouch,
        poly_note: 60,
        ..AutomationState::default()
    };
    add_point(&mut track, &state, 480, 80);
    state.lane = Lane::ChannelAftertouch;
    let channel = add_point(&mut track, &state, 240, 40);
    state.selected = Some(channel);
    state.selected = update_point(&mut track, &state, channel, 720, 90);
    assert_eq!(
        track
            .aftertouch
            .iter()
            .map(|event| (event.tick, event.note, event.value))
            .collect::<Vec<_>>(),
        vec![(480, Some(60), 80), (720, None, 90)]
    );
    assert_eq!(points(&track, &state).len(), 1);
    state.lane = Lane::PolyAftertouch;
    assert_eq!(points(&track, &state).len(), 1);
    state.poly_note = 61;
    assert!(points(&track, &state).is_empty());
}

#[test]
fn pitch_bend_and_controller_values_stay_in_midi_ranges() {
    let mut track = Track::default();
    let mut state = AutomationState::default();
    let index = add_point(&mut track, &state, 0, 20_000);
    assert_eq!(track.pitch_bends_mut()[index].value, 8191);
    state.selected = Some(index);
    update_point(&mut track, &state, index, 0, -20_000);
    assert_eq!(track.pitch_bends_mut()[0].value, -8192);

    state.lane = Lane::Controller;
    state.controller = 2;
    let index = add_point(&mut track, &state, 0, 200);
    assert_eq!(track.controllers_mut()[index].value, 127);
    assert_eq!(unsupported_controller_count(&track), 1);
    assert!(render_status("Preview".to_owned(), 1, 1, 0).contains("not rendered"));
    assert!(render_status("Preview".to_owned(), 0, 0, 2).contains("outside the kit"));
}

#[test]
fn deleting_one_cc_lane_preserves_other_controllers() {
    let mut track = Track::default();
    let mut state = AutomationState {
        lane: Lane::Controller,
        controller: 1,
        ..AutomationState::default()
    };
    add_point(&mut track, &state, 0, 20);
    state.controller = 2;
    let index = add_point(&mut track, &state, 120, 30);
    state.selected = Some(index);
    assert!(remove_selected(&mut track, &mut state));
    assert_eq!(track.controllers_mut().len(), 1);
    assert_eq!(track.controllers_mut()[0].controller, 1);
    assert!(state.selected.is_none());
}

#[test]
fn controller_ramp_materializes_midi_points_without_touching_other_lanes() {
    let mut track = Track {
        controllers: vec![
            ControllerEvent {
                tick: 0,
                controller: 1,
                value: 0,
            },
            ControllerEvent {
                tick: 120,
                controller: 11,
                value: 90,
            },
            ControllerEvent {
                tick: 480,
                controller: 1,
                value: 100,
            },
            ControllerEvent {
                tick: 960,
                controller: 1,
                value: 0,
            },
        ],
        ..Track::default()
    };
    let mut state = AutomationState {
        lane: Lane::Controller,
        controller: 1,
        selected: Some(0),
        ..AutomationState::default()
    };

    assert_eq!(
        ramp_controller_to_next(&mut track, &mut state, 120),
        Some(3)
    );
    let lane: Vec<_> = points(&track, &state)
        .into_iter()
        .map(|point| (point.tick, point.value))
        .collect();
    assert_eq!(
        lane,
        [
            (0, 0),
            (120, 25),
            (240, 50),
            (360, 75),
            (480, 100),
            (960, 0)
        ]
    );
    assert!(
        track
            .controllers
            .iter()
            .any(|event| { event.controller == 11 && event.tick == 120 && event.value == 90 })
    );
    assert_eq!(state.selected, Some(0));
}

#[test]
fn descending_controller_ramp_rounds_values_and_preserves_endpoints() {
    let mut track = Track::default();
    let mut state = AutomationState {
        lane: Lane::Controller,
        controller: 1,
        ..AutomationState::default()
    };
    add_point(&mut track, &state, 0, 127);
    add_point(&mut track, &state, 480, 0);
    state.selected = Some(0);

    assert_eq!(
        ramp_controller_to_next(&mut track, &mut state, 120),
        Some(3)
    );
    assert_eq!(
        track
            .controllers
            .iter()
            .map(|event| (event.tick, event.value))
            .collect::<Vec<_>>(),
        [(0, 127), (120, 95), (240, 63), (360, 32), (480, 0)]
    );
    assert_eq!(ramp_grid(480, 16), 120);
    assert_eq!(ramp_grid(384, 64), 24);
}

/// Runs one frame of the lanes and returns the drawn lane's widget: the one painter as tall as
/// a lane that takes clicks and drags.
pub(crate) fn lane_frame(
    context: &egui::Context,
    events: Vec<egui::Event>,
    show: &mut dyn FnMut(&mut egui::Ui),
) -> egui::WidgetRect {
    let _ = context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 700.0))),
            events,
            ..Default::default()
        },
        |ui| show(ui),
    );
    context.viewport(|viewport| {
        viewport
            .prev_pass
            .widgets
            .get_layer(egui::LayerId::background())
            .find(|widget| {
                widget.sense == egui::Sense::click_and_drag()
                    && (widget.rect.height() - LANE_HEIGHT).abs() < 0.5
            })
            .copied()
            .expect("a lane was drawn")
    })
}

/// Clicks a lane's point and shows that only the focused lane takes Delete: with focus taken
/// elsewhere and the pointer still over the lane, Delete removes nothing; clicked again, it does.
pub(crate) fn delete_follows_focus(
    point: impl Fn(Rect) -> Pos2,
    point_count: &dyn Fn() -> usize,
    show: &mut dyn FnMut(&mut egui::Ui),
) {
    let context = egui::Context::default();
    lane_frame(&context, vec![], show);
    let lane = lane_frame(&context, vec![], show);
    let position = point(lane.rect);
    let click = |pressed| egui::Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let delete = egui::Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    lane_frame(
        &context,
        vec![egui::Event::PointerMoved(position), click(true)],
        show,
    );
    lane_frame(&context, vec![click(false)], show);
    assert!(
        context.memory(|memory| memory.has_focus(lane.id)),
        "a click focuses the lane"
    );
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("the roll")));
    lane_frame(
        &context,
        vec![egui::Event::PointerMoved(position), delete.clone()],
        show,
    );
    assert_eq!(
        point_count(),
        1,
        "a hovered lane without focus takes no Delete"
    );
    lane_frame(&context, vec![click(true)], show);
    lane_frame(&context, vec![click(false)], show);
    lane_frame(&context, vec![delete], show);
    assert_eq!(
        point_count(),
        0,
        "the focused lane deletes its selected point"
    );
}

#[test]
fn the_automation_lane_takes_delete_only_with_focus_which_a_click_gives_it() {
    let mut project = evil_project::Project::new("Lane");
    project
        .add_track(Track {
            pitch_bends: vec![evil_project::PitchBend {
                tick: 480,
                value: 0,
            }],
            ..Track::default()
        })
        .unwrap();
    let project = std::cell::RefCell::new(project);
    let mut state = AutomationState::default();
    let roll = RollState::default();
    let pixels_per_tick = roll.pixels_per_tick(480);
    delete_follows_focus(
        |rect| {
            let point = Point {
                index: 0,
                tick: 480,
                value: 0,
            };
            point_position(point, rect, pixels_per_tick, Lane::PitchBend)
        },
        &|| project.borrow().tracks[0].pitch_bends.len(),
        &mut |ui| {
            show(ui, &mut project.borrow_mut(), &mut state, &roll);
        },
    );
}
