use crate::SynthNote;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VoiceAllocation {
    pub gate_frame: usize,
    pub release_end_frame: usize,
    pub audible_end_frame: usize,
}

#[derive(Clone, Copy)]
struct ActiveVoice {
    note_index: usize,
    pitch: u8,
    released: bool,
    release_end_frame: usize,
}

#[derive(Clone, Copy)]
enum EventKind {
    Off,
    On,
}

#[derive(Clone, Copy)]
struct Event {
    frame: usize,
    kind: EventKind,
    note_index: usize,
}

pub(crate) fn release_frame_count(release_seconds: f32, sample_rate: u32) -> usize {
    (release_seconds * sample_rate as f32).round() as usize
}

pub(crate) fn oldest_voice_to_steal(mut released: impl Iterator<Item = bool>) -> usize {
    released.position(|released| released).unwrap_or(0)
}

pub(crate) fn allocate(
    notes: &[SynthNote],
    sample_rate: u32,
    release_seconds: f32,
    limit: u8,
) -> Vec<VoiceAllocation> {
    debug_assert!(limit > 0);
    let release_frames = release_frame_count(release_seconds, sample_rate);
    let mut allocations: Vec<_> = notes
        .iter()
        .map(|note| {
            let gate_frame = ((note.start_seconds + note.duration_seconds) * f64::from(sample_rate))
                .round() as usize;
            let release_end_frame = gate_frame + release_frames;
            VoiceAllocation {
                gate_frame,
                release_end_frame,
                audible_end_frame: release_end_frame,
            }
        })
        .collect();
    let mut events = Vec::with_capacity(notes.len() * 2);
    for (note_index, note) in notes.iter().enumerate() {
        let start_frame = (note.start_seconds * f64::from(sample_rate)).round() as usize;
        events.push(Event {
            frame: start_frame,
            kind: EventKind::On,
            note_index,
        });
        events.push(Event {
            frame: allocations[note_index].gate_frame,
            kind: EventKind::Off,
            note_index,
        });
    }
    events.sort_by_key(|event| {
        (
            event.frame,
            match event.kind {
                EventKind::Off => 0,
                EventKind::On => 1,
            },
            event.note_index,
        )
    });

    let mut active = Vec::<ActiveVoice>::new();
    for event in events {
        let note = notes[event.note_index];
        match event.kind {
            EventKind::Off => {
                for voice in active
                    .iter_mut()
                    .filter(|voice| voice.pitch == note.pitch && !voice.released)
                {
                    voice.released = true;
                    voice.release_end_frame = event.frame + release_frames;
                    let allocation = &mut allocations[voice.note_index];
                    allocation.gate_frame = event.frame;
                    allocation.release_end_frame = event.frame + release_frames;
                    allocation.audible_end_frame = event.frame + release_frames;
                }
            }
            EventKind::On => {
                active.retain(|voice| voice.release_end_frame > event.frame);
                for voice in active.iter().filter(|voice| voice.pitch == note.pitch) {
                    let allocation = &mut allocations[voice.note_index];
                    allocation.audible_end_frame = allocation.audible_end_frame.min(event.frame);
                }
                active.retain(|voice| voice.pitch != note.pitch);
                while active.len() >= usize::from(limit) {
                    let stolen = oldest_voice_to_steal(active.iter().map(|voice| voice.released));
                    let voice = active.remove(stolen);
                    let allocation = &mut allocations[voice.note_index];
                    allocation.audible_end_frame = allocation.audible_end_frame.min(event.frame);
                }
                active.push(ActiveVoice {
                    note_index: event.note_index,
                    pitch: note.pitch,
                    released: false,
                    release_end_frame: allocations[event.note_index].release_end_frame,
                });
            }
        }
    }
    allocations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start_seconds: f64, duration_seconds: f64) -> SynthNote {
        SynthNote {
            pitch,
            velocity: 100,
            start_seconds,
            duration_seconds,
        }
    }

    #[test]
    fn capacity_steals_the_oldest_voice_and_hard_cuts_its_tail() {
        let allocations = allocate(
            &[note(60, 0.0, 1.0), note(62, 0.0, 1.0), note(64, 0.5, 1.0)],
            100,
            0.25,
            2,
        );
        assert_eq!(allocations[0].audible_end_frame, 50);
        assert_eq!(allocations[0].release_end_frame, 125);
        assert_eq!(allocations[1].audible_end_frame, 125);
        assert_eq!(allocations[2].audible_end_frame, 175);
    }

    #[test]
    fn replaced_notes_note_off_releases_the_current_same_pitch_voice() {
        let allocations = allocate(&[note(60, 0.0, 1.0), note(60, 0.5, 1.0)], 100, 0.25, 2);
        assert_eq!(allocations[0].audible_end_frame, 50);
        assert_eq!(allocations[1].gate_frame, 100);
        assert_eq!(allocations[1].release_end_frame, 125);
        assert_eq!(allocations[1].audible_end_frame, 125);
    }

    #[test]
    fn released_voices_are_stolen_before_older_held_voices() {
        let allocations = allocate(
            &[note(62, 0.0, 1.0), note(60, 0.1, 0.4), note(64, 0.6, 0.25)],
            100,
            0.25,
            2,
        );
        assert_eq!(allocations[0].audible_end_frame, 125);
        assert_eq!(allocations[1].audible_end_frame, 60);
    }

    #[test]
    fn finished_voices_expire_before_note_on_victim_selection() {
        let allocations = allocate(
            &[note(60, 0.0, 0.6), note(62, 0.1, 0.4), note(64, 0.75, 0.25)],
            100,
            0.25,
            2,
        );
        assert_eq!(allocations[0].release_end_frame, 85);
        assert_eq!(allocations[1].release_end_frame, 75);
        assert_eq!(allocations[0].audible_end_frame, 85);
        assert_eq!(allocations[1].audible_end_frame, 75);
        assert_eq!(allocations[2].audible_end_frame, 125);
    }

    #[test]
    fn simultaneous_chord_uses_input_order_and_allows_zero_frame_voices() {
        let allocations = allocate(
            &[note(64, 0.0, 1.0), note(60, 0.0, 1.0), note(67, 0.0, 1.0)],
            100,
            0.25,
            2,
        );
        assert_eq!(allocations[0].audible_end_frame, 0);
        assert_eq!(allocations[1].audible_end_frame, 125);
        assert_eq!(allocations[2].audible_end_frame, 125);
    }

    #[test]
    fn release_frames_use_the_shared_rounding_rule() {
        assert_eq!(release_frame_count(0.125, 100), 13);
        assert_eq!(release_frame_count(0.0, 100), 0);
    }
}
