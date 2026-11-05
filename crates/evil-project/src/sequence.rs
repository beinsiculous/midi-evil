use crate::{
    Aftertouch, ControllerEvent, Error, MixerAutomationEvent, Note, Pattern, PitchBend, Track,
    invalid,
};

/// The event storage shared by unique track performances and reusable patterns.
/// Editors use this interface so both contexts have identical editing behavior.
pub trait EventSequence {
    fn notes(&self) -> &[Note];
    fn notes_mut(&mut self) -> &mut Vec<Note>;
    fn pitch_bends(&self) -> &[PitchBend];
    fn pitch_bends_mut(&mut self) -> &mut Vec<PitchBend>;
    fn aftertouch(&self) -> &[Aftertouch];
    fn aftertouch_mut(&mut self) -> &mut Vec<Aftertouch>;
    fn controllers(&self) -> &[ControllerEvent];
    fn controllers_mut(&mut self) -> &mut Vec<ControllerEvent>;
    fn mixer_automation(&self) -> &[MixerAutomationEvent];
    fn mixer_automation_mut(&mut self) -> &mut Vec<MixerAutomationEvent>;
}

macro_rules! impl_event_sequence {
    ($kind:ty) => {
        impl EventSequence for $kind {
            fn notes(&self) -> &[Note] {
                &self.notes
            }
            fn notes_mut(&mut self) -> &mut Vec<Note> {
                &mut self.notes
            }
            fn pitch_bends(&self) -> &[PitchBend] {
                &self.pitch_bends
            }
            fn pitch_bends_mut(&mut self) -> &mut Vec<PitchBend> {
                &mut self.pitch_bends
            }
            fn aftertouch(&self) -> &[Aftertouch] {
                &self.aftertouch
            }
            fn aftertouch_mut(&mut self) -> &mut Vec<Aftertouch> {
                &mut self.aftertouch
            }
            fn controllers(&self) -> &[ControllerEvent] {
                &self.controllers
            }
            fn controllers_mut(&mut self) -> &mut Vec<ControllerEvent> {
                &mut self.controllers
            }
            fn mixer_automation(&self) -> &[MixerAutomationEvent] {
                &self.mixer_automation
            }
            fn mixer_automation_mut(&mut self) -> &mut Vec<MixerAutomationEvent> {
                &mut self.mixer_automation
            }
        }
    };
}

impl_event_sequence!(Track);
impl_event_sequence!(Pattern);

pub fn quantize_notes(
    sequence: &mut (impl EventSequence + ?Sized),
    grid_ticks: u64,
    selected: Option<usize>,
    length_ticks: Option<u64>,
) -> Result<(), Error> {
    if grid_ticks == 0 {
        return Err(invalid("quantize grid must be greater than zero"));
    }
    let updates: Result<Vec<_>, Error> = sequence
        .notes()
        .iter()
        .enumerate()
        .filter(|(index, _)| selected.is_none_or(|selected| selected == *index))
        .map(|(index, note)| {
            let start_tick =
                note.start_tick.saturating_add(grid_ticks / 2) / grid_ticks * grid_ticks;
            let duration_ticks = (note.duration_ticks.saturating_add(grid_ticks / 2) / grid_ticks
                * grid_ticks)
                .max(grid_ticks);
            start_tick
                .checked_add(duration_ticks)
                .ok_or_else(|| invalid("quantized note end exceeds the supported range"))?;
            Ok((index, start_tick, duration_ticks))
        })
        .collect();
    for (index, start_tick, duration_ticks) in updates? {
        let note = &mut sequence.notes_mut()[index];
        note.start_tick = start_tick;
        note.duration_ticks = duration_ticks;
    }
    if let Some(length) = length_ticks {
        sequence.notes_mut().retain(|note| note.start_tick < length);
    }
    Ok(())
}

pub fn transpose_notes(
    sequence: &mut (impl EventSequence + ?Sized),
    semitones: i16,
    selected: Option<usize>,
) -> Result<(), Error> {
    let notes = sequence.notes_mut();
    if notes.iter().enumerate().any(|(index, note)| {
        (selected.is_none() || selected == Some(index))
            && !(0..=127).contains(&(i16::from(note.pitch) + semitones))
    }) {
        return Err(invalid(
            "transpose would move a note outside the MIDI range",
        ));
    }
    for (index, note) in notes.iter_mut().enumerate() {
        if selected.is_none() || selected == Some(index) {
            note.pitch = (i16::from(note.pitch) + semitones) as u8;
        }
    }
    Ok(())
}

/// Shorten same-pitch overlaps without changing notes on other pitches.
pub fn clean_overlaps(sequence: &mut (impl EventSequence + ?Sized)) {
    let notes = sequence.notes_mut();
    notes.sort_by_key(|note| (note.pitch, note.start_tick));
    for index in 0..notes.len().saturating_sub(1) {
        let next_start = notes[index + 1].start_tick;
        let note = &mut notes[index];
        if note.start_tick < next_start {
            note.duration_ticks = note.duration_ticks.min(next_start - note.start_tick);
        }
    }
    notes.sort_by_key(|note| note.start_tick);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start: u64, duration: u64) -> Note {
        Note {
            pitch,
            start_tick: start,
            duration_ticks: duration,
            ..Note::default()
        }
    }

    #[test]
    fn quantize_snaps_selected_notes_without_underflow() {
        let mut track = Track {
            notes: vec![note(60, 10, 20), note(62, 130, 20)],
            ..Track::default()
        };
        quantize_notes(&mut track, 120, Some(0), None).unwrap();
        assert_eq!(track.notes[0].start_tick, 0);
        assert_eq!(track.notes[0].duration_ticks, 120);
        assert_eq!(track.notes[1].start_tick, 130);
        quantize_notes(&mut track, 120, None, Some(1920)).unwrap();
        assert_eq!(track.notes[1].start_tick, 120);
    }

    #[test]
    fn transpose_is_atomic_at_midi_boundaries() {
        let mut track = Track {
            notes: vec![note(60, 0, 120), note(127, 120, 120)],
            ..Track::default()
        };
        assert!(transpose_notes(&mut track, 1, None).is_err());
        assert_eq!(track.notes[0].pitch, 60);
        transpose_notes(&mut track, 12, Some(0)).unwrap();
        assert_eq!(track.notes[0].pitch, 72);
        assert_eq!(track.notes[1].pitch, 127);
    }

    #[test]
    fn clean_only_shortens_same_pitch_overlaps() {
        let mut track = Track {
            notes: vec![note(60, 120, 240), note(62, 120, 240), note(60, 0, 240)],
            ..Track::default()
        };
        clean_overlaps(&mut track);
        assert_eq!(track.notes[0].duration_ticks, 120);
        assert_eq!(track.notes[1].duration_ticks, 240);
        assert_eq!(track.notes[2].duration_ticks, 240);
    }
}
