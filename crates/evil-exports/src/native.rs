use std::collections::{BTreeMap, btree_map::Entry};
use std::path::Path;

use evil_project::InstrumentSource;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::{Error, Performance, TimedEvent, render_track};

const MAX_FILE_SIZE: usize = 1024 * 1024;
const MAX_EVENT_COUNT: usize = 262_144;

#[derive(Debug)]
pub struct NativeMidiReport {
    pub data: Vec<u8>,
    pub note_count: usize,
    pub track_count: usize,
    pub event_count: usize,
}

pub fn export_native_project_to_path(
    project: &evil_project::Project,
    patch_base: &Path,
    output: &Path,
) -> Result<NativeMidiReport, Error> {
    let report = export_native_project(project, patch_base)?;
    evil_project::atomic_write(output, &report.data)?;
    Ok(report)
}

pub fn export_native_project(
    project: &evil_project::Project,
    patch_base: &Path,
) -> Result<NativeMidiReport, Error> {
    let performance = project.to_performance()?;
    let mut tracks = vec![tempo_track(&performance)?];
    let mut next_channel = 0_u8;
    let mut instrument_channels = BTreeMap::new();
    let mut track_count = 0;
    let mut event_count = tracks[0].len();

    for (track_index, track) in project.tracks.iter().enumerate() {
        let instrument = project.instrument_for_track(track_index)?;
        let patch_program = match &instrument.source {
            InstrumentSource::Synth { patch } => {
                let patch = evil_render::load_patch(patch, patch_base)?;
                if let Entry::Vacant(entry) = instrument_channels.entry(track.instrument) {
                    let channel = if patch.pitch_tracking {
                        next_melodic_channel(&mut next_channel)?
                    } else {
                        9
                    };
                    entry.insert(channel);
                }
                patch.midi_program
            }
            InstrumentSource::DrumKit { .. } => {
                instrument_channels.entry(track.instrument).or_insert(9);
                None
            }
        };
        let channel = instrument_channels[&track.instrument];
        let mut events = Vec::new();
        if let Some(performance_track) = performance
            .tracks
            .iter()
            .find(|candidate| candidate.track_index == track_index)
        {
            for note in &performance_track.notes {
                events.push(TimedEvent {
                    tick: note.start_tick,
                    priority: 1,
                    message: MidiMessage::NoteOn {
                        key: u7::new(note.note),
                        vel: u7::new(note.velocity),
                    },
                });
                events.push(TimedEvent {
                    tick: note.start_tick + note.duration_ticks,
                    priority: 0,
                    message: MidiMessage::NoteOff {
                        key: u7::new(note.note),
                        vel: u7::new(0),
                    },
                });
            }

            for bend in &performance_track.pitch_bends {
                events.push(TimedEvent {
                    tick: bend.tick,
                    priority: 0,
                    message: MidiMessage::PitchBend {
                        bend: midly::PitchBend::from_int(bend.value),
                    },
                });
            }

            for pressure in &performance_track.aftertouch {
                let message = match pressure.note {
                    Some(note) => MidiMessage::Aftertouch {
                        key: u7::new(note),
                        vel: u7::new(pressure.value),
                    },
                    None => MidiMessage::ChannelAftertouch {
                        vel: u7::new(pressure.value),
                    },
                };
                events.push(TimedEvent {
                    tick: pressure.tick,
                    priority: 0,
                    message,
                });
            }

            for event in &performance_track.controllers {
                events.push(TimedEvent {
                    tick: event.tick,
                    priority: 0,
                    message: MidiMessage::Controller {
                        controller: u7::new(event.controller),
                        value: u7::new(event.value),
                    },
                });
            }
        }
        events.sort_by_key(|event| (event.tick, event.priority));
        let channel = u4::new(channel);
        let mut midi_track = render_track(events, channel, &track.name)?;
        let mut setup_events = Vec::new();
        setup_events.extend(pitch_bend_range_events(
            channel,
            instrument.pitch_bend_range_semitones,
        ));
        if let Some(program) = instrument.midi_program.or(patch_program) {
            setup_events.push(TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::ProgramChange {
                        program: u7::new(program),
                    },
                },
            });
        }
        setup_events.push(controller_event(channel, 7, gain_value(instrument.gain)));
        setup_events.push(controller_event(channel, 10, pan_value(instrument.pan)));
        midi_track.splice(1..1, setup_events);
        event_count += midi_track.len();
        track_count += 1;
        tracks.push(midi_track);
    }

    if performance.ticks_per_quarter == 0 || performance.ticks_per_quarter > 32_767 {
        return Err(Error::UnsupportedTiming);
    }
    let smf = Smf {
        header: Header {
            format: Format::Parallel,
            timing: Timing::Metrical(u15::new(performance.ticks_per_quarter)),
        },
        tracks,
    };
    let mut data = Vec::new();
    smf.write(&mut data).map_err(Error::Encode)?;
    if data.len() > MAX_FILE_SIZE || event_count > MAX_EVENT_COUNT {
        return Err(Error::ExportTooLarge {
            bytes: data.len(),
            events: event_count,
        });
    }
    Ok(NativeMidiReport {
        data,
        note_count: performance.note_count(),
        track_count,
        event_count,
    })
}

fn tempo_track(performance: &Performance) -> Result<Vec<TrackEvent<'_>>, Error> {
    let mut track = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"MIDI Evil")),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(
                u8::try_from(performance.beats_per_bar)
                    .map_err(|_| Error::UnsupportedMeter(performance.beats_per_bar))?,
                crate::meter_exponent(performance.beat_unit)?,
                24,
                8,
            )),
        },
    ];
    let mut previous_tick = 0_u64;
    for change in &performance.tempo_changes {
        let delta = change.tick - previous_tick;
        if delta > crate::MAX_MIDI_DELTA {
            return Err(Error::TimelineTooLong);
        }
        track.push(TrackEvent {
            delta: u28::new(delta as u32),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(
                change.microseconds_per_quarter,
            ))),
        });
        previous_tick = change.tick;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(track)
}

fn next_melodic_channel(next: &mut u8) -> Result<u8, Error> {
    if *next == 9 {
        *next += 1;
    }
    if *next > 15 {
        return Err(Error::TooManyTracks(usize::from(*next) + 1));
    }
    let channel = *next;
    *next += 1;
    Ok(channel)
}

fn controller_event(channel: u4, controller: u8, value: u8) -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Midi {
            channel,
            message: MidiMessage::Controller {
                controller: u7::new(controller),
                value: u7::new(value),
            },
        },
    }
}

fn pitch_bend_range_events(channel: u4, range: f32) -> Vec<TrackEvent<'static>> {
    if range == 2.0 {
        return Vec::new();
    }
    let mut semitones = range.trunc() as u8;
    let mut cents = (range.fract() * 100.0).round() as u8;
    if cents == 100 {
        semitones += 1;
        cents = 0;
    }
    vec![
        controller_event(channel, 101, 0),
        controller_event(channel, 100, 0),
        controller_event(channel, 6, semitones),
        controller_event(channel, 38, cents),
    ]
}

fn gain_value(gain: f32) -> u8 {
    (gain.clamp(0.0, 1.0) * 127.0).round() as u8
}

fn pan_value(pan: f32) -> u8 {
    ((pan.clamp(-1.0, 1.0) + 1.0) * 63.5).round() as u8
}

#[cfg(test)]
mod tests {
    use evil_project::{Aftertouch, ControllerEvent, Note, PitchBend, TempoChange, Track};

    use super::*;

    #[test]
    fn native_export_preserves_engine_metadata_and_channels() {
        let mut project = evil_project::Project::new("Metadata");
        project.beats_per_bar = 5;
        project.beat_unit = 2;
        project.tempo_changes = vec![TempoChange {
            tick: 480,
            bpm: 240.0,
        }];
        project
            .add_track(Track {
                name: "Lead & co".to_owned(),
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].gain = 0.5;
        project.instruments[0].pan = -1.0;
        project
            .add_track(Track {
                name: "Muted".to_owned(),
                muted: true,
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let drums = project
            .add_instrument(evil_project::Instrument::synth("Drums", "kick"))
            .unwrap();
        project
            .add_track(Track {
                name: "Drums".to_owned(),
                instrument: drums,
                notes: vec![Note {
                    pitch: 36,
                    ..Note::default()
                }],
                ..Track::default()
            })
            .unwrap();

        let report = export_native_project(&project, Path::new(".")).unwrap();
        assert_eq!(report.note_count, 2);
        assert_eq!(report.track_count, 3);
        let smf = Smf::parse(&report.data).unwrap();
        assert_eq!(smf.tracks.len(), 4);
        assert_eq!(smf.header.timing, Timing::Metrical(u15::new(480)));
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::TimeSignature(5, 1, 24, 8))
        )));
        let tempo_deltas: Vec<_> = smf.tracks[0]
            .iter()
            .filter_map(|event| match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(_)) => Some(u32::from(event.delta)),
                _ => None,
            })
            .collect();
        assert_eq!(tempo_deltas, [0, 480]);
        assert!(has_track_name(&smf.tracks[1], "Lead & co"));
        assert!(has_track_name(&smf.tracks[2], "Muted"));
        assert!(!has_note_on(&smf.tracks[2]));
        assert!(has_program(&smf.tracks[1], 80));
        assert!(has_controller(&smf.tracks[1], 7, 64));
        assert!(has_controller(&smf.tracks[1], 10, 0));
        assert!(smf.tracks[3].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Midi { channel, .. } if channel.as_int() == 9
        )));
        assert!(matches!(
            smf.tracks[1].last().map(|event| event.kind),
            Some(TrackEventKind::Meta(MetaMessage::EndOfTrack))
        ));
    }

    #[test]
    fn melodic_channels_skip_percussion_channel() {
        let mut next = 8;
        assert_eq!(next_melodic_channel(&mut next).unwrap(), 8);
        assert_eq!(next_melodic_channel(&mut next).unwrap(), 10);
        assert_eq!(next_melodic_channel(&mut next).unwrap(), 11);
    }

    #[test]
    fn tracks_sharing_an_instrument_reuse_its_channel() {
        let mut project = evil_project::Project::new("Shared channels");
        let instrument = project
            .add_instrument(evil_project::Instrument::default())
            .unwrap();
        for index in 0..64 {
            project
                .add_track(Track {
                    name: format!("Part {index}"),
                    instrument,
                    notes: vec![Note::default()],
                    ..Track::default()
                })
                .unwrap();
        }
        let report = export_native_project(&project, Path::new(".")).unwrap();
        assert_eq!(report.note_count, 64);
        assert_eq!(report.track_count, 64);
        let smf = Smf::parse(&report.data).unwrap();
        assert!(smf.tracks.iter().skip(1).all(|track| {
            track.iter().all(|event| {
                matches!(event.kind, TrackEventKind::Midi { channel, .. } if channel.as_int() == 0)
                    || matches!(event.kind, TrackEventKind::Meta(_))
            })
        }));
        assert!(has_track_name(&smf.tracks[1], "Part 0"));
        assert!(has_track_name(&smf.tracks[64], "Part 63"));
    }

    #[test]
    fn native_pitch_bend_round_trip_preserves_signed_values() {
        let mut project = evil_project::Project::new("Bend");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                pitch_bends: vec![
                    PitchBend {
                        tick: 0,
                        value: -8192,
                    },
                    PitchBend {
                        tick: 64,
                        value: 8191,
                    },
                ],
                aftertouch: vec![
                    Aftertouch {
                        tick: 0,
                        note: None,
                        value: 32,
                    },
                    Aftertouch {
                        tick: 64,
                        note: Some(60),
                        value: 127,
                    },
                ],
                controllers: vec![
                    ControllerEvent {
                        tick: 32,
                        controller: 1,
                        value: 96,
                    },
                    ControllerEvent {
                        tick: 64,
                        controller: 2,
                        value: 90,
                    },
                ],
                ..Track::default()
            })
            .unwrap();
        project.instruments[0].pitch_bend_range_semitones = 12.5;
        let report = export_native_project(&project, Path::new(".")).unwrap();
        let performance = evil_imports::import_midi(&report.data).unwrap();
        assert_eq!(performance.pitch_bend_count(), 2);
        assert_eq!(
            performance.tracks[0].pitch_bends,
            project.tracks[0].pitch_bends
        );
        assert_eq!(performance.aftertouch_count(), 2);
        assert_eq!(
            performance.tracks[0].aftertouch,
            project.tracks[0].aftertouch
        );
        assert_eq!(performance.controller_count(), 2);
        assert_eq!(
            performance.tracks[0].controllers,
            project.tracks[0].controllers
        );
        assert_eq!(performance.skipped_controller_events(), 1);
        assert_eq!(performance.tracks[0].pitch_bend_range_semitones, 12.5);
        assert_eq!(project.instruments[0].pitch_bend_range_semitones, 12.5);
    }

    #[test]
    fn native_export_writes_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let mut project = evil_project::Project::new("Atomic");
        project
            .add_track(Track {
                notes: vec![Note::default()],
                ..Track::default()
            })
            .unwrap();
        let output = directory.path().join("song.mid");
        export_native_project_to_path(&project, Path::new("."), &output).unwrap();
        let data = std::fs::read(&output).unwrap();
        let smf = Smf::parse(&data).unwrap();
        assert_eq!(smf.tracks.len(), 2);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    fn has_track_name(track: &[TrackEvent<'_>], expected: &str) -> bool {
        track.iter().any(|event| {
            matches!(event.kind, TrackEventKind::Meta(MetaMessage::TrackName(name)) if name == expected.as_bytes())
        })
    }

    fn has_note_on(track: &[TrackEvent<'_>]) -> bool {
        track.iter().any(|event| {
            matches!(
                event.kind,
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { .. },
                    ..
                }
            )
        })
    }

    fn has_program(track: &[TrackEvent<'_>], program: u8) -> bool {
        track.iter().any(|event| {
            matches!(event.kind, TrackEventKind::Midi { message: MidiMessage::ProgramChange { program: actual }, .. } if actual.as_int() == program)
        })
    }

    fn has_controller(track: &[TrackEvent<'_>], controller: u8, value: u8) -> bool {
        track.iter().any(|event| {
            matches!(event.kind, TrackEventKind::Midi { message: MidiMessage::Controller { controller: actual_controller, value: actual_value }, .. } if actual_controller.as_int() == controller && actual_value.as_int() == value)
        })
    }
}
