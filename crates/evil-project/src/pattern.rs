use std::collections::BTreeSet;

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

    /// Copy events from `source_pattern` that start within `target_pattern`'s length.
    pub fn copy_pattern_events(
        &mut self,
        source_pattern: usize,
        target_pattern: usize,
    ) -> Result<usize, Error> {
        if source_pattern >= self.patterns.len() || target_pattern >= self.patterns.len() {
            return Err(invalid("pattern does not exist"));
        }
        if source_pattern == target_pattern {
            return Err(invalid("source and target patterns must differ"));
        }
        let mut updated = self.clone();
        let source = self.patterns[source_pattern].clone();
        let target = &mut updated.patterns[target_pattern];
        let target_length = target.length_ticks;
        let mut copied = 0;

        let notes: Vec<_> = source
            .notes
            .iter()
            .filter(|note| note.start_tick < target_length)
            .copied()
            .collect();
        copied += notes.len();
        target.notes.extend(notes);

        let bends: Vec<_> = source
            .pitch_bends
            .iter()
            .filter(|event| event.tick < target_length)
            .copied()
            .collect();
        copied += bends.len();
        target.pitch_bends.extend(bends);

        let pressure: Vec<_> = source
            .aftertouch
            .iter()
            .filter(|event| event.tick < target_length)
            .copied()
            .collect();
        copied += pressure.len();
        target.aftertouch.extend(pressure);

        let controllers: Vec<_> = source
            .controllers
            .iter()
            .filter(|event| event.tick < target_length)
            .copied()
            .collect();
        copied += controllers.len();
        target.controllers.extend(controllers);

        let mixer_automation: Vec<_> = source
            .mixer_automation
            .iter()
            .filter(|event| event.tick < target_length)
            .copied()
            .collect();
        copied += mixer_automation.len();
        target.mixer_automation.extend(mixer_automation);

        updated.validate()?;
        *self = updated;
        Ok(copied)
    }

    /// Remove patterns not referenced by any arrangement part and remap part indices.
    pub fn remove_unused_patterns(&mut self) -> usize {
        let mut used = vec![false; self.patterns.len()];
        for track in &self.tracks {
            for part in &track.parts {
                if let Some(flag) = used.get_mut(part.pattern) {
                    *flag = true;
                }
            }
        }
        let original_count = self.patterns.len();
        let mut remap = vec![None; original_count];
        let mut retained = Vec::new();
        for (old_index, pattern) in self.patterns.drain(..).enumerate() {
            if used[old_index] {
                remap[old_index] = Some(retained.len());
                retained.push(pattern);
            }
        }
        self.patterns = retained;
        for track in &mut self.tracks {
            for part in &mut track.parts {
                if let Some(new_index) = remap.get(part.pattern).copied().flatten() {
                    part.pattern = new_index;
                }
            }
        }
        original_count - self.patterns.len()
    }

    /// Split an arrangement part at `split_tick`, keeping the rendered performance identical.
    pub fn split_part(
        &mut self,
        track_index: usize,
        part_index: usize,
        split_tick: u64,
    ) -> Result<(usize, usize), Error> {
        let part = self.part(track_index, part_index)?;
        let part_end = part
            .start_tick
            .checked_add(part.length_ticks)
            .ok_or_else(|| invalid("part end tick exceeds the supported range"))?;
        if split_tick <= part.start_tick || split_tick >= part_end {
            return Err(invalid(
                "split tick must fall strictly inside the arrangement part",
            ));
        }

        let mut updated = self.clone();
        let left_length = split_tick - part.start_tick;
        let right_length = part_end - split_tick;
        let pattern = &self.patterns[part.pattern];
        let right_pattern = if left_length.is_multiple_of(pattern.length_ticks) {
            part.pattern
        } else {
            let mut notes = Vec::new();
            let mut pitch_bends = Vec::new();
            let mut aftertouch = Vec::new();
            let mut controllers = Vec::new();
            let mut mixer_automation = Vec::new();
            expand_part(
                pattern,
                part,
                &mut notes,
                &mut pitch_bends,
                &mut aftertouch,
                &mut controllers,
                &mut mixer_automation,
            );
            notes.retain(|note| note.start_tick >= split_tick);
            for note in &mut notes {
                note.start_tick -= split_tick;
            }
            pitch_bends.retain(|event| event.tick >= split_tick);
            for event in &mut pitch_bends {
                event.tick -= split_tick;
            }
            aftertouch.retain(|event| event.tick >= split_tick);
            for event in &mut aftertouch {
                event.tick -= split_tick;
            }
            controllers.retain(|event| event.tick >= split_tick);
            for event in &mut controllers {
                event.tick -= split_tick;
            }
            mixer_automation.retain(|event| event.tick >= split_tick);
            for event in &mut mixer_automation {
                event.tick -= split_tick;
            }
            updated.add_pattern(Pattern {
                name: format!("{} Split", pattern.name),
                length_ticks: right_length,
                notes,
                pitch_bends,
                aftertouch,
                controllers,
                mixer_automation,
            })?
        };

        updated.tracks[track_index].parts[part_index].length_ticks = left_length;
        updated.tracks[track_index].parts.insert(
            part_index + 1,
            Part {
                pattern: right_pattern,
                start_tick: split_tick,
                length_ticks: right_length,
            },
        );
        updated.validate()?;
        *self = updated;
        Ok((part_index, part_index + 1))
    }

    /// Join two arrangement parts on the same track into one new pattern without altering other parts.
    pub fn join_parts(
        &mut self,
        track_index: usize,
        first_part_index: usize,
        second_part_index: usize,
    ) -> Result<usize, Error> {
        if first_part_index == second_part_index {
            return Err(invalid("joining requires two distinct arrangement parts"));
        }
        let first = self.part(track_index, first_part_index)?;
        let second = self.part(track_index, second_part_index)?;
        let (earlier, later) =
            if (first.start_tick, first_part_index) <= (second.start_tick, second_part_index) {
                (first, second)
            } else {
                (second, first)
            };
        let earlier_end = earlier
            .start_tick
            .checked_add(earlier.length_ticks)
            .ok_or_else(|| invalid("part end tick exceeds the supported range"))?;
        let later_end = later
            .start_tick
            .checked_add(later.length_ticks)
            .ok_or_else(|| invalid("part end tick exceeds the supported range"))?;
        let joined_start = earlier.start_tick;
        let joined_length = earlier_end.max(later_end) - joined_start;

        let mut notes = Vec::new();
        let mut pitch_bends = Vec::new();
        let mut aftertouch = Vec::new();
        let mut controllers = Vec::new();
        let mut mixer_automation = Vec::new();
        for part in [earlier, later] {
            expand_part(
                &self.patterns[part.pattern],
                part,
                &mut notes,
                &mut pitch_bends,
                &mut aftertouch,
                &mut controllers,
                &mut mixer_automation,
            );
        }
        for note in &mut notes {
            note.start_tick -= joined_start;
        }
        for event in &mut pitch_bends {
            event.tick -= joined_start;
        }
        for event in &mut aftertouch {
            event.tick -= joined_start;
        }
        for event in &mut controllers {
            event.tick -= joined_start;
        }
        for event in &mut mixer_automation {
            event.tick -= joined_start;
        }
        notes.sort_by_key(|note| (note.start_tick, note.pitch));
        pitch_bends.sort_by_key(|event| event.tick);
        aftertouch.sort_by_key(|event| event.tick);
        controllers.sort_by_key(|event| event.tick);
        mixer_automation.sort_by_key(|event| event.tick);

        let mut updated = self.clone();
        let joined_pattern = updated.add_pattern(Pattern {
            name: format!(
                "{} + {}",
                self.patterns[earlier.pattern].name, self.patterns[later.pattern].name
            ),
            length_ticks: joined_length,
            notes,
            pitch_bends,
            aftertouch,
            controllers,
            mixer_automation,
        })?;
        let keep_index = first_part_index.min(second_part_index);
        let drop_index = first_part_index.max(second_part_index);
        updated.tracks[track_index].parts[keep_index] = Part {
            pattern: joined_pattern,
            start_tick: joined_start,
            length_ticks: joined_length,
        };
        updated.tracks[track_index].parts.remove(drop_index);
        updated.validate()?;
        *self = updated;
        Ok(keep_index)
    }

    /// Move one arrangement part onto another track (or the same track) at `start_tick`.
    pub fn move_part_to_track(
        &mut self,
        source_track_index: usize,
        part_index: usize,
        target_track_index: usize,
        start_tick: u64,
    ) -> Result<usize, Error> {
        if target_track_index >= self.tracks.len() {
            return Err(invalid("target track does not exist"));
        }
        let mut part = self.part(source_track_index, part_index)?;
        part.start_tick = start_tick;
        self.validate_part(part)?;
        if source_track_index == target_track_index {
            self.tracks[source_track_index].parts[part_index] = part;
            return Ok(part_index);
        }
        self.tracks[source_track_index].parts.remove(part_index);
        let new_index = self.tracks[target_track_index].parts.len();
        self.tracks[target_track_index].parts.push(part);
        Ok(new_index)
    }

    /// Atomically move multiple selected `(track_index, part_index)` parts across tracks/time.
    /// Returns the new `(track_index, part_index)` coordinates in original selection order.
    pub fn move_parts(
        &mut self,
        selections: &[(usize, usize)],
        track_delta: isize,
        tick_delta: i64,
    ) -> Result<Vec<(usize, usize)>, Error> {
        if selections.is_empty() {
            return Err(invalid("select at least one arrangement part"));
        }
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(selections.len());
        for &(track_index, part_index) in selections {
            if !seen.insert((track_index, part_index)) {
                return Err(invalid("part selection contains duplicate entries"));
            }
            let mut part = self.part(track_index, part_index)?;
            let target_track = isize::try_from(track_index)
                .ok()
                .and_then(|value| value.checked_add(track_delta))
                .and_then(|value| usize::try_from(value).ok())
                .filter(|&target| target < self.tracks.len())
                .ok_or_else(|| invalid("target track is outside the track list"))?;
            let target_tick = i128::from(part.start_tick) + i128::from(tick_delta);
            if target_tick < 0 || target_tick > i128::from(u64::MAX) {
                return Err(invalid(
                    "target part start tick is outside the supported range",
                ));
            }
            part.start_tick = target_tick as u64;
            self.validate_part(part)?;
            planned.push((track_index, part_index, target_track, part));
        }

        let mut updated = self.clone();
        let mut removals: Vec<(usize, usize)> = selections.to_vec();
        removals.sort_unstable_by(|left, right| right.cmp(left));
        for (track_index, part_index) in removals {
            updated.tracks[track_index].parts.remove(part_index);
        }

        let mut moved_positions = Vec::with_capacity(planned.len());
        for (_, _, target_track, part) in planned {
            let new_index = updated.tracks[target_track].parts.len();
            updated.tracks[target_track].parts.push(part);
            moved_positions.push((target_track, new_index));
        }
        updated.validate()?;
        *self = updated;
        Ok(moved_positions)
    }

    /// Return the bounding `[min_start, max_end)` tick range covering the selected parts.
    pub fn parts_tick_range(&self, selections: &[(usize, usize)]) -> Result<(u64, u64), Error> {
        if selections.is_empty() {
            return Err(invalid("select at least one arrangement part"));
        }
        let mut min_start = u64::MAX;
        let mut max_end = 0_u64;
        for &(track_index, part_index) in selections {
            let part = self.part(track_index, part_index)?;
            let end_tick = part
                .start_tick
                .checked_add(part.length_ticks)
                .ok_or_else(|| invalid("part end tick exceeds the supported range"))?;
            min_start = min_start.min(part.start_tick);
            max_end = max_end.max(end_tick);
        }
        Ok((min_start, max_end))
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

    #[test]
    fn copy_pattern_events_and_remove_unused_patterns_preserve_references() {
        let mut project = project_with_part();
        let source = project
            .add_pattern(Pattern {
                name: "Source".into(),
                length_ticks: 960,
                notes: vec![
                    Note {
                        start_tick: 60,
                        duration_ticks: 60,
                        pitch: 72,
                        ..Note::default()
                    },
                    Note {
                        start_tick: 600,
                        duration_ticks: 60,
                        pitch: 74,
                        ..Note::default()
                    },
                ],
                controllers: vec![ControllerEvent {
                    tick: 120,
                    controller: 1,
                    value: 64,
                }],
                ..Pattern::default()
            })
            .unwrap();
        let unused = project
            .add_pattern(Pattern {
                name: "Unused".into(),
                ..Pattern::default()
            })
            .unwrap();
        assert_eq!(unused, 2);
        assert!(project.copy_pattern_events(source, source).is_err());
        assert_eq!(project.copy_pattern_events(source, 0).unwrap(), 2);
        assert_eq!(project.patterns[0].notes.len(), 2);
        assert_eq!(project.patterns[0].notes[1].pitch, 72);
        assert_eq!(project.patterns[0].controllers.len(), 1);

        assert_eq!(project.remove_unused_patterns(), 2);
        assert_eq!(project.patterns.len(), 1);
        assert_eq!(project.tracks[0].parts[0].pattern, 0);
    }

    #[test]
    fn split_and_join_parts_preserve_the_expanded_performance() {
        let mut project = project_with_part();
        let expected_before = project.to_performance().unwrap();

        let (left, right) = project.split_part(0, 0, 960).unwrap();
        assert_eq!((left, right), (0, 1));
        assert_eq!(project.tracks[0].parts[0].pattern, 0);
        assert_eq!(project.tracks[0].parts[1].pattern, 0);
        assert_eq!(
            project.to_performance().unwrap().tracks[0].notes,
            expected_before.tracks[0].notes
        );

        let mut mid_cycle = project_with_part();
        let expected_mid = mid_cycle.to_performance().unwrap();
        assert!(mid_cycle.split_part(0, 0, 480).is_err());
        let (left, right) = mid_cycle.split_part(0, 0, 720).unwrap();
        assert_eq!((left, right), (0, 1));
        assert_ne!(mid_cycle.tracks[0].parts[1].pattern, 0);
        assert_eq!(
            mid_cycle.to_performance().unwrap().tracks[0].notes,
            expected_mid.tracks[0].notes
        );

        let joined = mid_cycle.join_parts(0, 0, 1).unwrap();
        assert_eq!(joined, 0);
        assert_eq!(mid_cycle.tracks[0].parts.len(), 1);
        assert_eq!(
            mid_cycle.to_performance().unwrap().tracks[0].notes,
            expected_mid.tracks[0].notes
        );
        assert_eq!(
            mid_cycle.to_performance().unwrap().tracks[0].mixer_automation,
            expected_mid.tracks[0].mixer_automation
        );
    }

    #[test]
    fn cross_track_and_multi_part_moves_are_atomic_and_report_ranges() {
        let mut project = project_with_part();
        project.add_track(Track::default()).unwrap();
        project.duplicate_part(0, 0).unwrap();
        assert_eq!(
            project.parts_tick_range(&[(0, 0), (0, 1)]).unwrap(),
            (480, 2400)
        );

        let moved = project.move_part_to_track(0, 1, 1, 960).unwrap();
        assert_eq!(moved, 0);
        assert_eq!(project.tracks[0].parts.len(), 1);
        assert_eq!(project.tracks[1].parts.len(), 1);
        assert_eq!(project.tracks[1].parts[0].start_tick, 960);

        let before = project.clone();
        assert!(project.move_parts(&[(0, 0), (1, 0)], 0, -500).is_err());
        assert!(project.move_parts(&[(0, 0), (1, 0)], 1, 0).is_err());
        assert!(project.move_parts(&[(0, 0), (0, 0)], 0, 120).is_err());
        assert_eq!(project, before);

        let new_positions = project.move_parts(&[(0, 0), (1, 0)], 0, 120).unwrap();
        assert_eq!(new_positions, vec![(0, 0), (1, 0)]);
        assert_eq!(project.tracks[0].parts[0].start_tick, 600);
        assert_eq!(project.tracks[1].parts[0].start_tick, 1080);

        let cross = project.move_parts(&[(0, 0)], 1, -120).unwrap();
        assert_eq!(cross, vec![(1, 1)]);
        assert!(project.tracks[0].parts.is_empty());
        assert_eq!(project.tracks[1].parts.len(), 2);
        assert_eq!(project.tracks[1].parts[1].start_tick, 480);
    }
}
