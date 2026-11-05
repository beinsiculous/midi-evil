use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use crate::{
    Aftertouch, ControllerEvent, Error, Performance, PerformanceNote, PerformanceTrack, PitchBend,
    TempoChange,
};

#[derive(Clone, Copy)]
struct ActiveNote {
    start_tick: u64,
    velocity: u8,
}

pub fn import_midi_file(path: impl AsRef<Path>) -> Result<Performance, Error> {
    let path = path.as_ref();
    let data = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    import_midi(&data)
}

pub fn import_midi(data: &[u8]) -> Result<Performance, Error> {
    let smf = Smf::parse(data)?;
    if smf.header.format == Format::Sequential {
        return Err(Error::UnsupportedFormat);
    }
    let Timing::Metrical(division) = smf.header.timing else {
        return Err(Error::UnsupportedTiming);
    };
    let ticks_per_quarter = division.as_int();
    if ticks_per_quarter == 0 {
        return Err(Error::UnsupportedTiming);
    }

    let mut tracks = Vec::new();
    let mut tempo_changes = Vec::new();
    let mut beats_per_bar = 4;
    let mut beat_unit = 4;
    for (track_index, events) in smf.tracks.iter().enumerate() {
        let mut tick = 0_u64;
        let mut name = None;
        let mut notes = Vec::new();
        let mut pitch_bends = Vec::new();
        let mut aftertouch = Vec::new();
        let mut controllers = Vec::new();
        let mut pitch_bend_range_semitones = 2.0_f32;
        let mut selected_rpn = None;
        let mut rpn_msb = 0;
        let mut rpn_lsb = 0;
        let mut active: BTreeMap<(u8, u8), VecDeque<ActiveNote>> = BTreeMap::new();
        let mut skipped_controller_events = 0;
        let mut program = None;
        let mut volume = None;
        let mut pan = None;
        for event in events {
            tick += u64::from(u32::from(event.delta));
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(bytes)) if name.is_none() => {
                    let value = String::from_utf8_lossy(bytes).into_owned();
                    if !value.is_empty() {
                        name = Some(value);
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(microseconds)) => {
                    if microseconds.as_int() == 0 {
                        return Err(Error::UnsupportedTempo(0.0));
                    }
                    tempo_changes.push(TempoChange {
                        tick,
                        microseconds_per_quarter: microseconds.as_int(),
                    });
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, denominator, ..)) => {
                    if numerator == 0 {
                        return Err(Error::InvalidTimeSignature {
                            numerator,
                            denominator,
                        });
                    }
                    beats_per_bar = u32::from(numerator);
                    beat_unit = 1_u32.checked_shl(u32::from(denominator)).ok_or(
                        Error::InvalidTimeSignature {
                            numerator,
                            denominator,
                        },
                    )?;
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            active
                                .entry((channel, key.as_int()))
                                .or_default()
                                .push_back(ActiveNote {
                                    start_tick: tick,
                                    velocity: vel.as_int(),
                                });
                        }
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                            if let Some(note) = active
                                .get_mut(&(channel, key.as_int()))
                                .and_then(VecDeque::pop_front)
                            {
                                push_note(&mut notes, note, key.as_int(), tick);
                            }
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 7 =>
                        {
                            volume = Some(value.as_int());
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 10 =>
                        {
                            pan = Some(value.as_int());
                        }
                        MidiMessage::Controller { controller, value }
                            if evil_performance::controller_is_rendered(controller.as_int()) =>
                        {
                            controllers.push(ControllerEvent {
                                tick,
                                controller: controller.as_int(),
                                value: value.as_int(),
                            });
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 101 =>
                        {
                            rpn_msb = value.as_int();
                            selected_rpn = Some((rpn_msb, rpn_lsb));
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 100 =>
                        {
                            rpn_lsb = value.as_int();
                            selected_rpn = Some((rpn_msb, rpn_lsb));
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 6 && selected_rpn == Some((0, 0)) =>
                        {
                            pitch_bend_range_semitones =
                                f32::from(value.as_int()) + pitch_bend_range_semitones.fract();
                        }
                        MidiMessage::Controller { controller, value }
                            if controller.as_int() == 38 && selected_rpn == Some((0, 0)) =>
                        {
                            pitch_bend_range_semitones = pitch_bend_range_semitones.trunc()
                                + f32::from(value.as_int()) / 100.0;
                        }
                        MidiMessage::ProgramChange { program: actual } => {
                            program = Some(actual.as_int());
                        }
                        MidiMessage::PitchBend { bend } => {
                            pitch_bends.push(PitchBend {
                                tick,
                                value: bend.as_int(),
                            });
                        }
                        MidiMessage::Aftertouch { key, vel } => {
                            aftertouch.push(Aftertouch {
                                tick,
                                note: Some(key.as_int()),
                                value: vel.as_int(),
                            });
                        }
                        MidiMessage::ChannelAftertouch { vel } => {
                            aftertouch.push(Aftertouch {
                                tick,
                                note: None,
                                value: vel.as_int(),
                            });
                        }
                        MidiMessage::Controller { controller, value } => {
                            controllers.push(ControllerEvent {
                                tick,
                                controller: controller.as_int(),
                                value: value.as_int(),
                            });
                            skipped_controller_events += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        for (key, active_notes) in active {
            for note in active_notes {
                push_note(&mut notes, note, key.1, tick);
            }
        }
        if !notes.is_empty()
            || !pitch_bends.is_empty()
            || !aftertouch.is_empty()
            || !controllers.is_empty()
        {
            notes.sort_by_key(|note| (note.start_tick, note.note));
            tracks.push(PerformanceTrack {
                track_index,
                name: name.unwrap_or_else(|| format!("Track {track_index}")),
                notes,
                pitch_bends,
                aftertouch,
                controllers,
                mixer_automation: Vec::new(),
                pitch_bend_range_semitones,
                program,
                volume,
                pan,
                skipped_controller_events,
            });
        }
    }

    tempo_changes.sort_by_key(|change| change.tick);
    tempo_changes.dedup_by(|next, previous| {
        if next.tick == previous.tick {
            *previous = *next;
            true
        } else {
            false
        }
    });
    if tempo_changes.first().is_none_or(|change| change.tick != 0) {
        tempo_changes.insert(
            0,
            TempoChange {
                tick: 0,
                microseconds_per_quarter: 500_000,
            },
        );
    }
    let initial_tempo = tempo_changes[0].microseconds_per_quarter;
    Ok(Performance {
        tempo_bpm: (60_000_000.0 / f64::from(initial_tempo)) as f32,
        beats_per_bar,
        beat_unit,
        ticks_per_quarter,
        tempo_changes,
        tracks,
    })
}

fn push_note(notes: &mut Vec<PerformanceNote>, active: ActiveNote, key: u8, end_tick: u64) {
    if end_tick > active.start_tick {
        notes.push(PerformanceNote {
            note: key,
            velocity: active.velocity,
            start_tick: active.start_tick,
            duration_ticks: end_tick - active.start_tick,
        });
    }
}

#[cfg(test)]
mod tests {
    use midly::num::{u4, u7, u15, u24, u28};
    use midly::{Header, TrackEvent};

    use super::*;

    #[test]
    fn imports_notes_track_names_and_tempo() {
        let data = midi_file(vec![
            vec![
                event(
                    0,
                    TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
                ),
                event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
            ],
            vec![
                event(0, TrackEventKind::Meta(MetaMessage::TrackName(b"Lead"))),
                note_event(128, true, 60, 100),
                note_event(64, false, 60, 0),
                event(
                    0,
                    TrackEventKind::Midi {
                        channel: u4::new(0),
                        message: MidiMessage::Controller {
                            controller: u7::new(2),
                            value: u7::new(64),
                        },
                    },
                ),
                event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
            ],
        ]);
        let performance = import_midi(&data).unwrap();
        assert_eq!(performance.note_count(), 1);
        assert_eq!(performance.tracks[0].track_index, 1);
        assert_eq!(performance.tracks[0].name, "Lead");
        assert_eq!(performance.tracks[0].skipped_controller_events, 1);
        let note = performance.tracks[0].notes[0];
        assert_eq!(note.start_tick, 128);
        assert_eq!(note.duration_ticks, 64);
        let (start, duration) = performance.note_timing(&note);
        assert!((start - 0.5).abs() < 0.000_001);
        assert!((duration - 0.25).abs() < 0.000_001);
    }

    #[test]
    fn tempo_changes_produce_correct_note_seconds() {
        let data = midi_file(vec![
            vec![
                event(
                    0,
                    TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
                ),
                event(
                    128,
                    TrackEventKind::Meta(MetaMessage::Tempo(u24::new(250_000))),
                ),
                event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
            ],
            vec![
                note_event(128, true, 60, 100),
                note_event(128, false, 60, 0),
                event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
            ],
        ]);
        let performance = import_midi(&data).unwrap();
        let (start, duration) = performance.note_timing(&performance.tracks[0].notes[0]);
        assert!((start - 0.5).abs() < 0.000_001);
        assert!((duration - 0.25).abs() < 0.000_001);
    }

    #[test]
    fn imports_program_volume_and_pan() {
        let data = midi_file(vec![vec![
            event(
                0,
                TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            ),
            event(
                0,
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::ProgramChange {
                        program: u7::new(80),
                    },
                },
            ),
            event(
                0,
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::Controller {
                        controller: u7::new(7),
                        value: u7::new(100),
                    },
                },
            ),
            event(
                0,
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::Controller {
                        controller: u7::new(10),
                        value: u7::new(32),
                    },
                },
            ),
            note_event(128, true, 60, 100),
            note_event(64, false, 60, 0),
            event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
        ]]);
        let performance = import_midi(&data).unwrap();
        let track = &performance.tracks[0];
        assert_eq!(track.program, Some(80));
        assert_eq!(track.volume, Some(100));
        assert_eq!(track.pan, Some(32));
        let project = evil_project::Project::from_performance("Mix", &performance).unwrap();
        assert_eq!(project.instruments[0].midi_program, Some(80));
        assert_eq!(project.instruments[0].gain, 100.0 / 127.0);
        assert!(project.instruments[0].pan < 0.0);
    }

    #[test]
    fn imports_pitch_bend_and_aftertouch() {
        let data = midi_file(vec![vec![
            note_event(0, true, 60, 100),
            event(
                64,
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::PitchBend {
                        bend: midly::PitchBend::from_int(8191),
                    },
                },
            ),
            event(
                0,
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::ChannelAftertouch { vel: u7::new(64) },
                },
            ),
            note_event(64, false, 60, 0),
            event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
        ]]);
        let performance = import_midi(&data).unwrap();
        assert_eq!(performance.pitch_bend_count(), 1);
        assert_eq!(performance.aftertouch_count(), 1);
        assert_eq!(performance.skipped_controller_events(), 0);
        assert_eq!(performance.tracks[0].pitch_bends[0].tick, 64);
        assert_eq!(performance.tracks[0].pitch_bends[0].value, 8191);
        assert_eq!(performance.tracks[0].aftertouch[0].note, None);
        assert_eq!(performance.tracks[0].aftertouch[0].value, 64);

        let project = evil_project::Project::from_performance("Bend", &performance).unwrap();
        assert_eq!(
            project.tracks[0].pitch_bends,
            performance.tracks[0].pitch_bends
        );
        assert_eq!(
            project.tracks[0].aftertouch,
            performance.tracks[0].aftertouch
        );
        assert_eq!(project.instruments[0].pitch_bend_range_semitones, 2.0);
    }

    #[test]
    fn imports_rpn_range_and_supported_and_unsupported_controllers() {
        let data = midi_file(vec![vec![
            controller_event(0, 101, 0),
            controller_event(0, 100, 0),
            controller_event(0, 6, 12),
            controller_event(0, 38, 50),
            controller_event(0, 1, 64),
            controller_event(0, 11, 100),
            controller_event(0, 2, 90),
            event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
        ]]);
        let performance = import_midi(&data).unwrap();
        assert_eq!(performance.tracks[0].pitch_bend_range_semitones, 12.5);
        assert_eq!(performance.controller_count(), 3);
        assert_eq!(performance.skipped_controller_events(), 1);
        assert_eq!(performance.tracks[0].controllers[2].controller, 2);

        let project = evil_project::Project::from_performance("Controllers", &performance).unwrap();
        assert_eq!(project.instruments[0].pitch_bend_range_semitones, 12.5);
        assert_eq!(
            project.tracks[0].controllers,
            performance.tracks[0].controllers
        );
    }

    #[test]
    fn overlapping_notes_on_the_same_pitch_pair_fifo() {
        let data = midi_file(vec![vec![
            event(
                0,
                TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            ),
            note_event(0, true, 60, 100),
            note_event(64, true, 60, 90),
            note_event(64, false, 60, 0),
            note_event(64, false, 60, 0),
            event(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
        ]]);
        let performance = import_midi(&data).unwrap();
        let notes = &performance.tracks[0].notes;
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].start_tick, 0);
        assert_eq!(notes[0].duration_ticks, 128);
        assert_eq!(notes[0].velocity, 100);
        assert_eq!(notes[1].start_tick, 64);
        assert_eq!(notes[1].duration_ticks, 128);
        assert_eq!(notes[1].velocity, 90);
    }

    fn controller_event(delta: u32, controller: u8, value: u8) -> TrackEvent<'static> {
        event(
            delta,
            TrackEventKind::Midi {
                channel: u4::new(0),
                message: MidiMessage::Controller {
                    controller: u7::new(controller),
                    value: u7::new(value),
                },
            },
        )
    }

    fn note_event(delta: u32, on: bool, key: u8, velocity: u8) -> TrackEvent<'static> {
        event(
            delta,
            TrackEventKind::Midi {
                channel: u4::new(0),
                message: if on {
                    MidiMessage::NoteOn {
                        key: u7::new(key),
                        vel: u7::new(velocity),
                    }
                } else {
                    MidiMessage::NoteOff {
                        key: u7::new(key),
                        vel: u7::new(0),
                    }
                },
            },
        )
    }

    fn event(delta: u32, kind: TrackEventKind<'static>) -> TrackEvent<'static> {
        TrackEvent {
            delta: u28::new(delta),
            kind,
        }
    }

    fn midi_file(tracks: Vec<Vec<TrackEvent<'static>>>) -> Vec<u8> {
        let smf = Smf {
            header: Header {
                format: Format::Parallel,
                timing: Timing::Metrical(u15::new(128)),
            },
            tracks,
        };
        let mut data = Vec::new();
        smf.write(&mut data).unwrap();
        data
    }
}
