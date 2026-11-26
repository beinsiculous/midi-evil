//! Every command says what happened: the track panel, the file commands, the roll's toolbar,
//! the sound library, and the rename that belongs to the track it was made on.

use super::song_tests::song_harness;
use super::test_harness::*;
use super::*;

#[test]
fn the_track_panel_says_what_adding_renaming_muting_and_a_sound_did() {
    let mut harness = song_harness();
    harness.click("Add a synth track");
    assert_eq!(
        harness.announcements(),
        ["Added Track 3, a synth track with the Lead sound."]
    );
    harness.run(vec![]);
    let (field, _) = harness.node("Track name");
    harness.action(field, egui::accesskit::Action::Focus, None);
    harness.run(vec![egui::Event::Text("x".into())]);
    assert!(
        harness.announcements().is_empty(),
        "a keystroke in the field is the field's own"
    );
    harness.run(vec![plain(egui::Key::Tab)]);
    let renamed = harness.app.project.tracks[2].name.clone();
    assert_ne!(renamed, "Track 3");
    assert_eq!(
        harness.announcements(),
        [format!("Renamed Track 3 to {renamed}.")]
    );
    harness.run(vec![]);
    harness.click("Mute track");
    assert_eq!(harness.announcements(), [format!("{renamed} muted.")]);
    harness.run(vec![]);
    harness.click("Mute track");
    assert_eq!(harness.announcements(), [format!("{renamed} unmuted.")]);
}

#[test]
fn a_new_project_with_nothing_unsaved_says_so_and_a_save_says_where() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("song.evil");
    let mut harness = song_harness();
    harness.app.project_path = Some(path.clone());
    harness.run(vec![press(egui::Key::S, CONTROL)]);
    assert_eq!(harness.announcements(), ["Saved song.evil."]);
    harness.run(vec![]);
    harness.run(vec![press(egui::Key::N, CONTROL)]);
    assert_eq!(harness.announcements(), ["New project, 1 track."]);
    harness.run(vec![]);
    harness.app.open_project_path(&path);
    harness.run(vec![]);
    assert_eq!(harness.announcements(), ["Opened song.evil, 2 tracks."]);
}

#[test]
fn a_roll_key_is_announced_once_though_the_status_line_repeats_the_rolls_name() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 480),
        note_at(62, 480, 480),
    ]]));
    harness.focus_roll();
    harness.run(vec![plain(egui::Key::ArrowRight)]);
    harness.run(vec![]);
    harness.run(vec![plain(egui::Key::Delete)]);
    assert_eq!(harness.announcements(), ["Removed."]);
    assert!(harness.app.status.contains("Piano roll, "));
    harness.run(vec![]);
    assert!(harness.announcements().is_empty());
}

fn announcements_until_rendered(harness: &mut Harness) -> Vec<String> {
    let mut heard = Vec::new();
    for _ in 0..200 {
        harness.run(vec![]);
        heard.extend(harness.announcements());
        if harness.app.render_job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    heard
}

#[test]
fn what_a_dialog_command_does_once_its_dialog_has_answered_is_said() {
    let directory = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    let midi = directory.path().join("song.mid");
    harness.app.export_midi_to(&midi);
    assert_eq!(
        harness.announcements_after_a_frame(),
        ["Exported 1 notes from 1 tracks to song.mid."]
    );
    let wav = directory.path().join("song.wav");
    harness.app.start_render(PendingRender::Wav(wav.clone()));
    let heard = announcements_until_rendered(&mut harness);
    assert_eq!(
        heard.first().map(String::as_str),
        Some("Rendering in the background…")
    );
    assert!(
        heard
            .iter()
            .any(|text| text.starts_with("Rendered 1 notes from 1 tracks to song.wav")),
        "{heard:?}"
    );
    let plan = evil_render::stem_export_plan(&harness.app.project, None).unwrap();
    harness
        .app
        .start_wav_export_plan(directory.path().to_path_buf(), plan);
    let heard = announcements_until_rendered(&mut harness);
    assert!(
        heard[0].starts_with("Rendering export 1 of 1: "),
        "{heard:?}"
    );
    assert!(
        heard
            .iter()
            .any(|text| text.starts_with("Exported 1 WAV file(s)")),
        "{heard:?}"
    );
    let bach = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/public-domain/bach-invention-02/bach-invention-02.mid");
    harness.app.dirty = false;
    harness.app.import_midi_path(&bach);
    let heard = harness.announcements_after_a_frame();
    assert!(
        heard[0].starts_with("Imported bach-invention-02.mid, 2 tracks, "),
        "{heard:?}"
    );
}

#[test]
fn a_failed_batch_and_refused_bank_save_are_announced() {
    let directory = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    let missing = harness
        .app
        .project
        .add_instrument(Instrument::synth("Missing", "missing.json"))
        .unwrap();
    harness
        .app
        .project
        .add_track(Track {
            instrument: missing,
            notes: vec![note_at(62, 0, 480)],
            ..Track::default()
        })
        .unwrap();
    let plan = evil_render::stem_export_plan(&harness.app.project, None).unwrap();
    harness
        .app
        .start_wav_export_plan(directory.path().to_path_buf(), plan);
    let heard = announcements_until_rendered(&mut harness);
    assert!(
        heard.iter().any(|text| {
            text.starts_with("Export batch stopped before any output; could not resolve instrument source missing.json")
        }),
        "{heard:?}"
    );

    harness.app.view = View::Pads;
    let queued = harness
        .app
        .accept_render(PendingRender::Wav(directory.path().join("queued.wav")));
    harness.app.queued_render = Some(queued);
    harness.run(vec![press(egui::Key::S, CONTROL)]);
    assert_eq!(
        harness.announcements(),
        ["Withdraw the queued WAV export before saving a bank"]
    );
}

#[test]
fn cancelled_exports_and_same_path_retry_do_not_repeat_composed_outcomes() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.wav");
    let second = directory.path().join("second.wav");
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    let mut heard = Vec::new();
    for output in [&first, &second] {
        let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_hook = gate.clone();
        let block_once = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let block_once_for_hook = block_once.clone();
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        harness.app.render_owner.set_result_test_hook(move || {
            if block_once_for_hook.swap(false, std::sync::atomic::Ordering::AcqRel) {
                entered_sender.send(()).unwrap();
                let (lock, condition) = &*gate_for_hook;
                let mut allowed = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                while !*allowed {
                    allowed = condition
                        .wait(allowed)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        });
        harness.app.start_render(PendingRender::Wav(output.clone()));
        entered_receiver.recv().unwrap();
        assert!(output.exists());
        harness.app.cancel_active_render();
        let (lock, condition) = &*gate;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        condition.notify_all();
        heard.extend(announcements_until_rendered(&mut harness));
    }
    harness.app.start_render(PendingRender::Wav(first.clone()));
    heard.extend(announcements_until_rendered(&mut harness));

    for output in [&first, &second] {
        let published = format!("Published WAV export to {}", output.display());
        assert_eq!(
            heard
                .iter()
                .filter(|text| text.contains(&published))
                .count(),
            1,
            "{heard:?}"
        );
    }
    let rendered = "Rendered 1 notes from 1 tracks to first.wav";
    assert_eq!(
        heard
            .iter()
            .filter(|text| text.starts_with(rendered))
            .count(),
        1,
        "{heard:?}"
    );
    assert!(
        heard.iter().all(|text| {
            !(text.contains("Published WAV export") && text.contains("Rendered 1 notes"))
        }),
        "{heard:?}"
    );
}

#[test]
fn the_rolls_whole_sequence_buttons_say_what_they_did() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 470),
        note_at(62, 480, 480),
    ]]));
    harness.run(vec![]);
    for (button, heard) in [
        ("Quantize", "Quantized all 2 notes."),
        ("+1", "Moved all 2 notes a semitone up."),
        ("Copy", "Copied 2 notes."),
        ("Quantize", "Nothing changed."),
    ] {
        harness.click(button);
        let mut announced = harness.announcements();
        harness.run(vec![]);
        announced.extend(harness.announcements());
        assert_eq!(announced, [heard], "{button}");
    }
}

impl Harness {
    /// The announcements of the frame that follows a command run outside a frame.
    fn announcements_after_a_frame(&mut self) -> Vec<String> {
        self.run(vec![]);
        self.announcements()
    }
}

#[test]
fn forgetting_a_library_entry_says_so_and_leaves_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mine.json");
    std::fs::write(
        &path,
        evil_synth::Patch::default().to_json_pretty().unwrap(),
    )
    .unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    let sound = crate::instruments::Sound::file(&path, crate::instruments::SoundKind::Synth);
    harness.app.sounds.remember(sound.clone());
    harness.app.sounds.open = true;
    harness.run(vec![]);
    harness.run(vec![]);
    harness.click("Forget library entry");
    let heard = harness.announcements();
    assert!(
        heard.len() == 1
            && heard[0].starts_with("Forgot ")
            && heard[0].ends_with("the file is untouched."),
        "{heard:?}"
    );
    assert!(!harness.app.sounds.is_remembered(&sound));
    assert!(path.exists());
}

#[test]
fn clean_overlaps_says_it_acted_on_every_note_even_with_one_selected() {
    let mut harness = Harness::new(project_with(&[&[
        note_at(60, 0, 960),
        note_at(60, 480, 480),
    ]]));
    harness.focus_roll();
    harness.key(plain(egui::Key::ArrowRight));
    harness.click("Clean overlaps");
    let mut heard = harness.announcements();
    harness.run(vec![]);
    heard.extend(harness.announcements());
    assert_eq!(heard, ["Cleaned the overlaps of all 2 notes."]);
}

#[test]
fn a_rename_is_said_for_the_track_it_was_made_on_even_when_the_selection_moves_under_it() {
    let mut harness = song_harness();
    let (field, _) = harness.node("Track name");
    harness.action(field, egui::accesskit::Action::Focus, None);
    harness.run(vec![egui::Event::Text("x".into())]);
    let first = harness.app.project.tracks[0].name.clone();
    assert_ne!(first, "Track 1");
    harness.click("02   Track 2");
    assert_eq!(
        harness.status_and_heard(),
        format!("Renamed Track 1 to {first}.")
    );
    harness.run(vec![egui::Event::Text("y".into())]);
    harness.run(vec![plain(egui::Key::Tab)]);
    let second = harness.app.project.tracks[1].name.clone();
    assert_ne!(second, "Track 2");
    assert_eq!(
        harness.announcements(),
        [format!("Renamed Track 2 to {second}.")]
    );
    assert_eq!(
        harness.app.project.tracks[0].name, first,
        "the first rename stands"
    );

    harness.click(&format!("01   {first}"));
    harness.run(vec![]);
    let (field, _) = harness.node("Track name");
    harness.action(field, egui::accesskit::Action::Focus, None);
    harness.run(vec![egui::Event::Text("z".into())]);
    harness.app.dirty = false;
    harness.app.new_project();
    harness.run(vec![]);
    harness.run(vec![plain(egui::Key::Tab)]);
    assert!(
        harness
            .announcements()
            .iter()
            .all(|text| !text.starts_with("Renamed")),
        "a new project drops the old document's rename"
    );
}

impl Harness {
    /// The status line, which the last report wrote, checked against what was announced.
    fn status_and_heard(&self) -> String {
        assert!(
            self.announcements().contains(&self.app.status),
            "{:?} not in {:?}",
            self.app.status,
            self.announcements()
        );
        self.app.status.clone()
    }
}

#[test]
fn loading_sound_files_reports_only_what_this_load_came_to() {
    use crate::instruments::SoundKind;
    let directory = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.status = "An older outcome".into();
    harness.app.load_sound_paths(&[], SoundKind::Synth);
    harness.run(vec![]);
    assert_eq!(harness.announcements(), ["Nothing loaded."]);
    let mine = directory.path().join("mine.json");
    std::fs::write(
        &mine,
        evil_synth::Patch::default().to_json_pretty().unwrap(),
    )
    .unwrap();
    let missing = directory.path().join("missing.json");
    harness
        .app
        .load_sound_paths(&[mine, missing.clone()], SoundKind::Synth);
    harness.run(vec![]);
    let heard = harness.announcements();
    assert!(
        heard.len() == 1
            && heard[0].starts_with("Loaded 1 user instrument(s); Previous / Next cycles")
            && heard[0].contains(&format!("could not load {}", missing.display())),
        "{heard:?}"
    );
}
