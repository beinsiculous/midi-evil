use std::collections::BTreeMap;

use evil_project::{
    Aftertouch, ControllerEvent, EventSequence, Note, Pattern, PitchBend, Project, Track,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingTarget {
    Track(usize),
    Part { track: usize, part: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
    PitchBend { value: i16 },
    Aftertouch { note: Option<u8>, value: u8 },
    Controller { controller: u8, value: u8 },
}

#[derive(Clone, Copy, Debug)]
struct ActiveNote {
    pitch: u8,
    velocity: u8,
    timeline_start: u64,
    local_start: u64,
}

/// Accumulates one take and commits it atomically when recording stops.
/// Timeline positions are converted to pattern-local positions on input, so
/// repeated sections overdub the same reusable pattern on every pass.
pub struct Recorder {
    target: RecordingTarget,
    start_tick: u64,
    end_tick: u64,
    quantize: Option<u64>,
    active: BTreeMap<u8, ActiveNote>,
    notes: Vec<Note>,
    pitch_bends: Vec<PitchBend>,
    aftertouch: Vec<Aftertouch>,
    controllers: Vec<ControllerEvent>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Take {
    pub notes: usize,
    pub controllers: usize,
}

impl Recorder {
    pub fn new(
        target: RecordingTarget,
        start_tick: u64,
        end_tick: u64,
        quantize: Option<u64>,
    ) -> Self {
        Self {
            target,
            start_tick,
            end_tick: end_tick.max(start_tick.saturating_add(1)),
            quantize: quantize.filter(|grid| *grid > 0),
            active: BTreeMap::new(),
            notes: Vec::new(),
            pitch_bends: Vec::new(),
            aftertouch: Vec::new(),
            controllers: Vec::new(),
        }
    }

    pub fn target_track(&self) -> usize {
        match self.target {
            RecordingTarget::Track(track) | RecordingTarget::Part { track, .. } => track,
        }
    }

    pub fn pending_events(&self) -> usize {
        self.notes.len()
            + self.active.len()
            + self.pitch_bends.len()
            + self.aftertouch.len()
            + self.controllers.len()
    }

    pub fn input(&mut self, event: InputEvent, timeline_tick: u64, project: &Project) {
        match event {
            InputEvent::NoteOn { pitch, velocity } if velocity > 0 => {
                self.note_on(pitch, velocity, timeline_tick, project);
            }
            InputEvent::NoteOn { pitch, .. } | InputEvent::NoteOff { pitch } => {
                self.note_off(pitch, timeline_tick);
            }
            InputEvent::PitchBend { value } => {
                if let Some(tick) = self.local_tick(timeline_tick, project) {
                    self.pitch_bends.push(PitchBend { tick, value });
                }
            }
            InputEvent::Aftertouch { note, value } => {
                if let Some(tick) = self.local_tick(timeline_tick, project) {
                    self.aftertouch.push(Aftertouch { tick, note, value });
                }
            }
            InputEvent::Controller { controller, value } => {
                if let Some(tick) = self.local_tick(timeline_tick, project) {
                    self.controllers.push(ControllerEvent {
                        tick,
                        controller,
                        value,
                    });
                }
            }
        }
    }

    /// Record a one-shot pad hit with a grid-length gate.
    pub fn hit(&mut self, pitch: u8, velocity: u8, timeline_tick: u64, project: &Project) {
        let Some(local_start) = self.local_tick(timeline_tick, project) else {
            return;
        };
        let duration_ticks = self
            .quantize
            .unwrap_or_else(|| u64::from(project.ticks_per_quarter.max(1)) / 4)
            .max(1);
        self.notes.push(Note {
            pitch,
            velocity: velocity.max(1),
            start_tick: local_start,
            duration_ticks,
        });
    }

    pub fn panic(&mut self, timeline_tick: u64) {
        let active: Vec<_> = std::mem::take(&mut self.active).into_values().collect();
        for note in active {
            self.close_note(note, timeline_tick);
        }
    }

    pub fn finish(mut self, timeline_tick: u64, project: &mut Project) -> Result<Take, String> {
        self.panic(timeline_tick);
        let take = Take {
            notes: self.notes.len(),
            controllers: self.pitch_bends.len() + self.aftertouch.len() + self.controllers.len(),
        };
        let mut updated = project.clone();
        let sequence = sequence_mut(&mut updated, self.target)?;
        sequence.notes_mut().extend(self.notes);
        sequence.pitch_bends_mut().extend(self.pitch_bends);
        sequence.aftertouch_mut().extend(self.aftertouch);
        sequence.controllers_mut().extend(self.controllers);
        sequence.notes_mut().sort_by_key(|note| note.start_tick);
        sequence.pitch_bends_mut().sort_by_key(|event| event.tick);
        sequence.aftertouch_mut().sort_by_key(|event| event.tick);
        sequence.controllers_mut().sort_by_key(|event| event.tick);
        updated.validate().map_err(|error| error.to_string())?;
        *project = updated;
        Ok(take)
    }

    fn note_on(&mut self, pitch: u8, velocity: u8, timeline_tick: u64, project: &Project) {
        if let Some(previous) = self.active.remove(&pitch) {
            self.close_note(previous, timeline_tick);
        }
        let Some(local_start) = self.local_tick(timeline_tick, project) else {
            return;
        };
        self.active.insert(
            pitch,
            ActiveNote {
                pitch,
                velocity: velocity.max(1),
                timeline_start: timeline_tick,
                local_start,
            },
        );
    }

    fn note_off(&mut self, pitch: u8, timeline_tick: u64) {
        if let Some(note) = self.active.remove(&pitch) {
            self.close_note(note, timeline_tick);
        }
    }

    fn close_note(&mut self, note: ActiveNote, timeline_tick: u64) {
        let timeline_end = if timeline_tick >= note.timeline_start {
            timeline_tick.min(self.end_tick)
        } else {
            self.end_tick
        };
        let mut duration_ticks = timeline_end.saturating_sub(note.timeline_start).max(1);
        if let Some(grid) = self.quantize {
            duration_ticks = ((duration_ticks.saturating_add(grid / 2)) / grid).max(1) * grid;
        }
        self.notes.push(Note {
            pitch: note.pitch,
            velocity: note.velocity,
            start_tick: note.local_start,
            duration_ticks,
        });
    }

    fn local_tick(&self, timeline_tick: u64, project: &Project) -> Option<u64> {
        if timeline_tick < self.start_tick || timeline_tick >= self.end_tick {
            return None;
        }
        let tick = match self.target {
            RecordingTarget::Track(_) => timeline_tick,
            RecordingTarget::Part { track, part } => {
                let part = project.tracks.get(track)?.parts.get(part)?;
                let part_end = part.start_tick.checked_add(part.length_ticks)?;
                if timeline_tick < part.start_tick || timeline_tick >= part_end {
                    return None;
                }
                let pattern = project.patterns.get(part.pattern)?;
                (timeline_tick - part.start_tick) % pattern.length_ticks
            }
        };
        Some(match self.quantize {
            Some(grid) => tick / grid * grid,
            None => tick,
        })
    }
}

/// Convert elapsed recording time to song ticks even when the project has no
/// notes and therefore cannot produce an audio-preview clock.
pub fn recording_tick_at_elapsed(
    project: &Project,
    start_tick: u64,
    end_tick: Option<u64>,
    elapsed: f64,
) -> Result<u64, String> {
    let performance = project
        .to_performance()
        .map_err(|error| error.to_string())?;
    let start_seconds = performance.seconds_at(start_tick);
    if let Some(end_tick) = end_tick {
        let duration = (performance.seconds_at(end_tick) - start_seconds).max(f64::EPSILON);
        Ok(performance.tick_at_seconds(start_seconds + elapsed.max(0.0).rem_euclid(duration)))
    } else {
        Ok(performance.tick_at_seconds(start_seconds + elapsed.max(0.0)))
    }
}

fn sequence_mut(
    project: &mut Project,
    target: RecordingTarget,
) -> Result<&mut dyn EventSequence, String> {
    match target {
        RecordingTarget::Track(track) => project
            .tracks
            .get_mut(track)
            .map(|track| track as &mut Track as &mut dyn EventSequence)
            .ok_or_else(|| "The recorded track no longer exists".to_owned()),
        RecordingTarget::Part { track, part } => {
            let part = project
                .tracks
                .get(track)
                .and_then(|track| track.parts.get(part))
                .copied()
                .ok_or_else(|| "The recorded arrangement part no longer exists".to_owned())?;
            project
                .patterns
                .get_mut(part.pattern)
                .map(|pattern| pattern as &mut Pattern as &mut dyn EventSequence)
                .ok_or_else(|| "The recorded pattern no longer exists".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::{Part, Track};

    fn project() -> Project {
        let mut project = Project::new("Recording");
        project.add_track(Track::default()).unwrap();
        project
    }

    #[test]
    fn keyboard_take_quantizes_and_preserves_controllers() {
        let mut project = project();
        let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 1920, Some(120));
        recorder.input(
            InputEvent::NoteOn {
                pitch: 60,
                velocity: 100,
            },
            115,
            &project,
        );
        recorder.input(
            InputEvent::Controller {
                controller: 1,
                value: 90,
            },
            121,
            &project,
        );
        recorder.input(InputEvent::NoteOff { pitch: 60 }, 350, &project);
        let take = recorder.finish(350, &mut project).unwrap();
        assert_eq!(
            take,
            Take {
                notes: 1,
                controllers: 1
            }
        );
        assert_eq!(project.tracks[0].notes[0].start_tick, 0);
        assert_eq!(project.tracks[0].notes[0].duration_ticks, 240);
        assert_eq!(project.tracks[0].controllers[0].tick, 120);
    }

    #[test]
    fn loop_overdub_maps_repeated_hits_into_one_shared_pattern() {
        let mut project = project();
        let pattern = project
            .add_pattern(Pattern {
                length_ticks: 480,
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 0,
                    length_ticks: 1920,
                },
            )
            .unwrap();
        let mut recorder = Recorder::new(
            RecordingTarget::Part { track: 0, part: 0 },
            0,
            1920,
            Some(120),
        );
        recorder.hit(36, 100, 110, &project);
        recorder.hit(38, 110, 600, &project);
        recorder.finish(600, &mut project).unwrap();
        assert_eq!(project.patterns[0].notes.len(), 2);
        assert_eq!(project.patterns[0].notes[0].start_tick, 0);
        assert_eq!(project.patterns[0].notes[1].start_tick, 120);
        assert_eq!(project.tracks[0].notes.len(), 0);
    }

    #[test]
    fn same_frame_release_and_panic_close_notes_at_the_input_position() {
        let mut project = project();
        let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 480, None);
        recorder.input(
            InputEvent::NoteOn {
                pitch: 60,
                velocity: 100,
            },
            120,
            &project,
        );
        recorder.input(InputEvent::NoteOff { pitch: 60 }, 120, &project);
        recorder.input(
            InputEvent::NoteOn {
                pitch: 64,
                velocity: 90,
            },
            120,
            &project,
        );
        recorder.panic(240);
        recorder.finish(240, &mut project).unwrap();
        assert_eq!(project.tracks[0].notes[0].duration_ticks, 1);
        assert_eq!(project.tracks[0].notes[1].duration_ticks, 120);
    }

    #[test]
    fn recording_clock_works_without_notes_or_a_preview() {
        let project = project();
        assert!(project.tracks[0].notes.is_empty());
        assert_eq!(
            recording_tick_at_elapsed(&project, 0, Some(1_920), 1.0).unwrap(),
            960
        );
        assert_eq!(
            recording_tick_at_elapsed(&project, 0, Some(1_920), 2.0).unwrap(),
            0
        );
        assert_eq!(
            recording_tick_at_elapsed(&project, 0, Some(1_920), -0.1).unwrap(),
            0
        );
    }

    #[test]
    fn loop_overdub_restores_chronological_controller_order() {
        let mut project = project();
        project.tracks[0].controllers.push(ControllerEvent {
            tick: 1_200,
            controller: 1,
            value: 80,
        });
        let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 1_920, None);
        recorder.input(
            InputEvent::Controller {
                controller: 1,
                value: 20,
            },
            100,
            &project,
        );
        recorder.input(
            InputEvent::Controller {
                controller: 1,
                value: 40,
            },
            600,
            &project,
        );
        recorder.finish(600, &mut project).unwrap();
        assert_eq!(
            project.tracks[0]
                .controllers
                .iter()
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [100, 600, 1_200]
        );
    }

    #[test]
    fn stopping_closes_held_notes_at_the_loop_boundary() {
        let mut project = project();
        let mut recorder = Recorder::new(RecordingTarget::Track(0), 0, 480, None);
        recorder.input(
            InputEvent::NoteOn {
                pitch: 64,
                velocity: 90,
            },
            400,
            &project,
        );
        recorder.finish(120, &mut project).unwrap();
        assert_eq!(project.tracks[0].notes[0].duration_ticks, 80);
    }
}
