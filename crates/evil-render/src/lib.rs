use std::path::{Path, PathBuf};

use evil_project::Project;
use evil_synth::{
    MonoTrack, Patch, StereoSound, SynthAftertouch, SynthController, SynthNote, SynthPitchBend,
    mix_stereo,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Project(#[from] evil_project::Error),
    #[error(transparent)]
    Synth(#[from] evil_synth::Error),
    #[error("could not read synth patch {path}: {source}")]
    PatchIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the project has no unmuted notes to render")]
    NoNotes,
}

pub struct RenderOutput {
    pub sound: StereoSound,
    pub note_count: usize,
    pub track_count: usize,
    pub unsupported_controller_events: usize,
}

pub fn load_patch(source: &str, base: &Path) -> Result<Patch, Error> {
    if source.ends_with(".json") {
        load_patch_file(&base.join(source))
    } else {
        Ok(evil_synth::preset(source)?)
    }
}

pub fn load_patch_file(path: &Path) -> Result<Patch, Error> {
    let json = std::fs::read_to_string(path).map_err(|source| Error::PatchIo {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Patch::from_json(&json)?)
}

pub fn render_audition(patch: &Patch) -> Result<RenderOutput, Error> {
    let sound = evil_synth::render_notes_dry(
        patch,
        &[SynthNote {
            start_seconds: 0.0,
            duration_seconds: f64::from(patch.duration_seconds),
            pitch: 60,
            velocity: 100,
        }],
    )?;
    let sound = mix_stereo(
        &[MonoTrack {
            sound: &sound,
            gain: 1.0,
            pan: 0.0,
            delay: patch.delay.as_ref(),
            dc_block: patch.sync.is_some(),
        }],
        1.0,
    )?;
    Ok(RenderOutput {
        sound,
        note_count: 1,
        track_count: 1,
        unsupported_controller_events: 0,
    })
}

pub fn render_project(project: &Project, patch_base: &Path) -> Result<RenderOutput, Error> {
    let performance = project.to_performance()?;
    if performance.note_count() == 0 {
        return Err(Error::NoNotes);
    }
    let mut rendered = Vec::new();
    let mut note_count = 0;
    for track in &performance.tracks {
        let settings = &project.tracks[track.track_index];
        let mut patch = load_patch(&settings.patch, patch_base)?;
        patch.sample_rate = project.sample_rate;
        patch.pitch_bend_range_semitones = settings.pitch_bend_range_semitones;
        let notes: Vec<_> = track
            .notes
            .iter()
            .map(|note| {
                let (start_seconds, duration_seconds) = performance.note_timing(note);
                SynthNote {
                    start_seconds,
                    duration_seconds,
                    pitch: note.note,
                    velocity: note.velocity,
                }
            })
            .collect();
        note_count += notes.len();
        let pitch_bends: Vec<_> = track
            .pitch_bends
            .iter()
            .map(|bend| SynthPitchBend {
                time_seconds: performance.seconds_at(bend.tick),
                value: bend.value,
            })
            .collect();
        let aftertouch: Vec<_> = track
            .aftertouch
            .iter()
            .map(|pressure| SynthAftertouch {
                time_seconds: performance.seconds_at(pressure.tick),
                note: pressure.note,
                value: pressure.value,
            })
            .collect();
        let controllers: Vec<_> = track
            .controllers
            .iter()
            .map(|event| SynthController {
                time_seconds: performance.seconds_at(event.tick),
                controller: event.controller,
                value: event.value,
            })
            .collect();
        let sound = evil_synth::render_notes_dry_with_full_automation(
            &patch,
            &notes,
            &pitch_bends,
            &aftertouch,
            &controllers,
        )?;
        rendered.push((
            sound,
            settings.gain,
            settings.pan,
            patch.delay.clone(),
            patch.sync.is_some(),
        ));
    }

    let tracks: Vec<_> = rendered
        .iter()
        .map(|(sound, gain, pan, delay, dc_block)| MonoTrack {
            sound,
            gain: *gain,
            pan: *pan,
            delay: delay.as_ref(),
            dc_block: *dc_block,
        })
        .collect();
    let track_count = tracks.len();
    let unsupported_controller_events = performance
        .tracks
        .iter()
        .flat_map(|track| &track.controllers)
        .filter(|event| !evil_project::controller_is_rendered(event.controller))
        .count();
    let sound = mix_stereo(&tracks, project.master_gain)?;
    Ok(RenderOutput {
        sound,
        note_count,
        track_count,
        unsupported_controller_events,
    })
}

#[cfg(test)]
mod tests {
    use evil_project::{Note, Track};

    use super::*;

    #[test]
    fn renders_native_tracks_with_embedded_mix_settings() {
        let mut project = Project::new("Render Test");
        project
            .add_track(Track {
                pan: -0.5,
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let output = render_project(&project, Path::new(".")).unwrap();
        assert_eq!(output.note_count, 1);
        assert_eq!(output.track_count, 1);
        assert_eq!(output.sound.sample_rate, 48_000);
        assert_eq!(output.sound.left.len(), output.sound.right.len());
        assert!(output.sound.left.iter().any(|sample| sample.abs() > 0.001));
        assert!(output.sound.left != output.sound.right);
    }

    #[test]
    fn audition_uses_current_synthesis_parameters() {
        use evil_synth::{Delay, Filter, Lfo};

        let plain = Patch::default();
        let baseline = render_audition(&plain).unwrap().sound.left;
        let variations = [
            Patch {
                gain: 0.25,
                ..plain.clone()
            },
            Patch {
                envelope: evil_synth::Envelope {
                    attack_seconds: 0.1,
                    ..plain.envelope.clone()
                },
                ..plain.clone()
            },
            Patch {
                filter: Some(Filter::default()),
                ..plain.clone()
            },
            Patch {
                lfo: Some(Lfo {
                    rate_hz: 8.0,
                    depth_cents: 100.0,
                    ..Lfo::default()
                }),
                ..plain.clone()
            },
            Patch {
                delay: Some(Delay::default()),
                ..plain.clone()
            },
        ];
        for variation in &variations {
            let audition = render_audition(variation).unwrap();
            assert_eq!(audition.note_count, 1);
            assert_ne!(audition.sound.left, baseline);
        }
    }

    #[test]
    fn audition_respects_pitch_tracking() {
        let tracked = Patch::default();
        let fixed = Patch {
            pitch_tracking: false,
            ..Patch::default()
        };
        assert_ne!(
            render_audition(&tracked).unwrap().sound.left,
            render_audition(&fixed).unwrap().sound.left
        );
    }

    #[test]
    fn project_pitch_bend_changes_rendered_audio() {
        let mut project = Project::new("Bend");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                pitch_bends: vec![evil_project::PitchBend {
                    tick: 0,
                    value: 8191,
                }],
                pitch_bend_range_semitones: 12.0,
                ..Track::default()
            })
            .unwrap();
        let bent = render_project(&project, Path::new(".")).unwrap();
        project.tracks[0].pitch_bends.clear();
        let plain = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(bent.sound.left, plain.sound.left);
    }

    #[test]
    fn project_aftertouch_changes_rendered_audio() {
        let mut project = Project::new("Pressure");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                aftertouch: vec![evil_project::Aftertouch {
                    tick: 0,
                    note: None,
                    value: 32,
                }],
                ..Track::default()
            })
            .unwrap();
        let pressed = render_project(&project, Path::new(".")).unwrap();
        project.tracks[0].aftertouch.clear();
        let plain = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(pressed.sound.left, plain.sound.left);
    }

    #[test]
    fn project_controller_events_change_rendered_audio() {
        let mut project = Project::new("Controllers");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                controllers: vec![evil_project::ControllerEvent {
                    tick: 0,
                    controller: 11,
                    value: 32,
                }],
                ..Track::default()
            })
            .unwrap();
        let controlled = render_project(&project, Path::new(".")).unwrap();
        project.tracks[0].controllers.clear();
        let plain = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(controlled.sound.left, plain.sound.left);
    }

    #[test]
    fn render_reports_only_controllers_without_synth_mappings() {
        let mut project = Project::new("Controller report");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                controllers: vec![
                    evil_project::ControllerEvent {
                        tick: 0,
                        controller: 1,
                        value: 32,
                    },
                    evil_project::ControllerEvent {
                        tick: 0,
                        controller: 2,
                        value: 32,
                    },
                    evil_project::ControllerEvent {
                        tick: 0,
                        controller: 11,
                        value: 100,
                    },
                    evil_project::ControllerEvent {
                        tick: 0,
                        controller: 101,
                        value: 0,
                    },
                ],
                ..Track::default()
            })
            .unwrap();
        let rendered = render_project(&project, Path::new(".")).unwrap();
        assert_eq!(rendered.unsupported_controller_events, 2);
    }

    #[test]
    fn empty_or_fully_muted_projects_return_a_clear_error() {
        let project = Project::new("Empty");
        assert!(matches!(
            render_project(&project, Path::new(".")),
            Err(Error::NoNotes)
        ));
    }
}
