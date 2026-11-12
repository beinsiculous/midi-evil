//! Sound discovery and file identity shared by the rack and the sound browser.
use std::path::{Path, PathBuf};

use evil_project::{InstrumentSource, Project};
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "midi-evil.sound-library.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundKind {
    Synth,
    Kit,
}

impl SoundKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Synth => "Synth",
            Self::Kit => "Drum kit",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Synth => "json",
            Self::Kit => "evilkit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sound {
    pub name: String,
    pub reference: String,
    pub kind: SoundKind,
    pub user: bool,
}

pub enum LoadedSound {
    Synth(Box<evil_synth::Patch>),
    Kit(Box<evil_beats::DrumKit>),
}

impl Sound {
    pub fn from_source(source: &InstrumentSource, base: &Path) -> Self {
        let (reference, kind, user) = match source {
            InstrumentSource::Synth { patch } => (
                patch,
                SoundKind::Synth,
                patch.to_ascii_lowercase().ends_with(".json"),
            ),
            InstrumentSource::DrumKit { kit, .. } => (
                kit,
                SoundKind::Kit,
                !matches!(kit.as_str(), "maxwell" | "chiptune"),
            ),
        };
        if user {
            return Self::file(&base.join(reference), kind);
        }
        let name = match kind {
            SoundKind::Synth => evil_synth::preset(reference)
                .map(|patch| patch.name)
                .unwrap_or_else(|_| reference.clone()),
            SoundKind::Kit => match reference.as_str() {
                "chiptune" => "Chiptune".to_owned(),
                _ => "Maxwell".to_owned(),
            },
        };
        Self {
            name,
            reference: reference.to_ascii_lowercase(),
            kind,
            user: false,
        }
    }

    pub fn file(path: &Path, kind: SoundKind) -> Self {
        let path = path
            .canonicalize()
            .or_else(|_| std::path::absolute(path))
            .unwrap_or_else(|_| path.to_path_buf());
        Self {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            reference: path.to_string_lossy().into_owned(),
            kind,
            user: true,
        }
    }

    pub fn same_source(&self, other: &Self) -> bool {
        self.kind == other.kind && self.reference == other.reference
    }

    pub fn load(&self) -> Result<LoadedSound, String> {
        match self.kind {
            SoundKind::Synth => evil_render::load_patch(&self.reference, Path::new("."))
                .map(|patch| LoadedSound::Synth(Box::new(patch))),
            SoundKind::Kit => evil_render::load_drum_kit(&self.reference, Path::new("."))
                .map(|kit| LoadedSound::Kit(Box::new(kit))),
        }
        .map_err(|error| error.to_string())
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.user.then(|| PathBuf::from(&self.reference))
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Collection {
    #[default]
    All,
    Factory,
    User,
}

#[derive(Default)]
pub struct SoundBrowser {
    pub open: bool,
    pub query: String,
    pub collection: Collection,
    user_sounds: Vec<Sound>,
    factory_sounds: Vec<Sound>,
}

impl SoundBrowser {
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut browser = Self {
            factory_sounds: evil_synth::PRESET_NAMES
                .iter()
                .map(|name| {
                    Sound::from_source(
                        &InstrumentSource::Synth {
                            patch: (*name).to_owned(),
                        },
                        Path::new("."),
                    )
                })
                .chain(["maxwell", "chiptune"].map(|name| {
                    Sound::from_source(
                        &InstrumentSource::DrumKit {
                            kit: name.to_owned(),
                            base_note: 48,
                        },
                        Path::new("."),
                    )
                }))
                .collect(),
            ..Self::default()
        };
        if let Some(sounds) =
            storage.and_then(|storage| eframe::get_value::<Vec<Sound>>(storage, STORAGE_KEY))
        {
            for sound in sounds {
                browser.remember(sound);
            }
        }
        browser
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.user_sounds);
    }

    pub fn remember(&mut self, sound: Sound) {
        if !sound.user {
            return;
        }
        if let Some(existing) = self
            .user_sounds
            .iter_mut()
            .find(|existing| existing.same_source(&sound))
        {
            *existing = sound;
        } else {
            self.user_sounds.push(sound);
        }
    }

    pub fn forget(&mut self, sound: &Sound) {
        self.user_sounds
            .retain(|existing| !existing.same_source(sound));
    }

    pub fn is_remembered(&self, sound: &Sound) -> bool {
        self.user_sounds
            .iter()
            .any(|existing| existing.same_source(sound))
    }

    pub fn choices(&self, project: &Project, base: &Path, kind: SoundKind) -> Vec<Sound> {
        let mut choices: Vec<Sound> = Vec::new();
        for sound in self
            .factory_sounds
            .iter()
            .chain(&self.user_sounds)
            .cloned()
            .chain(
                project
                    .instruments
                    .iter()
                    .map(|instrument| Sound::from_source(&instrument.source, base)),
            )
            .filter(|sound| sound.kind == kind)
        {
            if !choices.iter().any(|existing| existing.same_source(&sound)) {
                choices.push(sound);
            }
        }
        let query = self.query.trim().to_lowercase();
        choices.retain(|sound| {
            (match self.collection {
                Collection::All => true,
                Collection::Factory => !sound.user,
                Collection::User => sound.user,
            }) && (sound.name.to_lowercase().contains(&query)
                || sound.reference.to_lowercase().contains(&query))
        });
        choices
    }
}

/// Wrap within the displayed collection; an absent selection starts at the nearest end.
pub fn cycle<'a>(choices: &'a [Sound], current: &Sound, backwards: bool) -> Option<&'a Sound> {
    if choices.is_empty() {
        return None;
    }
    let current = choices.iter().position(|sound| sound.same_source(current));
    let index = match (current, backwards) {
        (Some(0) | None, true) => choices.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) => (index + 1) % choices.len(),
        (None, false) => 0,
    };
    choices.get(index)
}

pub fn portable_file_reference(path: &Path, base: &Path) -> String {
    let absolute_base = absolute_directory(base);
    path.strip_prefix(&absolute_base)
        .or_else(|_| path.strip_prefix(base))
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub fn rebase_project_sources(project: &mut Project, old_base: &Path, new_base: &Path) {
    let old_base = absolute_directory(old_base);
    let new_base = absolute_directory(new_base);
    if old_base == new_base {
        return;
    }
    for instrument in &mut project.instruments {
        let reference = match &mut instrument.source {
            InstrumentSource::Synth { patch } if patch.to_ascii_lowercase().ends_with(".json") => {
                patch
            }
            InstrumentSource::DrumKit { kit, .. }
                if !matches!(kit.as_str(), "maxwell" | "chiptune") =>
            {
                kit
            }
            _ => continue,
        };
        // External library paths stay absolute; project assets retain their portability.
        if Path::new(reference).is_relative() {
            let path = old_base.join(&*reference);
            *reference = relative_path(&path, &new_base)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
        }
    }
}

fn absolute_directory(path: &Path) -> PathBuf {
    path.canonicalize()
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    let shared = path
        .iter()
        .zip(&base)
        .take_while(|(left, right)| left == right)
        .count();
    if shared == 0
        || base[shared..]
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return None;
    }
    let mut relative = PathBuf::new();
    for _ in &base[shared..] {
        relative.push("..");
    }
    for part in &path[shared..] {
        relative.push(part.as_os_str());
    }
    Some(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_save_as_remains_portable_when_the_whole_song_folder_moves() {
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("bundle");
        let old_base = bundle.join("old");
        let new_base = bundle.join("new");
        std::fs::create_dir_all(&old_base).unwrap();
        std::fs::create_dir_all(&new_base).unwrap();
        std::fs::write(
            old_base.join("patch.json"),
            evil_synth::Patch::default().to_json_pretty().unwrap(),
        )
        .unwrap();
        std::fs::write(
            old_base.join("kit.evilkit"),
            evil_beats::DrumKit::empty("portable").to_json().unwrap(),
        )
        .unwrap();
        let external = directory
            .path()
            .join("external.json")
            .to_string_lossy()
            .into_owned();
        let mut project = Project::new("Portable");
        project
            .add_instrument(evil_project::Instrument::synth("Synth", "patch.json"))
            .unwrap();
        project
            .add_instrument(evil_project::Instrument::drum_kit("Drums", "kit.evilkit"))
            .unwrap();
        project
            .add_instrument(evil_project::Instrument::synth("External", &external))
            .unwrap();
        rebase_project_sources(&mut project, &old_base, &new_base);
        assert_eq!(project.instruments[0].patch(), Some("../old/patch.json"));
        assert_eq!(project.instruments[2].patch(), Some(external.as_str()));
        project.save(new_base.join("song.evil")).unwrap();
        let relocated = directory.path().join("relocated");
        std::fs::rename(bundle, &relocated).unwrap();
        let restored = Project::load(relocated.join("new/song.evil")).unwrap();
        for instrument in &restored.instruments[..2] {
            assert!(
                Sound::from_source(&instrument.source, &relocated.join("new"))
                    .load()
                    .is_ok()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_project_directories_keep_local_references_relative() {
        let directory = tempfile::tempdir().unwrap();
        let physical = directory.path().join("physical");
        let alias = directory.path().join("alias");
        std::fs::create_dir(&physical).unwrap();
        std::os::unix::fs::symlink(&physical, &alias).unwrap();
        let mut project = Project::new("Symlink");
        project
            .add_instrument(evil_project::Instrument::synth("Synth", "patch.json"))
            .unwrap();
        rebase_project_sources(&mut project, &alias, &physical);
        assert_eq!(project.instruments[0].patch(), Some("patch.json"));
        assert_eq!(
            portable_file_reference(&physical.join("patch.json"), &alias),
            "patch.json"
        );
    }

    #[test]
    fn save_as_rebases_synth_and_kit_references_without_moving_the_files() {
        let old = tempfile::tempdir().unwrap();
        let new = tempfile::tempdir().unwrap();
        let patch_path = old.path().join("patch.json");
        let kit_path = old.path().join("kit.evilkit");
        std::fs::write(
            &patch_path,
            evil_synth::Patch::default().to_json_pretty().unwrap(),
        )
        .unwrap();
        std::fs::write(
            &kit_path,
            evil_beats::DrumKit::empty("test").to_json().unwrap(),
        )
        .unwrap();
        let mut project = Project::new("Rebased");
        project
            .add_instrument(evil_project::Instrument::synth("Synth", "patch.json"))
            .unwrap();
        project
            .add_instrument(evil_project::Instrument::drum_kit("Kit", "kit.evilkit"))
            .unwrap();
        rebase_project_sources(&mut project, old.path(), new.path());
        let output = new.path().join("saved.evil");
        project.save(&output).unwrap();
        let restored = Project::load(&output).unwrap();
        for instrument in restored.instruments {
            assert!(
                Sound::from_source(&instrument.source, new.path())
                    .load()
                    .is_ok()
            );
        }
        assert!(patch_path.exists() && kit_path.exists());
    }

    #[test]
    fn every_factory_sound_is_loadable_and_cycles_in_both_directions() {
        let browser = SoundBrowser::restore(None);
        for kind in [SoundKind::Synth, SoundKind::Kit] {
            let choices = browser.choices(&Project::new("test"), Path::new("."), kind);
            assert!(!choices.is_empty());
            for sound in &choices {
                assert!(sound.load().is_ok(), "{}", sound.reference);
            }
            assert_eq!(cycle(&choices, &choices[0], true), choices.last());
            assert_eq!(
                cycle(&choices, choices.last().unwrap(), false),
                choices.first()
            );
        }
    }

    #[test]
    fn library_deduplicates_paths_without_conflating_same_named_files() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut browser = SoundBrowser::restore(None);
        let sound = Sound::file(&first.path().join("lead.json"), SoundKind::Synth);
        browser.remember(sound.clone());
        browser.remember(sound.clone());
        browser.remember(Sound::file(
            &second.path().join("lead.json"),
            SoundKind::Synth,
        ));
        browser.collection = Collection::User;
        let mut project = Project::new("test");
        project
            .add_instrument(evil_project::Instrument::synth("Lead", "lead.json"))
            .unwrap();
        let choices = browser.choices(&project, first.path(), SoundKind::Synth);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0], sound);
        browser.query = " LEAD.JSON ".into();
        assert_eq!(
            browser
                .choices(&project, first.path(), SoundKind::Synth)
                .len(),
            2
        );
        browser.query = "missing".into();
        assert!(
            browser
                .choices(&project, first.path(), SoundKind::Synth)
                .is_empty()
        );
        assert!(cycle(&[], &sound, false).is_none());
        assert_eq!(cycle(&choices[1..], &sound, false), choices.last());
    }

    #[test]
    fn missing_and_invalid_files_fail_without_entering_the_library() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("custom.JSON");
        let sound = Sound::file(&path, SoundKind::Synth);
        assert!(sound.load().is_err());
        std::fs::write(&path, "not a patch").unwrap();
        assert!(sound.load().is_err());
        std::fs::write(
            &path,
            evil_synth::Patch::default().to_json_pretty().unwrap(),
        )
        .unwrap();
        assert!(sound.load().is_ok());
    }

    #[test]
    fn remembered_instruments_survive_a_storage_round_trip() {
        #[derive(Default)]
        struct Storage(std::collections::HashMap<String, String>);
        impl eframe::Storage for Storage {
            fn get_string(&self, key: &str) -> Option<String> {
                self.0.get(key).cloned()
            }
            fn set_string(&mut self, key: &str, value: String) {
                self.0.insert(key.into(), value);
            }
            fn flush(&mut self) {}
            fn remove_string(&mut self, key: &str) {
                self.0.remove(key);
            }
        }
        let mut browser = SoundBrowser::restore(None);
        let sound = Sound::file(Path::new("/tmp/remembered.evilkit"), SoundKind::Kit);
        browser.remember(sound.clone());
        let mut storage = Storage::default();
        browser.save(&mut storage);
        let restored = SoundBrowser::restore(Some(&storage));
        assert_eq!(restored.user_sounds, vec![sound]);
        assert!(!restored.factory_sounds.is_empty());
    }
}
