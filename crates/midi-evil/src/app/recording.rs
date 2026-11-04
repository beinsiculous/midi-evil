use super::*;
use crate::{
    beats::PadHit,
    keyboard::NoteEvent,
    recording::{InputEvent, Recorder, RecordingTarget},
};

impl StudioApp {
    pub(super) fn toggle_recording(&mut self) {
        if self.recorder.is_some() {
            self.stop_preview();
            return;
        }
        if self.project.tracks.is_empty() {
            self.status = "Add a track before recording".to_owned();
            return;
        }
        self.transport.repeat = true;
        self.transport.end_tick = self
            .transport
            .end_tick
            .max(self.transport.start_tick.saturating_add(1));
        let target = self.recording_target();
        let grid = self
            .record_quantize
            .then(|| (u64::from(self.project.ticks_per_quarter.max(1)) / 4).max(1));
        self.preview_project();
        self.recording_snapshot = Some(self.project.clone());
        self.recorder = Some(Recorder::new(
            target,
            self.transport.start_tick,
            self.transport.end_tick,
            grid,
        ));
        self.recording_started_at = Some(std::time::Instant::now());
        self.status = match target {
            RecordingTarget::Track(_) => {
                "Recording the unique timeline with the independent recording clock".to_owned()
            }
            RecordingTarget::Part { .. } => {
                "Recording loop overdub into the selected shared pattern".to_owned()
            }
        };
    }

    pub(super) fn record_note_events(&mut self, events: &[NoteEvent]) {
        let tick = self.recording_tick();
        if events.contains(&NoteEvent::Panic)
            && self.recorder.is_some()
            && let Some(recorder) = &mut self.recorder
        {
            recorder.panic(tick);
        }
        let events: Vec<_> = events
            .iter()
            .filter_map(|event| match *event {
                NoteEvent::On(pitch, velocity) => Some(InputEvent::NoteOn { pitch, velocity }),
                NoteEvent::Off(pitch) => Some(InputEvent::NoteOff { pitch }),
                NoteEvent::Panic => None,
            })
            .collect();
        self.record_input(&events);
    }

    pub(super) fn record_pad_hits(&mut self, hits: &[PadHit]) {
        if hits.is_empty() {
            return;
        }
        let Some(target_track) = self.recorder.as_ref().map(Recorder::target_track) else {
            return;
        };
        let Ok(instrument) = self.project.instrument_for_track(target_track) else {
            return;
        };
        let Some(base_note) = (match &instrument.source {
            InstrumentSource::DrumKit { base_note, .. } => Some(*base_note),
            _ => None,
        }) else {
            return;
        };
        let now = std::time::Instant::now();
        for hit in hits {
            let offset_seconds = hit.at.checked_duration_since(now).map_or_else(
                || -now.duration_since(hit.at).as_secs_f64(),
                |duration| duration.as_secs_f64(),
            );
            let tick = self.recording_tick_offset(offset_seconds);
            if let Some(recorder) = &mut self.recorder {
                recorder.hit(
                    base_note + hit.slot.min(15),
                    hit.velocity,
                    tick,
                    &self.project,
                );
            }
        }
    }

    pub(super) fn poll_midi_input(&mut self) {
        let events = self.midi_input.poll();
        if events.is_empty() {
            return;
        }
        self.record_input(&events);
        let Ok(instrument) = self.project.instrument_for_track(self.roll.selected_track) else {
            return;
        };
        match &instrument.source {
            InstrumentSource::Synth { .. } if matches!(self.view, View::Synth | View::Keyboard) => {
                for event in events {
                    match event {
                        InputEvent::NoteOn { pitch, velocity } if velocity > 0 => {
                            self.pending_midi_notes.push(NoteEvent::On(pitch, velocity))
                        }
                        InputEvent::NoteOn { pitch, .. } | InputEvent::NoteOff { pitch } => {
                            self.pending_midi_notes.push(NoteEvent::Off(pitch));
                        }
                        InputEvent::PitchBend { value } => {
                            self.keyboard.bend = if value < 0 {
                                f32::from(value) / 8192.0
                            } else {
                                f32::from(value) / 8191.0
                            };
                        }
                        InputEvent::Controller {
                            controller: 64,
                            value,
                        } => self.keyboard.sustain = value >= 64,
                        _ => {}
                    }
                }
            }
            InstrumentSource::DrumKit { base_note, .. } if self.view == View::Pads => {
                for event in events {
                    match event {
                        InputEvent::NoteOn { pitch, velocity }
                            if pitch >= *base_note && pitch - *base_note < 16 =>
                        {
                            self.beats.midi_input(
                                pitch - *base_note,
                                velocity > 0,
                                velocity.max(1),
                            );
                        }
                        InputEvent::NoteOff { pitch }
                            if pitch >= *base_note && pitch - *base_note < 16 =>
                        {
                            self.beats.midi_input(pitch - *base_note, false, 0);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn finish_recording(&mut self) {
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        self.recording_snapshot = None;
        let tick = self.recording_tick();
        self.recording_started_at = None;
        match recorder.finish(tick, &mut self.project) {
            Ok(take) => {
                self.status = format!(
                    "Recorded {} notes and {} controller events; Undo removes the take",
                    take.notes, take.controllers
                );
            }
            Err(error) => self.status = format!("Could not commit recording: {error}"),
        }
    }

    fn record_input(&mut self, events: &[InputEvent]) {
        if events.is_empty() {
            return;
        }
        let tick = self.recording_tick();
        if let Some(recorder) = &mut self.recorder {
            for &event in events {
                recorder.input(event, tick, &self.project);
            }
        }
    }

    fn recording_target(&self) -> RecordingTarget {
        let track = self.roll.selected_track;
        self.roll
            .selected_part
            .filter(|part| self.project.tracks[track].parts.get(*part).is_some())
            .map_or(RecordingTarget::Track(track), |part| {
                RecordingTarget::Part { track, part }
            })
    }
}
