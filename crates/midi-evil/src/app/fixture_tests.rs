//! The gates' tracked projects. Most are written by hand; the Bach invention's is the tracked MIDI
//! file imported, kept as a project because a browser cannot import a file, so it must go on
//! holding the same notes in the same order as an import of that file makes.

use super::*;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_tracked_bach_project_is_the_midi_file_imported_note_for_note() {
    let midi = repository().join("examples/public-domain/bach-invention-02/bach-invention-02.mid");
    let performance = evil_imports::import_midi_file(&midi).unwrap();
    let imported = Project::from_performance("bach-invention-02", &performance).unwrap();
    let tracked =
        Project::load(repository().join("coordination/web/probes/fixtures/bach.evil")).unwrap();
    assert_eq!(tracked, imported);
}

#[cfg(feature = "gate")]
#[test]
fn every_tracked_project_starts_the_workstation_from_its_text_with_no_file_behind_it() {
    let directory = repository().join("coordination/web/probes/fixtures");
    let mut opened = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
        let dialogs = Box::new(super::test_harness::ScriptedDialogs::default());
        let app = StudioApp::with_project_text(&creation, dialogs, &name, &text).unwrap();
        assert_eq!(app.project, Project::from_json(&text).unwrap(), "{name}");
        assert!(app.project_path.is_none() && !app.dirty, "{name}");
        assert!(
            app.status.starts_with(&format!("Opened {name}, ")),
            "{name}"
        );
        opened += 1;
    }
    assert_eq!(opened, 12);
    let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    let dialogs = Box::new(super::test_harness::ScriptedDialogs::default());
    assert!(StudioApp::with_project_text(&creation, dialogs, "broken.evil", "{").is_err());
}
