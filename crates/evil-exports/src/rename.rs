use midly::{MetaMessage, Smf, TrackEventKind};

use crate::Error;

/// Rename a track while preserving its notes, timing, and other events.
pub fn rename_midi_track(data: &[u8], track_index: usize, name: &str) -> Result<Vec<u8>, Error> {
    let mut smf = Smf::parse(data)?;
    let track = smf
        .tracks
        .get_mut(track_index)
        .ok_or(Error::MissingTrack(track_index))?;
    let track_name = TrackEventKind::Meta(MetaMessage::TrackName(name.as_bytes()));
    if let Some(event) = track
        .iter_mut()
        .find(|event| matches!(event.kind, TrackEventKind::Meta(MetaMessage::TrackName(_))))
    {
        event.kind = track_name;
    } else {
        track.insert(
            0,
            midly::TrackEvent {
                delta: midly::num::u28::new(0),
                kind: track_name,
            },
        );
    }
    let mut output = Vec::new();
    smf.write(&mut output).map_err(Error::Encode)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use midly::num::{u4, u7, u15, u24, u28};
    use midly::{Format, Header, MidiMessage, Timing, TrackEvent};

    use super::*;

    #[test]
    fn renaming_a_track_changes_only_its_name() {
        let smf = Smf {
            header: Header {
                format: Format::Parallel,
                timing: Timing::Metrical(u15::new(480)),
            },
            tracks: vec![
                vec![
                    TrackEvent {
                        delta: u28::new(0),
                        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
                    },
                    TrackEvent {
                        delta: u28::new(0),
                        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
                    },
                ],
                vec![
                    TrackEvent {
                        delta: u28::new(0),
                        kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Lead")),
                    },
                    TrackEvent {
                        delta: u28::new(128),
                        kind: TrackEventKind::Midi {
                            channel: u4::new(0),
                            message: MidiMessage::NoteOn {
                                key: u7::new(60),
                                vel: u7::new(100),
                            },
                        },
                    },
                    TrackEvent {
                        delta: u28::new(64),
                        kind: TrackEventKind::Midi {
                            channel: u4::new(0),
                            message: MidiMessage::NoteOff {
                                key: u7::new(60),
                                vel: u7::new(0),
                            },
                        },
                    },
                    TrackEvent {
                        delta: u28::new(0),
                        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
                    },
                ],
            ],
        };
        let mut data = Vec::new();
        smf.write(&mut data).unwrap();

        let renamed = rename_midi_track(&data, 1, "Lead & co").unwrap();
        let original_performance = evil_imports::import_midi(&data).unwrap();
        let renamed_performance = evil_imports::import_midi(&renamed).unwrap();
        assert_eq!(renamed_performance.tracks[0].name, "Lead & co");
        assert_eq!(
            original_performance.tracks[0].notes,
            renamed_performance.tracks[0].notes
        );
        assert!(rename_midi_track(&data, 99, "Missing").is_err());
    }
}
