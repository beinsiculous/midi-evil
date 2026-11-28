//! What the browser build shows: nothing the platform's list leaves out is drawn, a request for a
//! view it leaves out stays where it is, by every command that can ask for one, and no stop is
//! left that only a pointer could drag. The desktop draws all of it.

use super::front_end_tests::*;
use super::test_harness::*;
use super::*;

/// The names of what the browser leaves out, as the desktop draws them.
const LEFT_OUT: [&str; 25] = [
    "MIXER",
    "SYNTH",
    "KEYBOARD",
    "PADS",
    "Add a Maxwell drum-kit track",
    "Open Evil Synth",
    "Open Pads",
    "Samples",
    "Modulation",
    "Mixer & FX",
    "Automation",
    "MIDI input port",
    "Refresh MIDI inputs",
    "Import MIDI…",
    "Recover rhythm from WAV…",
    "Relink MIDI…",
    "Export MIDI…",
    "Render WAV…",
    "Render section WAV…",
    "Track stems…",
    "Arrangement parts…",
    "Export",
    "Standard MIDI…",
    "Stereo WAV…",
    "Load files…",
];

fn drum_and_synth_project() -> Project {
    let mut project = three_notes();
    let instrument = project
        .add_instrument(Instrument::drum_kit("Drums", "maxwell"))
        .unwrap();
    project
        .add_track(Track {
            name: "Drums".into(),
            instrument,
            notes: vec![note_at(36, 0, 120)],
            ..Track::default()
        })
        .unwrap();
    project
}

/// Every name in the tree, across the Song and Notes views, the Project menu, the About window,
/// the Keyboard notes, the sound library and the export row, with a note under the cursor and a
/// drum track and a synth track each selected in turn.
fn every_name(harness: &mut Harness) -> Vec<String> {
    let mut names = Vec::new();
    let mut collect = |harness: &mut Harness| {
        harness.run(vec![]);
        harness.run(vec![]);
        names.extend(
            harness
                .nodes()
                .iter()
                .filter_map(|node| node.label().map(str::to_owned)),
        );
    };
    harness.app.source_midi = Some(PathBuf::from("song.mid"));
    harness.app.show_export = true;
    harness.app.show_automation = true;
    for track in [0, 1] {
        harness.app.select_sound_track(track);
        for view in [View::Song, View::Notes] {
            harness.app.select_view(view);
            collect(harness);
        }
    }
    harness.app.select_sound_track(0);
    harness.app.select_view(View::Notes);
    harness.key(plain(egui::Key::ArrowRight));
    collect(harness);
    harness.click("Project");
    collect(harness);
    harness.app.show_about = true;
    harness.app.show_key_help = true;
    harness.app.sounds.open = true;
    collect(harness);
    names
}

#[test]
fn the_browser_draws_nothing_its_list_leaves_out_and_the_desktop_draws_it_all() {
    let mut desktop = Harness::new(drum_and_synth_project());
    let drawn = every_name(&mut desktop);
    for name in LEFT_OUT {
        assert!(
            drawn.iter().any(|drawn| drawn == name),
            "the desktop draws {name:?}"
        );
    }
    let mut harness = browser(drum_and_synth_project());
    let drawn = every_name(&mut harness);
    for name in LEFT_OUT {
        assert!(
            !drawn.iter().any(|drawn| drawn == name),
            "the browser draws {name:?}"
        );
    }
    for name in [
        "Add a synth track",
        "Track actions",
        "Undo",
        "Keyboard notes…",
    ] {
        assert!(
            drawn.iter().any(|drawn| drawn == name),
            "the browser draws {name:?}"
        );
    }
}

#[test]
fn in_the_browser_a_request_for_a_view_it_leaves_out_stays_where_it_is_by_every_command() {
    let hidden = [View::Mixer, View::Synth, View::Keyboard, View::Pads];
    let mut harness = browser(drum_and_synth_project());
    for start in [View::Song, View::Notes] {
        assert!(harness.app.select_view(start));
        for view in hidden {
            assert!(!harness.app.select_view(view), "{view:?} from {start:?}");
            assert!(harness.app.view() == start, "{view:?} from {start:?}");
        }
        harness.app.open_patch_lab();
        assert!(harness.app.view() == start, "opening the patch lab");
        harness.app.select_sound_track(1);
        harness.app.keep_view_with_sound_kind();
        assert!(harness.app.view() == start, "a drum track's sound");
        harness.app.select_sound_track(0);
        harness.app.keep_view_with_sound_kind();
        assert!(harness.app.view() == start, "a synth track's sound");
        harness.app.add_drum_track();
        assert!(harness.app.view() == start, "adding a drum track");
        harness.app.add_track_quietly().unwrap();
        assert!(harness.app.view() == start, "adding a synth track");
        harness.run(vec![]);
    }
    let mut desktop = Harness::new(drum_and_synth_project());
    desktop.app.add_drum_track();
    assert!(desktop.app.view() == View::Pads, "the desktop goes to Pads");
    for view in hidden {
        assert!(desktop.app.select_view(view), "the desktop shows {view:?}");
    }
}

#[test]
fn in_the_browser_no_window_moves_and_the_side_panel_does_not_resize() {
    // Two windows open: the stops they and the side panel's edge would add, by name in the tree
    // and by id among the frame's focusable widgets. egui makes a title bar's widget only for a
    // window that moves or collapses, and the panel's handle only while it resizes.
    let title_bars = [
        egui::Id::new("keyboard-notes").with("__title_click"),
        egui::Id::new("sound-library").with("__title_click"),
    ];
    let handle = egui::Id::new("tracks").with("__resize");
    let stops = |harness: &mut Harness| {
        harness.app.show_key_help = true;
        harness.app.sounds.open = true;
        harness.run(vec![]);
        harness.run(vec![]);
        let focusable = |stop: &egui::Id| {
            harness.context.viewport(|viewport| {
                viewport
                    .this_pass
                    .widgets
                    .get(*stop)
                    .is_some_and(|rect| rect.sense.is_focusable())
            })
        };
        let in_order: Vec<egui::Id> = title_bars
            .into_iter()
            .chain([handle])
            .filter(focusable)
            .collect();
        let named: Vec<String> = harness
            .nodes()
            .iter()
            .filter_map(|node| node.label().map(str::to_owned))
            .filter(|name| name.contains("title bar") || name.starts_with("Side panel width"))
            .collect();
        (in_order, named)
    };
    let mut desktop = Harness::new(three_notes());
    let (in_order, named) = stops(&mut desktop);
    for (stop, what) in title_bars
        .into_iter()
        .zip([
            "the Keyboard notes' title bar",
            "the Sound library's title bar",
        ])
        .chain([(handle, "the side panel's handle")])
    {
        assert!(
            in_order.contains(&stop),
            "on the desktop {what} is focusable"
        );
    }
    assert!(
        named
            .iter()
            .any(|name| name.starts_with("Keyboard notes, title bar"))
    );
    assert!(
        named
            .iter()
            .any(|name| name.starts_with("Side panel width"))
    );
    let mut harness = browser(three_notes());
    assert_eq!(stops(&mut harness), (Vec::new(), Vec::new()));
}
