use evil_project::Project;

const HISTORY_LIMIT: usize = 64;

/// A whole project as it was, with the editor's selection that belonged to it, so undo puts the
/// cursor back where it was instead of guessing it from the notes.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot<Selection> {
    pub project: Project,
    pub selection: Selection,
}

/// One hold of one key: a fresh press of one of the roll's repeating commands and that
/// command's repeats. Its edits are one undo entry, as one keypress's would be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gesture(u64);

impl Gesture {
    pub fn new(number: u64) -> Self {
        Self(number)
    }
}

pub struct EditorHistory<Selection = ()> {
    undo: Vec<Snapshot<Selection>>,
    redo: Vec<Snapshot<Selection>>,
    /// The gesture whose state-before is the entry on top, while that entry may still absorb
    /// the gesture's later edits.
    open_gesture: Option<Gesture>,
}

impl<Selection> Default for EditorHistory<Selection> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            open_gesture: None,
        }
    }
}

impl<Selection: Clone> EditorHistory<Selection> {
    pub fn rebase_instruments(&mut self, old_base: &std::path::Path, new_base: &std::path::Path) {
        self.open_gesture = None;
        for snapshot in self.undo.iter_mut().chain(&mut self.redo) {
            crate::instruments::rebase_project_sources(&mut snapshot.project, old_base, new_base);
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open_gesture = None;
    }

    /// The gesture's key came up, or anything else ended it: its entry is closed.
    pub fn end_gesture(&mut self) {
        self.open_gesture = None;
    }

    /// Records the state before an edit. An edit of the same gesture as the entry on top is not
    /// recorded again: that entry already holds the state from before the gesture began.
    pub fn record(&mut self, project: Project, selection: Selection, gesture: Option<Gesture>) {
        if gesture.is_some() && gesture == self.open_gesture {
            return;
        }
        self.open_gesture = gesture;
        if self
            .undo
            .last()
            .is_some_and(|snapshot| snapshot.project == project)
        {
            return;
        }
        self.undo.push(Snapshot { project, selection });
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

    pub fn undo_target(&self) -> Option<&Project> {
        self.undo.last().map(|snapshot| &snapshot.project)
    }

    pub fn redo_target(&self) -> Option<&Project> {
        self.redo.last().map(|snapshot| &snapshot.project)
    }

    /// The state being left is kept with its own selection, so redo returns to it.
    pub fn undo(&mut self, current: &Project, selection: Selection) -> Option<Snapshot<Selection>> {
        self.open_gesture = None;
        let previous = self.undo.pop()?;
        self.redo.push(Snapshot {
            project: current.clone(),
            selection,
        });
        Some(previous)
    }

    pub fn redo(&mut self, current: &Project, selection: Selection) -> Option<Snapshot<Selection>> {
        self.open_gesture = None;
        let next = self.redo.pop()?;
        self.undo.push(Snapshot {
            project: current.clone(),
            selection,
        });
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
        history.record(original.clone(), (), None);
        let edited = Project::new("Edited");
        assert_eq!(history.undo(&edited, ()).unwrap().project.name, "Original");
        assert_eq!(history.redo(&original, ()).unwrap().project.name, "Edited");
        history.record(original.clone(), (), None);
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
        history.record(original.clone(), (), None);
        let restored = history.undo(&edited, ()).unwrap().project;
        assert_eq!(restored.patterns.len(), 1);
        assert!(restored.patterns[0].notes.is_empty());
        assert_eq!(history.redo(&restored, ()).unwrap().project, edited);
    }

    #[test]
    fn each_snapshot_carries_the_selection_that_belonged_to_it() {
        let mut history = EditorHistory::default();
        let before = Project::new("Before");
        let after = Project::new("After");
        history.record(before.clone(), "cursor on the first note", None);
        let undone = history.undo(&after, "cursor on the added note").unwrap();
        assert_eq!(undone.selection, "cursor on the first note");
        let redone = history
            .redo(&undone.project, "cursor on the first note")
            .unwrap();
        assert_eq!(redone.selection, "cursor on the added note");
    }

    #[test]
    fn a_gestures_edits_are_one_entry_until_anything_else_touches_the_history() {
        let hold = Some(Gesture::new(1));
        let mut history = EditorHistory::default();
        let states: Vec<Project> = (0..4).map(|step| Project::new(format!("{step}"))).collect();
        history.record(states[0].clone(), (), hold);
        history.record(states[1].clone(), (), hold);
        history.record(states[2].clone(), (), hold);
        assert_eq!(history.undo(&states[3], ()).unwrap().project.name, "0");
        assert!(!history.can_undo(), "one entry for the whole hold");
        history.redo(&states[0], ());
        history.record(states[3].clone(), (), hold);
        assert_eq!(
            history.undo(&states[0], ()).unwrap().project.name,
            "3",
            "undo closed it"
        );
        history.record(states[1].clone(), (), hold);
        history.end_gesture();
        history.record(states[2].clone(), (), hold);
        history.record(states[3].clone(), (), None);
        history.record(states[0].clone(), (), hold);
        let mut names = Vec::new();
        while let Some(snapshot) = history.undo(&states[0], ()) {
            names.push(snapshot.project.name);
        }
        assert_eq!(names, ["0", "3", "2", "1", "0"]);
    }
}
