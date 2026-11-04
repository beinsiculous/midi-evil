use std::path::{Path, PathBuf};

use evil_beats::{DrumKit, KitHit};
use evil_project::{InstrumentSource, Project};
use evil_synth::{
    MonoTrack, Patch, StereoSound, StereoTrack, SynthAftertouch, SynthController, SynthNote,
    SynthPitchBend, SynthTempoChange, mix_stereo, mix_stereo_with_sends,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Project(#[from] evil_project::Error),
    #[error(transparent)]
    Synth(#[from] evil_synth::Error),
    #[error(transparent)]
    Beats(#[from] evil_beats::Error),
    #[error("could not read synth patch {path}: {source}")]
    PatchIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the project has no unmuted notes to render")]
    NoNotes,
    #[error("invalid playback range: {0}")]
    InvalidSection(String),
}

pub struct RenderOutput {
    pub sound: StereoSound,
    pub note_count: usize,
    pub track_count: usize,
    pub unsupported_controller_events: usize,
    pub skipped_drum_notes: usize,
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

pub fn load_drum_kit(source: &str, base: &Path) -> Result<DrumKit, Error> {
    if source == "maxwell" {
        Ok(evil_beats::maxwell_kit()?)
    } else {
        Ok(DrumKit::read(base.join(source))?)
    }
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
            waveshaper: patch.waveshaper.as_ref(),
            delay: patch.delay.as_ref(),
            controllers: &[],
            inserts: &[],
            send_gains: [0.0; evil_synth::SEND_COUNT],
            dc_block: patch.uses_sync(),
        }],
        1.0,
    )?;
    Ok(RenderOutput {
        sound,
        note_count: 1,
        track_count: 1,
        unsupported_controller_events: 0,
        skipped_drum_notes: 0,
    })
}

/// Render from a timeline tick, optionally stopping at a loop boundary.
/// Earlier notes and automation are rendered before cropping so sustained
/// voices, LFO phase, tempo changes, and effect tails retain their state.
pub fn render_project_range(
    project: &Project,
    patch_base: &Path,
    start_tick: u64,
    end_tick: Option<u64>,
) -> Result<RenderOutput, Error> {
    if end_tick.is_some_and(|end| end <= start_tick) {
        return Err(Error::InvalidSection(
            "end tick must be greater than start tick".to_owned(),
        ));
    }
    let performance = project.to_performance()?;
    let start_seconds = performance.seconds_at(start_tick);
    let end_seconds = end_tick.map(|end| performance.seconds_at(end));
    let mut input = project.clone();
    if let Some(end) = end_tick {
        input.truncate_timeline(end);
    }
    let mut output = render_project(&input, patch_base)?;
    let sample_rate = output.sound.sample_rate;
    let start_frame = (start_seconds * f64::from(sample_rate)).round() as usize;
    let end_frame = end_seconds.map(|seconds| (seconds * f64::from(sample_rate)).round() as usize);
    if let Some(end_frame) = end_frame
        && end_frame > output.sound.left.len()
    {
        output.sound.left.resize(end_frame, 0.0);
        output.sound.right.resize(end_frame, 0.0);
    }
    let start_frame = start_frame.min(output.sound.left.len());
    let end_frame = end_frame.unwrap_or(output.sound.left.len());
    output.sound.left = output.sound.left[start_frame..end_frame].to_vec();
    output.sound.right = output.sound.right[start_frame..end_frame].to_vec();
    output.note_count = performance
        .tracks
        .iter()
        .flat_map(|track| &track.notes)
        .filter(|note| {
            note.start_tick < end_tick.unwrap_or(u64::MAX)
                && note.start_tick.saturating_add(note.duration_ticks) > start_tick
        })
        .count();
    Ok(output)
}

pub fn render_project(project: &Project, patch_base: &Path) -> Result<RenderOutput, Error> {
    let performance = project.to_performance()?;
    if performance.note_count() == 0 {
        return Err(Error::NoNotes);
    }
    let mut rendered = Vec::new();
    let mut stereo_rendered = Vec::new();
    let mut note_count = 0;
    let mut skipped_drum_notes = 0;
    let mut unsupported_controller_events = 0;
    let tempo_changes: Vec<_> = performance
        .tempo_changes
        .iter()
        .map(|change| SynthTempoChange {
            time_seconds: performance.seconds_at(change.tick),
            bpm: 60_000_000.0 / change.microseconds_per_quarter as f32,
        })
        .collect();
    for track in &performance.tracks {
        let settings = project.instrument_for_track(track.track_index)?;
        match &settings.source {
            InstrumentSource::Synth { patch: source } => {
                let mut patch = load_patch(source, patch_base)?;
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
                unsupported_controller_events += track
                    .controllers
                    .iter()
                    .filter(|event| !evil_project::controller_is_rendered(event.controller))
                    .count();
                let sound = evil_synth::render_notes_dry_with_tempo_and_full_automation(
                    &patch,
                    &notes,
                    &pitch_bends,
                    &aftertouch,
                    &controllers,
                    &tempo_changes,
                )?;
                rendered.push((
                    sound,
                    settings.gain,
                    settings.pan,
                    patch.waveshaper.clone(),
                    patch.delay.clone(),
                    controllers,
                    settings.inserts.clone(),
                    settings.send_gains,
                    patch.uses_sync(),
                ));
            }
            InstrumentSource::DrumKit { kit, base_note } => {
                let kit = load_drum_kit(kit, patch_base)?;
                let mut hits = Vec::new();
                for note in &track.notes {
                    let Some(pad_index) = note
                        .note
                        .checked_sub(*base_note)
                        .filter(|offset| *offset < evil_project::DRUM_PAD_COUNT)
                    else {
                        skipped_drum_notes += 1;
                        continue;
                    };
                    if kit.pads[usize::from(pad_index)].is_none() {
                        skipped_drum_notes += 1;
                        continue;
                    }
                    let (start_seconds, _) = performance.note_timing(note);
                    hits.push(KitHit {
                        pad_index: usize::from(pad_index),
                        start_seconds,
                        duration_seconds: None,
                        velocity: note.velocity,
                    });
                }
                note_count += hits.len();
                unsupported_controller_events += track.controllers.len();
                let sound = evil_beats::render_kit(&kit, &hits, project.sample_rate)?;
                stereo_rendered.push((
                    sound,
                    settings.gain,
                    settings.pan,
                    settings.inserts.clone(),
                    settings.send_gains,
                ));
            }
        }
    }

    let tracks: Vec<_> = rendered
        .iter()
        .map(
            |(sound, gain, pan, waveshaper, delay, controllers, inserts, send_gains, dc_block)| {
                MonoTrack {
                    sound,
                    gain: *gain,
                    pan: *pan,
                    waveshaper: waveshaper.as_ref(),
                    delay: delay.as_ref(),
                    controllers,
                    inserts,
                    send_gains: *send_gains,
                    dc_block: *dc_block,
                }
            },
        )
        .collect();
    let stereo_tracks: Vec<_> = stereo_rendered
        .iter()
        .map(|(sound, gain, pan, inserts, send_gains)| StereoTrack {
            sound,
            gain: *gain,
            pan: *pan,
            inserts,
            send_gains: *send_gains,
        })
        .collect();
    let track_count = tracks.len() + stereo_tracks.len();
    let sound = mix_stereo_with_sends(
        &tracks,
        &stereo_tracks,
        &project.send_buses,
        project.master_gain,
    )?;
    Ok(RenderOutput {
        sound,
        note_count,
        track_count,
        unsupported_controller_events,
        skipped_drum_notes,
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
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].pan = -0.5;
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
    fn insert_waveshaper_precedes_delay_and_can_silence_the_whole_tail() {
        let delayed = Patch {
            duration_seconds: 0.01,
            envelope: evil_synth::Envelope {
                release_seconds: 0.0,
                ..evil_synth::Envelope::default()
            },
            delay: Some(evil_synth::Delay::default()),
            ..Patch::default()
        };
        let baseline = render_audition(&delayed).unwrap().sound.left;
        assert!(baseline.iter().any(|sample| sample.abs() > 0.001));

        let silenced = Patch {
            waveshaper: Some(evil_synth::Waveshaper {
                level: 0.0,
                ..evil_synth::Waveshaper::default()
            }),
            ..delayed.clone()
        };
        assert!(
            render_audition(&silenced)
                .unwrap()
                .sound
                .left
                .iter()
                .all(|sample| *sample == 0.0)
        );
        let enabled = Patch {
            waveshaper: Some(evil_synth::Waveshaper::default()),
            ..delayed
        };
        assert_ne!(render_audition(&enabled).unwrap().sound.left, baseline);
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
    fn range_crops_exact_frames_and_preserves_sustained_note_state() {
        let mut project = Project::new("Section");
        project
            .add_track(Track {
                notes: vec![
                    Note {
                        start_tick: 0,
                        duration_ticks: 960,
                        ..Note::default()
                    },
                    Note {
                        start_tick: 960,
                        duration_ticks: 480,
                        ..Note::default()
                    },
                ],
                ..Track::default()
            })
            .unwrap();
        let full = render_project(&project, Path::new(".")).unwrap();
        let section = render_project_range(&project, Path::new("."), 480, Some(960)).unwrap();
        assert_eq!(section.sound.left.len(), 24_000);
        assert_eq!(section.sound.left, full.sound.left[24_000..48_000]);
        assert_eq!(section.note_count, 1);

        let rest = render_project_range(&project, Path::new("."), 480, None).unwrap();
        assert_eq!(rest.sound.left, full.sound.left[24_000..]);
        assert_eq!(rest.note_count, 2);
        assert!(render_project_range(&project, Path::new("."), 960, Some(480)).is_err());
    }

    #[test]
    fn project_sections_resample_into_valid_embedded_oscillator_audio() {
        let mut project = Project::new("Resample");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let rendered = render_project_range(&project, Path::new("."), 0, Some(480)).unwrap();
        let sample =
            evil_synth::SampleOscillator::from_stereo("Resampled bar", &rendered.sound).unwrap();
        assert_eq!(sample.name, "Resampled bar");
        assert_eq!(sample.sample_rate, project.sample_rate);
        assert_eq!(sample.samples.len(), 24_000);
        assert!(sample.samples.iter().any(|sample| sample.abs() > 0.001));
        sample.validate().unwrap();
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
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].pitch_bend_range_semitones = 12.0;
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
    fn project_insert_automation_changes_audio_and_reports_only_unknown_controllers() {
        let directory = tempfile::tempdir().unwrap();
        let patch = Patch {
            waveshaper: Some(evil_synth::Waveshaper::default()),
            delay: Some(evil_synth::Delay::default()),
            ..Patch::default()
        };
        std::fs::write(
            directory.path().join("insert.json"),
            patch.to_json_pretty().unwrap(),
        )
        .unwrap();
        let mut project = Project::new("Insert automation");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].source = evil_project::InstrumentSource::Synth {
            patch: "insert.json".to_owned(),
        };
        let plain = render_project(&project, directory.path()).unwrap();

        project.tracks[0].controllers = (20..=25)
            .map(|controller| evil_project::ControllerEvent {
                tick: 0,
                controller,
                value: 127,
            })
            .collect();
        project.tracks[0]
            .controllers
            .push(evil_project::ControllerEvent {
                tick: 0,
                controller: 101,
                value: 0,
            });
        project.tracks[0]
            .controllers
            .extend((20..=25).map(|controller| evil_project::ControllerEvent {
                tick: 240,
                controller,
                value: 0,
            }));
        let automated = render_project(&project, directory.path()).unwrap();
        assert_ne!(automated.sound.left, plain.sound.left);
        assert_eq!(automated.unsupported_controller_events, 1);
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
    fn project_render_loads_trimmed_reverse_looped_drum_kits() {
        let directory = tempfile::tempdir().unwrap();
        let mut kit = evil_beats::DrumKit::empty("Playback modes");
        kit.gain = 1.0;
        let mut pad = evil_beats::KitPad::new(evil_beats::DrumPad::new(
            vec![0.0, 0.25, 0.5, 0.75, 1.0],
            48_000,
        ));
        pad.voice.envelope.release_seconds = 0.0;
        pad.voice.playback_start_frame = 1;
        pad.voice.playback_end_frame = Some(4);
        pad.voice.direction = evil_beats::SampleDirection::Reverse;
        pad.voice.loop_enabled = true;
        pad.gate_seconds = Some(0.01);
        pad.choke_group = 1;
        kit.pads[0] = Some(pad);
        std::fs::write(directory.path().join("kit.evilkit"), kit.to_json().unwrap()).unwrap();

        let mut project = Project::new("Drum playback modes");
        let instrument = project
            .add_instrument(evil_project::Instrument::drum_kit("Kit", "kit.evilkit"))
            .unwrap();
        project
            .add_track(Track {
                instrument,
                notes: vec![Note {
                    pitch: 36,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let reversed = render_project(&project, directory.path()).unwrap();
        assert_eq!(reversed.note_count, 1);
        assert!(reversed.sound.left.iter().any(|sample| *sample != 0.0));

        kit.pads[0].as_mut().unwrap().voice.direction = evil_beats::SampleDirection::Forward;
        std::fs::write(directory.path().join("kit.evilkit"), kit.to_json().unwrap()).unwrap();
        let forward = render_project(&project, directory.path()).unwrap();
        assert_ne!(reversed.sound.left, forward.sound.left);
    }

    #[test]
    fn project_channel_inserts_and_send_buses_change_rendered_audio() {
        let mut project = Project::new("Channel graph");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let dry = render_project(&project, Path::new(".")).unwrap();
        project.instruments[0].inserts =
            vec![evil_synth::InsertEffect::Bitcrush(evil_synth::Bitcrush {
                bits: 2,
                downsample: 1,
                mix: 0.5,
            })];
        let inserted = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(dry.sound.left, inserted.sound.left);

        project.instruments[0].send_gains = [0.5, 0.0];
        project.send_buses[0].gain = 0.5;
        let project = Project::from_json(&project.to_json_pretty().unwrap()).unwrap();
        let sent = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(
            inserted.sound.left,
            sent.sound.left,
            "send gain {} / sends {:?} / energy {} vs {}",
            project.send_buses[0].gain,
            project.instruments[0].send_gains,
            inserted
                .sound
                .left
                .iter()
                .map(|sample| sample.abs())
                .sum::<f32>(),
            sent.sound
                .left
                .iter()
                .map(|sample| sample.abs())
                .sum::<f32>()
        );
        assert_eq!(sent.note_count, 1);
    }

    #[test]
    fn project_render_loads_filtered_trg_bus_sends_and_tails() {
        let directory = tempfile::tempdir().unwrap();
        let mut kit = evil_beats::DrumKit::empty("Filtered bus");
        kit.gain = 1.0;
        let mut pad = evil_beats::KitPad::new(evil_beats::DrumPad::new(vec![0.3], 48_000));
        pad.voice.gain = 1.0;
        pad.voice.envelope.release_seconds = 0.0;
        pad.pan = -1.0;
        pad.bus_sends[0] = 1.0;
        kit.pads[0] = Some(pad);
        kit.buses[0].filter = Some(evil_beats::DrumFilter {
            cutoff_hz: 800.0,
            resonance: 0.2,
            ..evil_beats::DrumFilter::default()
        });
        kit.buses[0].pan = 1.0;
        std::fs::write(directory.path().join("kit.evilkit"), kit.to_json().unwrap()).unwrap();

        let mut project = Project::new("Filtered TRG bus");
        let instrument = project
            .add_instrument(evil_project::Instrument::drum_kit("Kit", "kit.evilkit"))
            .unwrap();
        project
            .add_track(Track {
                instrument,
                notes: vec![Note {
                    pitch: 36,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let unprocessed = render_project(&project, directory.path()).unwrap();
        project.instruments[0].inserts =
            vec![evil_synth::InsertEffect::Bitcrush(evil_synth::Bitcrush {
                bits: 2,
                downsample: 1,
                mix: 0.5,
            })];
        let filtered = render_project(&project, directory.path()).unwrap();
        assert_ne!(unprocessed.sound.left, filtered.sound.left);
        assert!(filtered.sound.left.len() > 1);
        assert!(
            filtered.sound.right.iter().any(|sample| *sample != 0.0),
            "unprocessed right {:?} / filtered right {:?}",
            &unprocessed.sound.right[..unprocessed.sound.right.len().min(8)],
            &filtered.sound.right[..filtered.sound.right.len().min(8)]
        );

        kit.buses[0].muted = true;
        std::fs::write(directory.path().join("kit.evilkit"), kit.to_json().unwrap()).unwrap();
        let muted = render_project(&project, directory.path()).unwrap();
        assert_eq!(muted.sound.left.len(), 1);
        assert!(muted.sound.right.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn renders_drum_kit_tracks_and_reports_unmapped_notes() {
        let mut project = Project::new("Drums");
        let instrument = project
            .add_instrument(evil_project::Instrument::drum_kit("Kit", "maxwell"))
            .unwrap();
        project
            .add_track(Track {
                instrument,
                notes: vec![
                    Note {
                        pitch: 36,
                        ..Note::default()
                    },
                    Note {
                        pitch: 60,
                        ..Note::default()
                    },
                ],
                controllers: vec![evil_project::ControllerEvent {
                    tick: 0,
                    controller: 1,
                    value: 100,
                }],
                ..Track::default()
            })
            .unwrap();
        let output = render_project(&project, Path::new(".")).unwrap();
        assert_eq!(output.note_count, 1);
        assert_eq!(output.skipped_drum_notes, 1);
        assert_eq!(output.unsupported_controller_events, 1);
        assert_eq!(output.track_count, 1);
        assert!(output.sound.left.iter().any(|sample| sample.abs() > 0.001));
        assert_eq!(output.sound.left.len(), output.sound.right.len());
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
