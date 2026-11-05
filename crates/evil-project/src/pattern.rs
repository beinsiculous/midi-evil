use serde::{Deserialize, Serialize};

use crate::{
    Aftertouch, ControllerEvent, Error, MixerAutomationEvent, Note, PitchBend, Project, invalid,
    validate_note,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pattern {
    pub name: String,
    pub length_ticks: u64,
    pub notes: Vec<Note>,
    pub pitch_bends: Vec<PitchBend>,
    pub aftertouch: Vec<Aftertouch>,
    pub controllers: Vec<ControllerEvent>,
    pub mixer_automation: Vec<MixerAutomationEvent>,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            name: "Pattern".to_owned(),
            length_ticks: 1_920,
            notes: Vec::new(),
            pitch_bends: Vec::new(),
            aftertouch: Vec::new(),
            controllers: Vec::new(),
            mixer_automation: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    pub pattern: usize,
    pub start_tick: u64,
    pub length_ticks: u64,
}

impl Pattern {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        validate_events(
            self.length_ticks,
            &self.notes,
            &self.pitch_bends,
            &self.aftertouch,
            &self.controllers,
            &self.mixer_automation,
        )?;
        if self
            .notes
            .iter()
            .any(|note| note.start_tick >= self.length_ticks)
            || self
                .pitch_bends
                .iter()
                .any(|event| event.tick >= self.length_ticks)
            || self
                .aftertouch
                .iter()
                .any(|event| event.tick >= self.length_ticks)
            || self
                .controllers
                .iter()
                .any(|event| event.tick >= self.length_ticks)
            || self
                .mixer_automation
                .iter()
                .any(|event| event.tick >= self.length_ticks)
        {
            return Err(invalid("pattern events must start inside the pattern"));
        }
        Ok(())
    }
}

pub(crate) fn validate_events(
    length_ticks: u64,
    notes: &[Note],
    pitch_bends: &[PitchBend],
    aftertouch: &[Aftertouch],
    controllers: &[ControllerEvent],
    mixer_automation: &[MixerAutomationEvent],
) -> Result<(), Error> {
    if length_ticks == 0 {
        return Err(invalid("pattern length must be greater than zero"));
    }
    for note in notes {
        validate_note(note)?;
    }
    for bend in pitch_bends {
        if !(-8192..=8191).contains(&bend.value) {
            return Err(invalid("pitch bend values must be between -8192 and 8191"));
        }
    }
    for pressure in aftertouch {
        if pressure.value > 127 || pressure.note.is_some_and(|note| note > 127) {
            return Err(invalid(
                "aftertouch notes and values must be between 0 and 127",
            ));
        }
    }
    for event in controllers {
        if event.controller > 127 || event.value > 127 {
            return Err(invalid(
                "controller numbers and values must be between 0 and 127",
            ));
        }
    }
    if mixer_automation.iter().any(|event| !event.is_valid()) {
        return Err(invalid(
            "mixer automation has an invalid target, non-finite value, or out-of-range value",
        ));
    }
    Ok(())
}

impl Project {
    pub fn move_part(
        &mut self,
        track_index: usize,
        part_index: usize,
        start_tick: u64,
    ) -> Result<(), Error> {
        let mut part = self.part(track_index, part_index)?;
        part.start_tick = start_tick;
        self.validate_part(part)?;
        self.tracks[track_index].parts[part_index] = part;
        Ok(())
    }

    pub fn resize_part(
        &mut self,
        track_index: usize,
        part_index: usize,
        length_ticks: u64,
    ) -> Result<(), Error> {
        let mut part = self.part(track_index, part_index)?;
        part.length_ticks = length_ticks;
        self.validate_part(part)?;
        self.tracks[track_index].parts[part_index] = part;
        Ok(())
    }

    pub fn duplicate_part(
        &mut self,
        track_index: usize,
        part_index: usize,
    ) -> Result<usize, Error> {
        let mut part = self.part(track_index, part_index)?;
        part.start_tick = part
            .start_tick
            .checked_add(part.length_ticks)
            .ok_or_else(|| invalid("duplicated part start exceeds the supported range"))?;
        self.validate_part(part)?;
        let new_index = part_index + 1;
        self.tracks[track_index].parts.insert(new_index, part);
        Ok(new_index)
    }

    /// Clone a part's pattern so edits no longer affect other arrangement parts.
    pub fn make_part_unique(
        &mut self,
        track_index: usize,
        part_index: usize,
    ) -> Result<usize, Error> {
        let part = self.part(track_index, part_index)?;
        let mut pattern = self.patterns[part.pattern].clone();
        pattern.name = format!("{} Unique", pattern.name);
        let pattern_index = self.add_pattern(pattern)?;
        self.tracks[track_index].parts[part_index].pattern = pattern_index;
        Ok(pattern_index)
    }

    pub fn remove_part(&mut self, track_index: usize, part_index: usize) -> Result<Part, Error> {
        self.part(track_index, part_index)?;
        Ok(self.tracks[track_index].parts.remove(part_index))
    }

    /// Expand one arrangement part into the track's unique timeline performance.
    pub fn merge_part_into_track(
        &mut self,
        track_index: usize,
        part_index: usize,
    ) -> Result<(), Error> {
        let part = self.part(track_index, part_index)?;
        let pattern = &self.patterns[part.pattern];
        let track = self
            .tracks
            .get_mut(track_index)
            .ok_or_else(|| invalid("track does not exist"))?;
        expand_part(
            pattern,
            part,
            &mut track.notes,
            &mut track.pitch_bends,
            &mut track.aftertouch,
            &mut track.controllers,
            &mut track.mixer_automation,
        );
        track.parts.remove(part_index);
        Ok(())
    }

    /// Copy unique events and parts starting in a section immediately after it.
    /// Linked parts keep sharing their original patterns; partial parts are cropped.
    pub fn duplicate_section(&mut self, start_tick: u64, end_tick: u64) -> Result<usize, Error> {
        if end_tick <= start_tick {
            return Err(invalid("section end must be after its start"));
        }
        let length = end_tick - start_tick;
        end_tick
            .checked_add(length)
            .ok_or_else(|| invalid("duplicated section exceeds the supported range"))?;
        let mut updated = self.clone();
        let mut copied = 0;
        for track in &mut updated.tracks {
            let notes: Vec<_> = track
                .notes
                .iter()
                .filter(|note| (start_tick..end_tick).contains(&note.start_tick))
                .map(|note| Note {
                    start_tick: note.start_tick + length,
                    ..*note
                })
                .collect();
            copied += notes.len();
            track.notes.extend(notes);
            let bends: Vec<_> = track
                .pitch_bends
                .iter()
                .filter(|event| (start_tick..end_tick).contains(&event.tick))
                .map(|event| PitchBend {
                    tick: event.tick + length,
                    ..*event
                })
                .collect();
            copied += bends.len();
            track.pitch_bends.extend(bends);
            let pressure: Vec<_> = track
                .aftertouch
                .iter()
                .filter(|event| (start_tick..end_tick).contains(&event.tick))
                .map(|event| Aftertouch {
                    tick: event.tick + length,
                    ..*event
                })
                .collect();
            copied += pressure.len();
            track.aftertouch.extend(pressure);
            let controllers: Vec<_> = track
                .controllers
                .iter()
                .filter(|event| (start_tick..end_tick).contains(&event.tick))
                .map(|event| ControllerEvent {
                    tick: event.tick + length,
                    ..*event
                })
                .collect();
            copied += controllers.len();
            track.controllers.extend(controllers);
            let mixer_automation: Vec<_> = track
                .mixer_automation
                .iter()
                .filter(|event| (start_tick..end_tick).contains(&event.tick))
                .map(|event| MixerAutomationEvent {
                    tick: event.tick + length,
                    ..*event
                })
                .collect();
            copied += mixer_automation.len();
            track.mixer_automation.extend(mixer_automation);
            let parts: Vec<_> = track
                .parts
                .iter()
                .filter(|part| (start_tick..end_tick).contains(&part.start_tick))
                .map(|part| Part {
                    start_tick: part.start_tick + length,
                    length_ticks: part.length_ticks.min(end_tick - part.start_tick),
                    ..*part
                })
                .collect();
            copied += parts.len();
            track.parts.extend(parts);
        }
        updated.validate()?;
        *self = updated;
        Ok(copied)
    }

    fn part(&self, track_index: usize, part_index: usize) -> Result<Part, Error> {
        self.tracks
            .get(track_index)
            .and_then(|track| track.parts.get(part_index))
            .copied()
            .ok_or_else(|| invalid("arrangement part does not exist"))
    }
}

pub(crate) fn expand_part(
    pattern: &Pattern,
    part: Part,
    notes: &mut Vec<Note>,
    pitch_bends: &mut Vec<PitchBend>,
    aftertouch: &mut Vec<Aftertouch>,
    controllers: &mut Vec<ControllerEvent>,
    mixer_automation: &mut Vec<MixerAutomationEvent>,
) {
    let part_end = part.start_tick + part.length_ticks;
    for occurrence_start in (part.start_tick..part_end).step_by(pattern.length_ticks as usize) {
        for note in &pattern.notes {
            let start_tick = occurrence_start + note.start_tick;
            if start_tick < part_end {
                notes.push(Note {
                    start_tick,
                    ..*note
                });
            }
        }
        for bend in &pattern.pitch_bends {
            let tick = occurrence_start + bend.tick;
            if tick < part_end {
                pitch_bends.push(PitchBend { tick, ..*bend });
            }
        }
        for pressure in &pattern.aftertouch {
            let tick = occurrence_start + pressure.tick;
            if tick < part_end {
                aftertouch.push(Aftertouch { tick, ..*pressure });
            }
        }
        for event in &pattern.controllers {
            let tick = occurrence_start + event.tick;
            if tick < part_end {
                controllers.push(ControllerEvent { tick, ..*event });
            }
        }
        for event in &pattern.mixer_automation {
            let tick = occurrence_start + event.tick;
            if tick < part_end {
                mixer_automation.push(MixerAutomationEvent { tick, ..*event });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Track;

    fn project_with_part() -> Project {
        let mut project = Project::new("Parts");
        let pattern = project
            .add_pattern(Pattern {
                name: "Shared".into(),
                length_ticks: 480,
                notes: vec![Note {
                    start_tick: 120,
                    duration_ticks: 60,
                    ..Note::default()
                }],
                mixer_automation: vec![MixerAutomationEvent {
                    tick: 120,
                    parameter: crate::MixerParameter::Gain,
                    value: 0.5,
                }],
                ..Pattern::default()
            })
            .unwrap();
        project.add_track(Track::default()).unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 480,
                    length_ticks: 960,
                },
            )
            .unwrap();
        project
    }

    #[test]
    fn duplicate_move_and_crop_preserve_shared_pattern() {
        let mut project = project_with_part();
        let duplicate = project.duplicate_part(0, 0).unwrap();
        assert_eq!(duplicate, 1);
        assert_eq!(project.tracks[0].parts[1].start_tick, 1440);
        assert_eq!(project.tracks[0].parts[1].pattern, 0);
        project.move_part(0, 1, 1920).unwrap();
        project.resize_part(0, 1, 240).unwrap();
        assert_eq!(project.tracks[0].parts[1].length_ticks, 240);
        assert!(project.resize_part(0, 1, 0).is_err());
    }

    #[test]
    fn make_unique_detaches_only_the_selected_part() {
        let mut project = project_with_part();
        project.duplicate_part(0, 0).unwrap();
        let unique = project.make_part_unique(0, 0).unwrap();
        project.patterns[unique].notes[0].pitch = 72;
        assert_eq!(project.patterns[0].notes[0].pitch, 60);
        assert_eq!(project.tracks[0].parts[1].pattern, 0);
    }

    #[test]
    fn merge_expands_repetitions_and_removes_the_part() {
        let mut project = project_with_part();
        project.merge_part_into_track(0, 0).unwrap();
        assert!(project.tracks[0].parts.is_empty());
        assert_eq!(
            project.tracks[0]
                .notes
                .iter()
                .map(|note| note.start_tick)
                .collect::<Vec<_>>(),
            [600, 1080]
        );
        assert_eq!(
            project.tracks[0]
                .mixer_automation
                .iter()
                .map(|event| event.tick)
                .collect::<Vec<_>>(),
            [600, 1080]
        );
        assert_eq!(project.patterns.len(), 1);
    }

    #[test]
    fn duplicate_section_copies_events_and_crops_linked_parts() {
        let mut project = project_with_part();
        project.tracks[0].notes.push(Note {
            start_tick: 480,
            ..Note::default()
        });
        let copied = project.duplicate_section(0, 960).unwrap();
        assert_eq!(copied, 2);
        assert_eq!(project.tracks[0].notes[1].start_tick, 1440);
        assert_eq!(project.tracks[0].parts[1].start_tick, 1440);
        assert_eq!(project.tracks[0].parts[1].length_ticks, 480);
        assert_eq!(project.tracks[0].parts[1].pattern, 0);
    }
}
