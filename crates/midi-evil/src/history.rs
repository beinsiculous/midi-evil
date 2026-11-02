use evil_project::Project;

const HISTORY_LIMIT: usize = 64;

#[derive(Default)]
pub struct EditorHistory {
    undo: Vec<Project>,
    redo: Vec<Project>,
}

impl EditorHistory {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn record(&mut self, project: Project) {
        if self.undo.last() == Some(&project) {
            return;
        }
        self.undo.push(project);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, current: &Project) -> Option<Project> {
        let previous = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(previous)
    }

    pub fn redo(&mut self, current: &Project) -> Option<Project> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_restores_snapshots_and_discards_redo_on_new_edit() {
        let mut history = EditorHistory::default();
        let original = Project::new("Original");
        history.record(original.clone());
        let edited = Project::new("Edited");
        assert_eq!(history.undo(&edited).unwrap().name, "Original");
        assert_eq!(history.redo(&original).unwrap().name, "Edited");
        history.record(original.clone());
        assert!(!history.can_redo());
    }

    #[test]
    fn pattern_edits_restore_shared_and_unique_part_snapshots() {
        use evil_project::{Part, Pattern, Track};

        let mut original = Project::new("Patterns");
        original.add_track(Track::default()).unwrap();
        let pattern = original.add_pattern(Pattern::default()).unwrap();
        original
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 1920,
                },
            )
            .unwrap();
        let mut edited = original.clone();
        edited.make_part_unique(0, 0).unwrap();
        edited.patterns[1].notes.push(evil_project::Note::default());
        let mut history = EditorHistory::default();
        history.record(original.clone());
        let restored = history.undo(&edited).unwrap();
        assert_eq!(restored.patterns.len(), 1);
        assert!(restored.patterns[0].notes.is_empty());
        assert_eq!(history.redo(&restored).unwrap(), edited);
    }
}
