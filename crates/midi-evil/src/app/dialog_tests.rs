//! Every dialog the application opens, answered by the harness: what each asks, what the
//! command does once it has answered, and that the commands which silence the keyboard and the
//! beats before a dialog still do.

use super::test_harness::*;
use super::*;
use evil_project::{Part, Pattern};

/// A command run outside a frame, as a menu item or a key would run it.
type Command = fn(&mut StudioApp);

const WAV_OR_AIFF: (&str, &[&str]) = ("WAV / AIFF audio", &["wav", "aif", "aiff", "aifc"]);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/public-domain/bach-invention-02")
        .join(name)
}

/// Eight seconds of clicks at 120 beats per minute, which the recovery hears as fifteen onsets.
fn write_pulse(path: &Path) {
    let sample_rate = 48_000;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _beat in 0..16 {
        for frame in 0..sample_rate as usize / 2 {
            let value: i16 = if frame < 800 { 8_000 } else { 0 };
            writer.write_sample(value).unwrap();
        }
    }
    writer.finalize().unwrap();
}

fn folder(directory: &Path) -> Asked {
    Asked {
        kind: DialogKind::PickFolder,
        title: None,
        description: None,
        filters: Vec::new(),
        directory: Some(directory.to_path_buf()),
        file_name: None,
    }
}

impl Harness {
    fn heard_after_a_frame(&mut self) -> Vec<String> {
        self.run(vec![]);
        self.announcements()
    }

    /// A held note on the keyboard, the sustain pedal down, the pitch bent, and a pad held.
    fn hold_a_note_and_a_beat(&mut self) {
        let app = &mut self.app;
        app.keyboard_binding = Some((0, "lead".to_owned()));
        app.pending_midi_notes
            .push(crate::keyboard::NoteEvent::On(60, 100));
        app.keyboard.sustain = true;
        app.keyboard.bend = 0.5;
        app.beats.midi_input(0, true, 100);
        assert!(app.beats.is_sounding());
    }

    fn keyboard_and_beats_are_silent(&self) -> bool {
        let app = &self.app;
        app.keyboard_binding.is_none()
            && app.pending_midi_notes.is_empty()
            && !app.keyboard.sustain
            && app.keyboard.bend == 0.0
            && !app.beats.is_sounding()
    }
}

#[test]
fn relink_midi_reaches_past_its_dialog_and_the_next_save_writes_the_new_link() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("bach-invention-02.mid");
    let renamed = directory.path().join("bach-invention-02-renamed.mid");
    std::fs::copy(fixture("bach-invention-02.mid"), &original).unwrap();
    std::fs::copy(fixture("bach-invention-02-renamed.mid"), &renamed).unwrap();
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.import_midi_path(&original);
    harness.run(vec![]);

    harness.dialogs.answer(Answer::Nothing);
    harness.click("Project");
    harness.run(vec![]);
    harness.click("Relink MIDI…");
    assert_eq!(harness.announcements(), ["The MIDI link is unchanged."]);
    assert_eq!(harness.app.source_midi.as_deref(), Some(original.as_path()));

    harness.dialogs.answer(Answer::Paths(vec![renamed.clone()]));
    harness.click("Project");
    harness.run(vec![]);
    harness.click("Relink MIDI…");
    assert_eq!(
        harness.announcements(),
        ["Relinked MIDI to bach-invention-02-renamed.mid."]
    );
    let midi_file = Asked::file(
        DialogKind::PickFile,
        ("Standard MIDI File", &["mid", "midi"]),
        None,
        None,
    );
    assert_eq!(harness.dialogs.take_asked(), [midi_file.clone(), midi_file]);
    assert!(harness.app.dirty);

    harness.app.save_project();
    let heard = harness.heard_after_a_frame();
    assert!(
        heard.len() == 1
            && heard[0].starts_with("Saved bach-invention-02.evil and exported ")
            && heard[0].ends_with(" notes to bach-invention-02-renamed.mid."),
        "{heard:?}"
    );
    let saved = Project::load(directory.path().join("bach-invention-02.evil")).unwrap();
    assert_eq!(
        saved.source_midi.as_deref(),
        Some(renamed.to_string_lossy().as_ref())
    );
}

#[test]
fn choose_wav_and_create_draft_reach_past_their_dialogs_and_the_draft_opens() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("pulse.wav");
    write_pulse(&source);
    let draft = directory.path().join("recovered-rhythm.evil");
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.recovery.open = true;
    harness.run(vec![]);
    harness.run(vec![]);

    harness.dialogs.answer(Answer::Nothing);
    harness.click("Choose WAV…");
    assert!(harness.announcements().is_empty(), "a cancelled choice");
    harness.dialogs.answer(Answer::Paths(vec![source.clone()]));
    harness.click("Choose WAV…");
    assert_eq!(
        harness.announcements(),
        [format!("Chose {}.", source.display())]
    );
    let wav_file = Asked::file(DialogKind::PickFile, ("WAV audio", &["wav"]), None, None);
    assert_eq!(harness.dialogs.take_asked(), [wav_file.clone(), wav_file]);

    harness.run(vec![]);
    harness.dialogs.answer(Answer::Paths(vec![draft.clone()]));
    harness.click("Create .evil draft…");
    assert_eq!(
        harness.dialogs.take_asked(),
        [Asked::file(
            DialogKind::SaveFile,
            ("MIDI Evil project", &["evil"]),
            None,
            Some("recovered-rhythm.evil"),
        )]
    );
    let mut heard = harness.announcements();
    for _ in 0..200 {
        if heard.iter().any(|text| text.starts_with("Draft created")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        harness.run(vec![]);
        heard.extend(harness.announcements());
    }
    assert_eq!(
        heard.first().map(String::as_str),
        Some("Analyzing the audio and creating the draft…")
    );
    assert!(
        heard
            .iter()
            .any(|text| text.starts_with("Draft created: 15 notes at ")
                && text.ends_with(" beats per minute; Open recovered draft opens it.")),
        "{heard:?}"
    );
    assert_eq!(Project::load(&draft).unwrap().tracks[0].notes.len(), 15);

    harness.run(vec![]);
    harness.click("Open recovered draft");
    assert_eq!(
        harness.announcements(),
        ["Opened recovered-rhythm.evil, 1 track."]
    );
    assert_eq!(harness.app.project_path.as_deref(), Some(draft.as_path()));
}

/// The eleven dialogs that come through `file_dialog`, by the twelve commands that open them
/// (Render WAV… and Render section WAV… share one), each answered "nothing chosen". Their
/// cancelled paths silence nothing themselves, so a keyboard and beats found silent after the
/// answer were silenced in front of the dialog.
/// The request for a file dialog is built with the keyboard and the beats already silent, and
/// nothing has been asked yet: the silence is in front of the dialog, not behind its answer.
#[test]
fn a_file_dialog_is_silent_before_anything_is_asked() {
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.run(vec![]);
    harness.hold_a_note_and_a_beat();
    drop(harness.app.file_dialog());
    assert!(harness.keyboard_and_beats_are_silent());
    assert_eq!(harness.dialogs.take_asked(), []);
}

#[test]
fn the_eleven_dialogs_that_silence_first_still_do_and_ask_what_they_asked() {
    let base = tempfile::tempdir().unwrap();
    let base = base.path();
    let mut project = project_with(&[&[note_at(60, 0, 480)]]);
    let pattern = project
        .add_pattern(Pattern {
            length_ticks: 480,
            notes: vec![note_at(60, 0, 480)],
            ..Pattern::default()
        })
        .unwrap();
    project
        .add_part(
            0,
            Part {
                pattern,
                start_tick: 0,
                length_ticks: 480,
            },
        )
        .unwrap();
    let mut harness = Harness::new(project);
    harness.app.project_base = Some(base.to_path_buf());
    harness.app.source_midi = Some(base.join("song.mid"));
    assert!(harness.app.open_patch_lab());
    harness.app.view = View::Notes;
    harness.run(vec![]);
    harness.run(vec![]);
    harness.announcements();

    let projects = ("MIDI Evil Projects", &["evil", "rmp"][..]);
    let midi = ("Standard MIDI File", &["mid", "midi"][..]);
    let wav = ("WAV audio", &["wav"][..]);
    let patch = ("Synth patch", &["json"][..]);
    use DialogKind::{PickFile, PickFiles, SaveFile};
    let commands: [(Command, Asked, Option<&str>); 12] = [
        (
            StudioApp::open_project,
            Asked::file(PickFile, projects, Some(base), None),
            Some("Nothing opened."),
        ),
        (
            StudioApp::save_project_as,
            Asked::file(SaveFile, projects, Some(base), Some("Untitled.evil")),
            Some("Not saved."),
        ),
        (
            StudioApp::relink_midi,
            Asked::file(PickFile, midi, Some(base), None),
            Some("The MIDI link is unchanged."),
        ),
        (
            StudioApp::export_midi,
            Asked::file(
                SaveFile,
                ("Standard MIDI File", &["mid"]),
                Some(base),
                Some("Untitled.mid"),
            ),
            Some("Nothing exported."),
        ),
        (
            StudioApp::import_midi,
            Asked::file(PickFile, midi, Some(base), None),
            Some("Nothing imported."),
        ),
        (
            StudioApp::render_audio,
            Asked::file(SaveFile, wav, Some(base), Some("Untitled.wav")),
            Some("Nothing rendered."),
        ),
        (
            StudioApp::render_section_audio,
            Asked::file(SaveFile, wav, Some(base), Some("Section.wav")),
            Some("Nothing rendered."),
        ),
        (
            StudioApp::render_track_stems,
            folder(base),
            Some("Nothing rendered."),
        ),
        (
            StudioApp::render_arrangement_parts,
            folder(base),
            Some("Nothing rendered."),
        ),
        (
            |app| {
                let mut lab = app.patch_lab.take().unwrap();
                app.save_patch_lab(&mut lab);
                app.patch_lab = Some(lab);
            },
            Asked::file(SaveFile, patch, Some(base), Some("lead-custom.json")),
            None,
        ),
        (
            StudioApp::load_sound_files,
            Asked::file(PickFiles, ("Synth", &["json"]), Some(base), None),
            Some("Nothing loaded."),
        ),
        (
            StudioApp::load_patch_draft,
            Asked::file(PickFile, patch, Some(base), None),
            None,
        ),
    ];
    for (index, (command, asked, heard)) in commands.into_iter().enumerate() {
        harness.hold_a_note_and_a_beat();
        harness.dialogs.answer(Answer::Nothing);
        command(&mut harness.app);
        assert!(harness.keyboard_and_beats_are_silent(), "command {index}");
        assert_eq!(harness.dialogs.take_asked(), [asked], "command {index}");
        let expected: Vec<String> = heard.into_iter().map(str::to_owned).collect();
        assert_eq!(harness.heard_after_a_frame(), expected, "command {index}");
    }
}

#[test]
fn cancelled_batch_export_dialogs_leave_the_take_active() {
    for command in [
        StudioApp::render_track_stems as Command,
        StudioApp::render_arrangement_parts as Command,
    ] {
        let mut project = project_with(&[&[note_at(60, 0, 480)]]);
        let pattern = project
            .add_pattern(Pattern {
                length_ticks: 480,
                notes: vec![note_at(60, 0, 480)],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 480,
                },
            )
            .unwrap();
        let mut harness = Harness::new(project);
        let mut recorder = crate::recording::Recorder::new(
            crate::recording::RecordingTarget::Track(0),
            0,
            1_920,
            None,
        );
        recorder.hit(64, 100, 0, &harness.app.project);
        harness.app.recorder = Some(recorder);
        harness.app.recording_started_at = Some(std::time::Instant::now());
        harness.dialogs.answer(Answer::Nothing);

        command(&mut harness.app);

        assert!(harness.app.recorder.is_some());
        assert_eq!(harness.app.project.tracks[0].notes.len(), 1);
        assert!(harness.app.render_job.is_none());
    }
}

#[test]
fn accepted_batch_exports_commit_the_take_before_planning() {
    for arrangement_parts in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project_with(&[&[]]);
        let (pattern, target) = if arrangement_parts {
            let pattern = project
                .add_pattern(Pattern {
                    length_ticks: 480,
                    ..Pattern::default()
                })
                .unwrap();
            project
                .add_part(
                    0,
                    Part {
                        pattern,
                        start_tick: 0,
                        length_ticks: 480,
                    },
                )
                .unwrap();
            (
                Some(pattern),
                crate::recording::RecordingTarget::Part { track: 0, part: 0 },
            )
        } else {
            (None, crate::recording::RecordingTarget::Track(0))
        };
        let mut harness = Harness::new(project);
        let mut recorder = crate::recording::Recorder::new(target, 0, 1_920, None);
        recorder.hit(64, 100, 0, &harness.app.project);
        harness.app.recorder = Some(recorder);
        harness.app.recording_started_at = Some(std::time::Instant::now());
        harness
            .dialogs
            .answer(Answer::Paths(vec![directory.path().to_path_buf()]));

        if arrangement_parts {
            harness.app.render_arrangement_parts();
            assert_eq!(
                harness.app.project.patterns[pattern.unwrap()].notes.len(),
                1
            );
        } else {
            harness.app.render_track_stems();
            assert_eq!(harness.app.project.tracks[0].notes.len(), 1);
        }
        assert!(harness.app.recorder.is_none());
        assert!(harness.app.render_job.is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while harness.app.render_job.is_some() && std::time::Instant::now() < deadline {
            harness.run(vec![]);
            std::thread::yield_now();
        }
        assert!(harness.app.render_job.is_none());
        assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    }
}

#[test]
fn refused_or_empty_batch_exports_do_not_open_a_dialog_or_commit_the_take() {
    let directory = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(project_with(&[&[evil_project::Note {
        duration_ticks: 50_000,
        ..evil_project::Note::default()
    }]]));
    harness
        .app
        .start_render(PendingRender::Wav(directory.path().join("active.wav")));
    let mut recorder = crate::recording::Recorder::new(
        crate::recording::RecordingTarget::Track(0),
        0,
        1_920,
        None,
    );
    recorder.hit(64, 100, 0, &harness.app.project);
    harness.app.recorder = Some(recorder);
    harness.app.recording_started_at = Some(std::time::Instant::now());
    for command in [
        StudioApp::render_audio as Command,
        StudioApp::render_section_audio,
        StudioApp::render_track_stems,
        StudioApp::render_arrangement_parts,
    ] {
        command(&mut harness.app);
        assert!(harness.dialogs.take_asked().is_empty());
        assert!(harness.app.recorder.is_some());
    }
    harness.app.cancel_active_render();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while harness.app.render_job.is_some() && std::time::Instant::now() < deadline {
        harness.app.poll_render();
        std::thread::yield_now();
    }

    let mut empty = Harness::new(project_with(&[&[]]));
    empty.app.recorder = Some(crate::recording::Recorder::new(
        crate::recording::RecordingTarget::Track(0),
        0,
        1_920,
        None,
    ));
    empty.app.recording_started_at = Some(std::time::Instant::now());
    empty.app.render_track_stems();
    assert!(empty.dialogs.take_asked().is_empty());
    assert!(empty.app.recorder.is_some());
}

#[test]
fn the_drum_banks_dialogs_ask_what_they_asked_and_a_dirty_bank_asks_first() {
    let directory = tempfile::tempdir().unwrap();
    let pulse = directory.path().join("pulse.wav");
    write_pulse(&pulse);
    let bank = directory.path().join("kit.evilkit");
    let pad = directory.path().join("pad.wav");
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.view = View::Pads;
    harness.run(vec![]);
    harness.run(vec![]);
    let bank_file = ("Evil drum bank", &["evilkit"][..]);

    harness.dialogs.answer(Answer::Nothing);
    harness.click("Open bank");
    harness.dialogs.answer(Answer::Paths(vec![bank.clone()]));
    harness.click("Save bank");
    assert_eq!(
        harness.dialogs.take_asked(),
        [
            Asked::file(DialogKind::PickFile, bank_file, None, None),
            Asked::file(
                DialogKind::SaveFile,
                bank_file,
                None,
                Some("maxwell-kit.evilkit")
            ),
        ]
    );
    assert!(evil_beats::DrumKit::read(&bank).is_ok());

    harness.click("EDIT PAD");
    harness.run(vec![]);
    harness.dialogs.answer(Answer::Paths(vec![pulse]));
    harness.click("Load WAV/AIFF");
    assert!(harness.app.beats.has_unsaved_changes());
    harness.run(vec![]);
    harness.dialogs.answer(Answer::Paths(vec![pad.clone()]));
    harness.click("Save WAV");
    assert_eq!(
        harness.dialogs.take_asked(),
        [
            Asked::file(DialogKind::PickFile, WAV_OR_AIFF, None, None),
            Asked::file(
                DialogKind::SaveFile,
                ("WAV audio", &["wav"]),
                None,
                Some("pad.wav")
            ),
        ]
    );
    assert!(pad.exists());

    // The toolbar's transport button is above the page's tab and carries the same name.
    harness.click_in_reading_order("PLAY", 1);
    harness.run(vec![]);
    harness.dialogs.answer(Answer::No);
    harness.click("New bank");
    assert!(harness.app.beats.has_unsaved_changes(), "no keeps the bank");
    harness.run(vec![]);
    harness.dialogs.answer(Answer::Yes);
    harness.click("New bank");
    assert!(!harness.app.beats.has_unsaved_changes(), "yes replaces it");
    let replace = Asked::confirm(
        "Unsaved drum bank",
        "Replace this bank and discard its unsaved changes?",
    );
    assert_eq!(harness.dialogs.take_asked(), [replace.clone(), replace]);
}

#[test]
fn the_patch_labs_sample_dialogs_ask_what_they_asked() {
    let directory = tempfile::tempdir().unwrap();
    let pulse = directory.path().join("pulse.wav");
    write_pulse(&pulse);
    let zone = directory.path().join("zone.wav");
    write_pulse(&zone);
    let mut harness = Harness::new(project_with(&[&[note_at(60, 0, 480)]]));
    harness.app.view = View::Synth;
    harness.run(vec![]);
    harness.app.patch_lab.as_mut().unwrap().page = patch_lab::PatchPage::Samples;
    harness.run(vec![]);
    harness.run(vec![]);

    harness.dialogs.answer(Answer::Paths(vec![pulse]));
    harness.click_in_reading_order("Load sample audio", 0);
    let sampled =
        |harness: &Harness| harness.app.patch_lab.as_ref().unwrap().patch.oscillators[0].clone();
    assert_eq!(sampled(&harness).waveform, evil_synth::Waveform::Sample);
    harness.run(vec![]);
    harness.click("Add key zone");
    harness.run(vec![]);
    harness.dialogs.answer(Answer::Paths(vec![zone]));
    harness.click_in_reading_order("Load / replace zone WAV", 0);
    let zones = sampled(&harness).sample_zones;
    assert_eq!(zones[0].sample.name, "zone");
    assert_eq!(zones[1].sample.name, "pulse");
    let audio = Asked::file(DialogKind::PickFile, WAV_OR_AIFF, None, None);
    assert_eq!(harness.dialogs.take_asked(), [audio.clone(), audio]);
}

#[test]
fn each_confirm_carries_its_title_and_description_and_only_yes_goes_ahead() {
    let mut harness = Harness::new(project_with(&[
        &[note_at(60, 0, 480)],
        &[note_at(62, 0, 480)],
    ]));
    harness.run(vec![]);
    harness.announcements();

    harness.app.dirty = true;
    harness.dialogs.answer(Answer::No);
    harness.app.new_project();
    assert_eq!(
        harness.heard_after_a_frame(),
        ["Kept the unsaved changes; no new project."]
    );
    harness.dialogs.answer(Answer::Yes);
    harness.app.new_project();
    assert_eq!(
        harness.heard_after_a_frame(),
        ["Discarded the unsaved changes. New project, 1 track."]
    );
    let discard = Asked::confirm("Unsaved changes", "Discard unsaved changes?");
    assert_eq!(harness.dialogs.take_asked(), [discard.clone(), discard]);

    harness.app.project = project_with(&[&[note_at(60, 0, 480)], &[note_at(62, 0, 480)]]);
    harness.app.reset_history();
    assert!(harness.app.open_patch_lab());
    harness.app.patch_lab.as_mut().unwrap().patch.gain = 0.123;
    harness.dialogs.answer(Answer::No);
    harness.app.remove_selected_track();
    assert_eq!(harness.app.project.tracks.len(), 2, "no keeps the track");
    harness.dialogs.answer(Answer::Yes);
    harness.app.remove_selected_track();
    assert_eq!(harness.app.project.tracks.len(), 1, "yes removes it");
    let replace = Asked::confirm(
        "Unsaved instrument edits",
        "Replace the current sound draft? Save it in Synth or Pads first to keep your edits.",
    );
    assert_eq!(harness.dialogs.take_asked(), [replace.clone(), replace]);

    assert!(harness.app.open_patch_lab());
    harness.app.patch_lab.as_mut().unwrap().patch.gain = 0.456;
    harness.app.history.record(
        harness.app.project.clone(),
        harness.app.editor_selection(),
        None,
    );
    harness.app.project.instruments[0].source = InstrumentSource::Synth {
        patch: "bass".into(),
    };
    let bass = harness.app.project.instruments[0].source.clone();
    harness.dialogs.answer(Answer::No);
    harness.app.undo();
    assert_eq!(
        harness.app.project.instruments[0].source, bass,
        "no keeps it"
    );
    harness.dialogs.answer(Answer::Yes);
    harness.app.undo();
    assert_ne!(
        harness.app.project.instruments[0].source, bass,
        "yes undoes"
    );
    let history = Asked::confirm(
        "Unsaved instrument drafts",
        "This undo or redo changes the project's instruments. Discard unsaved instrument drafts? Save them first to keep your edits.",
    );
    assert_eq!(harness.dialogs.take_asked(), [history.clone(), history]);
}
