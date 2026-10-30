use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use evil_project::Project;
use evil_render::RenderOutput;

pub struct RenderJob {
    receiver: Receiver<Result<RenderOutput, String>>,
}

impl RenderJob {
    pub fn start(project: Project, patch_base: PathBuf) -> Self {
        Self::spawn(move || evil_render::render_project(&project, &patch_base))
    }

    pub fn start_range(
        project: Project,
        patch_base: PathBuf,
        start_tick: u64,
        end_tick: Option<u64>,
    ) -> Self {
        Self::spawn(move || {
            evil_render::render_project_range(&project, &patch_base, start_tick, end_tick)
        })
    }

    pub fn start_audition(patch: evil_synth::Patch) -> Self {
        Self::spawn(move || evil_render::render_audition(&patch))
    }

    fn spawn(
        render: impl FnOnce() -> Result<RenderOutput, evil_render::Error> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = render().map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        Self { receiver }
    }

    pub fn try_finish(&self) -> Option<Result<RenderOutput, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("render worker disconnected".to_owned())),
        }
    }

    #[cfg(test)]
    fn wait(self) -> Result<RenderOutput, String> {
        self.receiver.recv().map_err(|error| error.to_string())?
    }
}

#[cfg(test)]
mod tests {
    use evil_project::{Note, Track};

    use super::*;

    #[test]
    fn worker_renders_a_project_without_blocking_the_caller() {
        let mut project = Project::new("Background");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let job = RenderJob::start(project, PathBuf::from("."));
        assert_eq!(job.wait().unwrap().note_count, 1);
    }

    #[test]
    fn worker_audits_an_unsaved_patch() {
        let patch = evil_synth::Patch {
            name: "Unsaved audition".to_owned(),
            ..evil_synth::Patch::default()
        };
        let job = RenderJob::start_audition(patch);
        let output = job.wait().unwrap();
        assert_eq!(output.note_count, 1);
        assert!(output.sound.left.iter().any(|sample| sample.abs() > 0.001));
    }
}
