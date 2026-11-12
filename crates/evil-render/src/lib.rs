use std::path::{Path, PathBuf};

use evil_beats::{DrumKit, KitHit};
use evil_project::{InstrumentSource, Project};
use evil_synth::{
    MonoTrack, Patch, StereoSound, StereoTrack, SynthAftertouch, SynthController, SynthNote,
    SynthPitchBend, SynthTempoChange, mix_stereo_analyzed,
    mix_stereo_with_sends_and_tempo_analyzed_range,
};
use thiserror::Error;

mod export;

pub use export::{
    ExportPlan, ExportSpec, part_export_plan, part_file_name, stem_export_plan, stem_file_name,
};

/// Selects the original project material included in a render.
/// Track and timeline-part indices are zero-based positions in the original project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderScope {
    FullMix,
    Track {
        track_index: usize,
    },
    TrackPart {
        track_index: usize,
        part_index: usize,
    },
}

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
    #[error("invalid render selection: {0}")]
    InvalidSelection(String),
    #[error("render selection is empty: {0}")]
    EmptySelection(String),
}

pub struct RenderOutput {
    pub sound: StereoSound,
    pub mix_analysis: evil_synth::MixAnalysis,
    pub channel_meters: Vec<(usize, evil_synth::ChannelMeter)>,
    pub note_count: usize,
    pub track_count: usize,
    pub unsupported_controller_events: usize,
    pub unsupported_mixer_automation_events: usize,
    pub skipped_drum_notes: usize,
}

pub fn load_patch(source: &str, base: &Path) -> Result<Patch, Error> {
    if source.to_ascii_lowercase().ends_with(".json") {
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
    match source {
        "maxwell" => Ok(evil_beats::maxwell_kit()?),
        "chiptune" => Ok(evil_beats::chiptune_kit()?),
        _ => Ok(DrumKit::read(base.join(source))?),
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
    let mixed = mix_stereo_analyzed(
        &[MonoTrack {
            sound: &sound,
            gain: 1.0,
            pan: 0.0,
            waveshaper: patch.waveshaper.as_ref(),
            delay: patch.delay.as_ref(),
            controllers: &[],
            automation: &[],
            inserts: &[],
            send_gains: [0.0; evil_synth::SEND_COUNT],
            dc_block: patch.uses_sync(),
        }],
        1.0,
    )?;
    Ok(RenderOutput {
        sound: mixed.sound,
        mix_analysis: mixed.analysis,
        channel_meters: vec![(
            0,
            mixed
                .channel_meters
                .into_iter()
                .next()
                .expect("one audition channel"),
        )],
        note_count: 1,
        track_count: 1,
        unsupported_controller_events: 0,
        unsupported_mixer_automation_events: 0,
        skipped_drum_notes: 0,
    })
}

/// Render a full mix, one temporarily-unmuted original track, or one original timeline part.
/// Scoped renders are solo exports: other tracks are excluded, sends contain only the selected
/// track's contribution, and the input project is never modified. Part renders start at frame zero
/// at the part start and play through the complete note, release, insert, and send tail.
/// Master processing is applied independently to every stem, so summing separately saturated stems
/// is not guaranteed to reproduce the full mix.
pub fn render_project_scoped(
    project: &Project,
    patch_base: &Path,
    scope: RenderScope,
) -> Result<RenderOutput, Error> {
    if let RenderScope::TrackPart {
        track_index,
        part_index,
    } = scope
    {
        let part = project
            .tracks
            .get(track_index)
            .and_then(|track| track.parts.get(part_index))
            .copied()
            .ok_or_else(|| {
                Error::InvalidSelection(format!(
                    "arrangement part index {part_index} does not exist on track index {track_index}"
                ))
            })?;
        return render_project_scoped_range(project, patch_base, scope, part.start_tick, None);
    }
    let input = scoped_project(project, scope)?;
    render_project_with_analysis_range(&input, patch_base, None)
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
    render_project_scoped_range(
        project,
        patch_base,
        RenderScope::FullMix,
        start_tick,
        end_tick,
    )
}

/// Exact-range variant of scoped rendering. Unlike a complete part render, an explicit range can
/// crop a part to arbitrary boundaries; track stems retain the full-mix range semantics.
pub fn render_project_scoped_range(
    project: &Project,
    patch_base: &Path,
    scope: RenderScope,
    start_tick: u64,
    end_tick: Option<u64>,
) -> Result<RenderOutput, Error> {
    if end_tick.is_some_and(|end| end <= start_tick) {
        return Err(Error::InvalidSection(
            "end tick must be greater than start tick".to_owned(),
        ));
    }
    let mut input = scoped_project(project, scope)?;
    let performance = input.to_performance()?;
    let start_seconds = performance.seconds_at(start_tick);
    let end_seconds = end_tick.map(|end| performance.seconds_at(end));
    let sample_rate = project.sample_rate;
    let start_frame = (start_seconds * f64::from(sample_rate)).round() as usize;
    let end_frame = end_seconds.map(|seconds| (seconds * f64::from(sample_rate)).round() as usize);
    if let Some(end) = end_tick {
        input.truncate_timeline(end);
    }
    let mut output = render_project_with_analysis_range(
        &input,
        patch_base,
        Some((start_frame, end_frame.unwrap_or(usize::MAX))),
    )?;
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
    render_project_scoped(project, patch_base, RenderScope::FullMix)
}

/// Clone and isolate a selection while retaining original track/instrument indices and mix routing.
fn scoped_project(project: &Project, scope: RenderScope) -> Result<Project, Error> {
    project.validate()?;
    let selected_track = match scope {
        RenderScope::FullMix => return Ok(project.clone()),
        RenderScope::Track { track_index } | RenderScope::TrackPart { track_index, .. } => {
            track_index
        }
    };
    export::validate_track(project, selected_track)?;
    let selected_part = match scope {
        RenderScope::TrackPart { part_index, .. } => Some(
            project.tracks[selected_track]
                .parts
                .get(part_index)
                .copied()
                .ok_or_else(|| {
                    Error::InvalidSelection(format!(
                        "arrangement part index {part_index} does not exist on track index {selected_track}"
                    ))
                })?,
        ),
        _ => None,
    };
    if !export::scope_has_notes(project, scope) {
        let selection = match scope {
            RenderScope::Track { track_index } => {
                format!("track index {track_index} has no notes to render")
            }
            RenderScope::TrackPart {
                track_index,
                part_index,
            } => format!(
                "arrangement part index {part_index} on track index {track_index} has no notes to render"
            ),
            RenderScope::FullMix => unreachable!("full mix returned above"),
        };
        return Err(Error::EmptySelection(selection));
    }

    let mut input = project.clone();
    for (index, track) in input.tracks.iter_mut().enumerate() {
        track.muted = index != selected_track;
    }
    if let Some(part) = selected_part {
        let track = &mut input.tracks[selected_track];
        track.notes.clear();
        track.pitch_bends.clear();
        track.aftertouch.clear();
        track.controllers.clear();
        track.mixer_automation.clear();
        track.parts = vec![part];
    }
    Ok(input)
}

fn render_project_with_analysis_range(
    project: &Project,
    patch_base: &Path,
    analysis_range: Option<(usize, usize)>,
) -> Result<RenderOutput, Error> {
    let performance = project.to_performance()?;
    if performance.note_count() == 0 {
        return Err(Error::NoNotes);
    }
    let mut rendered = Vec::new();
    let mut stereo_rendered = Vec::new();
    let mut rendered_track_indices = Vec::new();
    let mut stereo_track_indices = Vec::new();
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
                let mixer_automation: Vec<_> = track
                    .mixer_automation
                    .iter()
                    .map(|event| evil_synth::SynthMixerAutomation {
                        time_seconds: performance.seconds_at(event.tick),
                        parameter: event.parameter,
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
                rendered_track_indices.push(track.track_index);
                rendered.push((
                    sound,
                    settings.gain,
                    settings.pan,
                    patch.waveshaper.clone(),
                    patch.delay.clone(),
                    controllers,
                    mixer_automation,
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
                let mixer_automation: Vec<_> = track
                    .mixer_automation
                    .iter()
                    .map(|event| evil_synth::SynthMixerAutomation {
                        time_seconds: performance.seconds_at(event.tick),
                        parameter: event.parameter,
                        value: event.value,
                    })
                    .collect();
                stereo_track_indices.push(track.track_index);
                stereo_rendered.push((
                    sound,
                    settings.gain,
                    settings.pan,
                    mixer_automation,
                    settings.inserts.clone(),
                    settings.send_gains,
                ));
            }
        }
    }

    let tracks: Vec<_> = rendered
        .iter()
        .map(
            |(
                sound,
                gain,
                pan,
                waveshaper,
                delay,
                controllers,
                automation,
                inserts,
                send_gains,
                dc_block,
            )| {
                MonoTrack {
                    sound,
                    gain: *gain,
                    pan: *pan,
                    waveshaper: waveshaper.as_ref(),
                    delay: delay.as_ref(),
                    controllers,
                    automation,
                    inserts,
                    send_gains: *send_gains,
                    dc_block: *dc_block,
                }
            },
        )
        .collect();
    let stereo_tracks: Vec<_> = stereo_rendered
        .iter()
        .map(
            |(sound, gain, pan, automation, inserts, send_gains)| StereoTrack {
                sound,
                gain: *gain,
                pan: *pan,
                automation,
                inserts,
                send_gains: *send_gains,
            },
        )
        .collect();
    let track_count = tracks.len() + stereo_tracks.len();
    let mixed = mix_stereo_with_sends_and_tempo_analyzed_range(
        &tracks,
        &stereo_tracks,
        &project.send_buses,
        project.master_gain,
        &tempo_changes,
        analysis_range,
    )?;
    let channel_meters = rendered_track_indices
        .into_iter()
        .chain(stereo_track_indices)
        .zip(mixed.channel_meters)
        .collect();
    Ok(RenderOutput {
        sound: mixed.sound,
        mix_analysis: mixed.analysis,
        channel_meters,
        note_count,
        track_count,
        unsupported_controller_events,
        unsupported_mixer_automation_events: mixed.unsupported_mixer_automation_events,
        skipped_drum_notes,
    })
}

#[cfg(test)]
mod tests {
    use evil_project::{Note, Track};

    use super::*;

    fn assert_stem_matches_manually_soloed_clone(project: &Project, track_index: usize) {
        let original = project.clone();
        let mut solo = project.clone();
        for (index, track) in solo.tracks.iter_mut().enumerate() {
            track.muted = index != track_index;
        }
        let expected = render_project(&solo, Path::new(".")).unwrap();
        let stem =
            render_project_scoped(project, Path::new("."), RenderScope::Track { track_index })
                .unwrap();
        assert_eq!(stem.sound.left, expected.sound.left);
        assert_eq!(stem.sound.right, expected.sound.right);
        assert_eq!(stem.mix_analysis, expected.mix_analysis);
        assert_eq!(stem.note_count, expected.note_count);
        assert_eq!(stem.track_count, 1);
        assert_eq!(stem.channel_meters[0].0, track_index);
        assert_eq!(
            stem.unsupported_controller_events,
            expected.unsupported_controller_events
        );
        assert_eq!(
            stem.unsupported_mixer_automation_events,
            expected.unsupported_mixer_automation_events
        );
        assert_eq!(stem.skipped_drum_notes, expected.skipped_drum_notes);
        assert_eq!(
            project, &original,
            "scoped rendering must not mutate its input"
        );

        let expected_range = render_project_range(&solo, Path::new("."), 0, Some(960)).unwrap();
        let stem_range = render_project_scoped_range(
            project,
            Path::new("."),
            RenderScope::Track { track_index },
            0,
            Some(960),
        )
        .unwrap();
        assert_eq!(stem_range.sound.left, expected_range.sound.left);
        assert_eq!(stem_range.sound.right, expected_range.sound.right);
    }

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
        assert!(
            section.mix_analysis.pre_saturator_peak[0] <= full.mix_analysis.pre_saturator_peak[0]
        );
        assert!(section.mix_analysis.clipped_frames <= full.mix_analysis.clipped_frames);
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
    fn native_typed_mixer_automation_renders_and_reports_stale_insert_targets() {
        use evil_project::{EffectParameter, MixerAutomationEvent, MixerParameter};

        let mut project = Project::new("Typed mixer automation");
        project
            .add_track(Track {
                notes: vec![Note {
                    duration_ticks: 1_920,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].inserts = vec![evil_synth::InsertEffect::Reverb(
            evil_synth::Reverb::default(),
        )];
        project.tracks[0].mixer_automation = vec![
            MixerAutomationEvent {
                tick: 0,
                parameter: MixerParameter::Gain,
                value: 1.0,
            },
            MixerAutomationEvent {
                tick: 0,
                parameter: MixerParameter::Insert {
                    slot: 0,
                    parameter: EffectParameter::ReverbMix,
                },
                value: 0.0,
            },
            MixerAutomationEvent {
                tick: 960,
                parameter: MixerParameter::Gain,
                value: 0.25,
            },
            MixerAutomationEvent {
                tick: 960,
                parameter: MixerParameter::Insert {
                    slot: 0,
                    parameter: EffectParameter::ReverbMix,
                },
                value: 0.75,
            },
        ];
        let plain = render_project(&project, Path::new(".")).unwrap();
        project.tracks[0].mixer_automation.clear();
        let static_mix = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(plain.sound.left, static_mix.sound.left);
        assert_eq!(plain.unsupported_mixer_automation_events, 0);

        project.tracks[0]
            .mixer_automation
            .push(MixerAutomationEvent {
                tick: 0,
                parameter: MixerParameter::Insert {
                    slot: 1,
                    parameter: EffectParameter::DelayMix,
                },
                value: 1.0,
            });
        let unsupported = render_project(&project, Path::new(".")).unwrap();
        assert_eq!(unsupported.unsupported_mixer_automation_events, 1);
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
    fn project_beat_synced_delay_follows_the_tempo_map() {
        let mut project = Project::new("Beat delay");
        project.sample_rate = 8_000;
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].inserts = vec![evil_synth::InsertEffect::Delay(evil_synth::Delay {
            beats_per_cycle: Some(1.0),
            feedback: 0.0,
            mix: 1.0,
            mode: evil_synth::DelayMode::Mono,
            ..evil_synth::Delay::default()
        })];
        let fast = render_project(&project, Path::new(".")).unwrap();
        project.tempo_bpm = 60.0;
        let slow = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(fast.sound.left, slow.sound.left);
        assert!(slow.sound.left.len() > fast.sound.left.len());

        project.tempo_changes.push(evil_project::TempoChange {
            tick: 480,
            bpm: 240.0,
        });
        let mapped = render_project(&project, Path::new(".")).unwrap();
        assert_ne!(mapped.sound.left, slow.sound.left);
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
        let mut project = Project::from_json(&project.to_json_pretty().unwrap()).unwrap();
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

        project.instruments[0].send_gains = [0.0; evil_synth::SEND_COUNT];
        project.instruments[0].inserts = vec![evil_synth::InsertEffect::Reverb(
            evil_synth::Reverb::default(),
        )];
        let reverbed = render_project(&project, Path::new(".")).unwrap();
        assert!(reverbed.sound.left.len() > sent.sound.left.len());
        assert!(reverbed.sound.left.iter().all(|sample| sample.is_finite()));
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
    fn native_parity_chiptune_builtin_renders_a_dense_sixteen_pad_pattern() {
        let kit = load_drum_kit("chiptune", Path::new(".")).unwrap();
        assert_eq!(kit.name, "MIDI Evil / Chiptune");
        assert!(kit.pads.iter().all(Option::is_some));
        assert_eq!(kit.pads[2].as_ref().unwrap().choke_group, 1);
        assert_eq!(kit.pads[3].as_ref().unwrap().choke_group, 1);

        let mut project = Project::new("Chiptune audit");
        let instrument = project
            .add_instrument(evil_project::Instrument::drum_kit("Chiptune", "chiptune"))
            .unwrap();
        project
            .add_track(Track {
                instrument,
                notes: (0..64)
                    .map(|index| Note {
                        pitch: 36 + (index % 16) as u8,
                        start_tick: index * 30,
                        duration_ticks: 30,
                        velocity: 40 + ((index * 3) % 88) as u8,
                    })
                    .collect(),
                ..Track::default()
            })
            .unwrap();
        let first = render_project(&project, Path::new(".")).unwrap();
        let second = render_project(&project, Path::new(".")).unwrap();
        assert_eq!(first.note_count, 64);
        assert_eq!(first.skipped_drum_notes, 0);
        assert_eq!(first.sound.left, second.sound.left);
        assert_eq!(first.sound.right, second.sound.right);
        assert!(first.sound.left.iter().any(|sample| sample.abs() > 0.001));
        assert!(
            first
                .sound
                .left
                .iter()
                .chain(&first.sound.right)
                .all(|sample| sample.is_finite())
        );
    }

    #[test]
    fn stems_match_soloed_clones_for_shared_instruments_and_originally_muted_tracks() {
        let mut project = Project::new("Solo stems");
        let shared = project
            .add_instrument(evil_project::Instrument::synth("Shared", "lead"))
            .unwrap();
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                instrument: shared,
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project
            .add_track(Track {
                name: "Muted harmony".to_owned(),
                instrument: shared,
                muted: true,
                notes: vec![Note {
                    pitch: 67,
                    start_tick: 480,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        assert_stem_matches_manually_soloed_clone(&project, 0);
        assert_stem_matches_manually_soloed_clone(&project, 1);
    }

    #[test]
    fn synth_and_drum_stems_keep_their_original_routing_and_warning_counts() {
        let mut project = Project::new("Synth and drums");
        project
            .add_track(Track {
                name: "Synth".to_owned(),
                notes: vec![Note::default()],
                controllers: vec![evil_project::ControllerEvent {
                    tick: 0,
                    controller: 2,
                    value: 1,
                }],
                ..Track::default()
            })
            .unwrap();
        let drums = project
            .add_instrument(evil_project::Instrument::drum_kit("Kit", "maxwell"))
            .unwrap();
        project
            .add_track(Track {
                name: "Drums".to_owned(),
                instrument: drums,
                notes: vec![
                    Note {
                        pitch: 36,
                        ..Note::default()
                    },
                    Note {
                        pitch: 127,
                        ..Note::default()
                    },
                ],
                ..Track::default()
            })
            .unwrap();
        assert_stem_matches_manually_soloed_clone(&project, 0);
        assert_stem_matches_manually_soloed_clone(&project, 1);
        let drum_stem = render_project_scoped(
            &project,
            Path::new("."),
            RenderScope::Track { track_index: 1 },
        )
        .unwrap();
        assert_eq!(drum_stem.note_count, 1);
        assert_eq!(drum_stem.skipped_drum_notes, 1);
    }

    #[test]
    fn stems_include_only_selected_send_contribution_inserts_master_and_complete_tails() {
        let mut project = Project::new("Stem graph");
        project.master_gain = 0.65;
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project
            .add_track(Track {
                name: "Other".to_owned(),
                notes: vec![Note {
                    pitch: 72,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].inserts = vec![evil_synth::InsertEffect::Reverb(
            evil_synth::Reverb::default(),
        )];
        project.instruments[0].send_gains = [0.5, 0.0];
        project.instruments[1].send_gains = [1.0, 0.0];
        project.send_buses[0].gain = 0.5;
        project.send_buses[0].effect = Some(evil_synth::InsertEffect::Reverb(
            evil_synth::Reverb::default(),
        ));
        assert_stem_matches_manually_soloed_clone(&project, 0);
        let stem = render_project_scoped(
            &project,
            Path::new("."),
            RenderScope::Track { track_index: 0 },
        )
        .unwrap();
        assert!(stem.sound.left.len() > 24_000);
        assert!(
            stem.sound.left[24_000..]
                .iter()
                .any(|sample| *sample != 0.0)
        );
    }

    #[test]
    fn part_render_isolates_cropped_repetitions_at_frame_zero_with_a_full_tail() {
        use evil_project::{Part, Pattern};

        let mut project = Project::new("Part stem");
        project
            .add_track(Track {
                name: "Lead".to_owned(),
                notes: vec![Note {
                    pitch: 72,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        let pattern = project
            .add_pattern(Pattern {
                name: "Verse".to_owned(),
                length_ticks: 480,
                notes: vec![
                    Note {
                        start_tick: 0,
                        duration_ticks: 120,
                        ..Note::default()
                    },
                    Note {
                        start_tick: 240,
                        duration_ticks: 60,
                        ..Note::default()
                    },
                ],
                ..Pattern::default()
            })
            .unwrap();
        let selected = Part {
            pattern,
            start_tick: 480,
            length_ticks: 720,
        };
        project.add_part(0, selected).unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 2_400,
                    length_ticks: 480,
                },
            )
            .unwrap();
        project
            .add_track(Track {
                name: "Other".to_owned(),
                notes: vec![Note {
                    start_tick: 480,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].inserts = vec![evil_synth::InsertEffect::Reverb(
            evil_synth::Reverb::default(),
        )];

        let mut isolated = project.clone();
        isolated.tracks[1].muted = true;
        isolated.tracks[0].notes.clear();
        isolated.tracks[0].parts = vec![selected];
        let expected = render_project_range(&isolated, Path::new("."), 480, None).unwrap();
        let part = render_project_scoped(
            &project,
            Path::new("."),
            RenderScope::TrackPart {
                track_index: 0,
                part_index: 0,
            },
        )
        .unwrap();
        assert_eq!(
            part.note_count, 3,
            "two full occurrences and one cropped repeat"
        );
        assert_eq!(part.sound.left, expected.sound.left);
        assert_eq!(part.sound.right, expected.sound.right);
        assert_eq!(part.sound.left.len(), expected.sound.left.len());
        let part_end_frame = 60_000;
        assert!(part.sound.left.len() > part_end_frame);
        assert!(
            part.sound.left[part_end_frame..]
                .iter()
                .any(|sample| *sample != 0.0),
            "the export continues beyond the arrangement part boundary"
        );
    }

    #[test]
    fn invalid_and_explicitly_empty_scopes_are_clear_errors() {
        let mut project = Project::new("Invalid scopes");
        project.add_track(Track::default()).unwrap();
        assert!(matches!(
            render_project_scoped(
                &project,
                Path::new("."),
                RenderScope::Track { track_index: 3 }
            ),
            Err(Error::InvalidSelection(_))
        ));
        assert!(matches!(
            render_project_scoped(
                &project,
                Path::new("."),
                RenderScope::Track { track_index: 0 }
            ),
            Err(Error::EmptySelection(_))
        ));
        assert!(matches!(
            render_project_scoped(
                &project,
                Path::new("."),
                RenderScope::TrackPart {
                    track_index: 0,
                    part_index: 0,
                }
            ),
            Err(Error::InvalidSelection(_))
        ));
        assert!(matches!(
            render_project_scoped_range(
                &project,
                Path::new("."),
                RenderScope::TrackPart {
                    track_index: 0,
                    part_index: 0,
                },
                0,
                None,
            ),
            Err(Error::InvalidSelection(_))
        ));
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
