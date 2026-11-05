use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use evil_project::Project;
use evil_render::{ExportPlan, ExportSpec, RenderOutput, RenderScope};

pub struct RenderJob {
    receiver: Receiver<Result<RenderOutput, String>>,
}

impl RenderJob {
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

    pub fn start_wav(project: Project, patch_base: PathBuf, output: PathBuf) -> Self {
        Self::start_scoped_wav(project, patch_base, RenderScope::FullMix, output)
    }

    pub fn start_range_wav(
        project: Project,
        patch_base: PathBuf,
        start_tick: u64,
        end_tick: Option<u64>,
        output: PathBuf,
    ) -> Self {
        Self::spawn(move || {
            let rendered =
                evil_render::render_project_range(&project, &patch_base, start_tick, end_tick)?;
            rendered.sound.write_wav(output)?;
            Ok(rendered)
        })
    }

    pub fn start_scoped_wav(
        project: Project,
        patch_base: PathBuf,
        scope: RenderScope,
        output: PathBuf,
    ) -> Self {
        Self::spawn(move || {
            let rendered = evil_render::render_project_scoped(&project, &patch_base, scope)?;
            // Finalize this file on the worker before the UI is notified or another export starts.
            rendered.sound.write_wav(output)?;
            Ok(rendered)
        })
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

/// Remaining scoped exports. Only the active RenderJob owns audio; the queue owns just specs.
pub struct WavExportQueue {
    pub output_directory: PathBuf,
    remaining: VecDeque<ExportSpec>,
    pub completed: usize,
    pub total: usize,
    pub skipped: usize,
}

impl WavExportQueue {
    pub fn new(output_directory: PathBuf, plan: ExportPlan) -> Self {
        Self {
            output_directory,
            total: plan.exports.len(),
            remaining: plan.exports.into(),
            completed: 0,
            skipped: plan.skipped,
        }
    }

    pub fn next_spec(&mut self) -> Option<ExportSpec> {
        self.remaining.pop_front()
    }

    pub fn complete_current(&mut self) {
        self.completed += 1;
    }

    pub fn output_for(&self, spec: &ExportSpec) -> PathBuf {
        self.output_directory.join(&spec.file_name)
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use evil_project::{Note, Track};

    use super::*;

    fn project() -> Project {
        let mut project = Project::new("Background");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project
            .add_track(Track {
                name: "Muted harmony".to_owned(),
                notes: vec![Note {
                    pitch: 67,
                    ..Note::default()
                }],
                muted: true,
                ..Track::default()
            })
            .unwrap();
        project
    }

    #[test]
    fn worker_renders_a_project_without_blocking_the_caller() {
        let job = RenderJob::start_range(project(), PathBuf::from("."), 0, None);
        assert_eq!(job.wait().unwrap().note_count, 1);
    }

    #[test]
    fn scoped_wav_worker_unmutes_only_the_selection_and_writes_pcm16_stereo() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("02-muted-harmony.wav");
        let job = RenderJob::start_scoped_wav(
            project(),
            PathBuf::from("."),
            RenderScope::Track { track_index: 1 },
            output.clone(),
        );
        let rendered = job.wait().unwrap();
        assert_eq!(rendered.note_count, 1);
        assert_eq!(rendered.track_count, 1);
        assert_eq!(rendered.channel_meters[0].0, 1);
        let wav = std::fs::read(&output).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        let data = wav.windows(4).position(|chunk| chunk == b"data").unwrap();
        let data_bytes = u32::from_le_bytes(wav[data + 4..data + 8].try_into().unwrap());
        assert_eq!(data_bytes as usize, rendered.sound.left.len() * 4);
    }

    #[test]
    fn native_parity_full_and_section_wav_exports_match_the_full_mix_crop() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project();
        project.tracks[0].notes[0].duration_ticks = 1_920;
        let full_path = directory.path().join("full.wav");
        let section_path = directory.path().join("section.wav");
        let full = RenderJob::start_wav(project.clone(), PathBuf::from("."), full_path.clone())
            .wait()
            .unwrap();
        let section = RenderJob::start_range_wav(
            project,
            PathBuf::from("."),
            480,
            Some(960),
            section_path.clone(),
        )
        .wait()
        .unwrap();
        assert_eq!(section.sound.left.len(), 24_000);
        assert_eq!(section.sound.right.len(), 24_000);
        assert_eq!(section.sound.left, full.sound.left[24_000..48_000]);
        assert_eq!(section.sound.right, full.sound.right[24_000..48_000]);

        for (path, expected) in [(&full_path, full.sound.left.len()), (&section_path, 24_000)] {
            let mut reader = hound::WavReader::open(path).unwrap();
            assert_eq!(reader.spec().channels, 2);
            assert_eq!(reader.spec().sample_rate, 48_000);
            assert_eq!(reader.spec().bits_per_sample, 16);
            assert_eq!(reader.samples::<i16>().count(), expected * 2);
        }
        let section_bytes = std::fs::read(&section_path).unwrap();
        RenderJob::start_range_wav(
            {
                let mut rerun = self::project();
                rerun.tracks[0].notes[0].duration_ticks = 1_920;
                rerun
            },
            PathBuf::from("."),
            480,
            Some(960),
            section_path.clone(),
        )
        .wait()
        .unwrap();
        assert_eq!(std::fs::read(&section_path).unwrap(), section_bytes);
    }

    #[test]
    fn export_queue_holds_only_specs_and_advances_sequentially() {
        let specs = vec![
            ExportSpec {
                scope: RenderScope::Track { track_index: 0 },
                file_name: "01-lead.wav".to_owned(),
            },
            ExportSpec {
                scope: RenderScope::TrackPart {
                    track_index: 0,
                    part_index: 2,
                },
                file_name: "01-lead__part-0003-verse.wav".to_owned(),
            },
        ];
        let mut queue = WavExportQueue::new(
            PathBuf::from("exports"),
            ExportPlan {
                exports: specs,
                skipped: 1,
            },
        );
        assert!(!queue.is_empty());
        assert_eq!(queue.total, 2);
        assert_eq!(queue.skipped, 1);
        let first = queue.next_spec().unwrap();
        assert_eq!(queue.completed, 0);
        assert_eq!(
            queue.output_for(&first),
            Path::new("exports").join("01-lead.wav")
        );
        queue.complete_current();
        let second = queue.next_spec().unwrap();
        assert_eq!(queue.completed, 1);
        assert!(matches!(
            second.scope,
            RenderScope::TrackPart { part_index: 2, .. }
        ));
        queue.complete_current();
        assert!(queue.next_spec().is_none());
        assert_eq!(queue.completed, 2);
    }
}
