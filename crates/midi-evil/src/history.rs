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
    fn repeated_snapshots_are_not_duplicated() {
        let mut history = EditorHistory::default();
        let project = Project::default();
        history.record(project.clone());
        history.record(project);
        assert_eq!(history.undo.len(), 1);
    }
}
