use evil_project::Project;

use crate::{Error, RenderScope};

/// One scoped WAV export. `file_name` is a bounded ASCII file name, never a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSpec {
    pub scope: RenderScope,
    pub file_name: String,
}

/// A non-transactional batch plan. Noteless items omitted from `exports` are counted in `skipped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    pub exports: Vec<ExportSpec>,
    pub skipped: usize,
}

/// Plan one stem per note-containing original track, or one explicitly selected stem.
/// Track indices are zero-based original project indices.
pub fn stem_export_plan(
    project: &Project,
    track_index: Option<usize>,
) -> Result<ExportPlan, Error> {
    project.validate()?;
    if let Some(index) = track_index {
        validate_track(project, index)?;
    }
    let indices: Vec<_> = match track_index {
        Some(index) => vec![index],
        None => (0..project.tracks.len()).collect(),
    };
    let mut plan = ExportPlan {
        exports: Vec::new(),
        skipped: 0,
    };
    for index in indices {
        let scope = RenderScope::Track { track_index: index };
        if scope_has_notes(project, scope) {
            plan.exports.push(ExportSpec {
                scope,
                file_name: stem_file_name(index, &project.tracks[index].name),
            });
        } else {
            plan.skipped += 1;
        }
    }
    if let Some(index) = track_index
        && plan.exports.is_empty()
    {
        return Err(Error::EmptySelection(format!(
            "track index {index} has no notes to export"
        )));
    }
    Ok(plan)
}

/// Plan one export per note-containing timeline part, optionally filtered by original track/part.
/// Track and part indices are zero-based original project indices. A part filter requires a track.
pub fn part_export_plan(
    project: &Project,
    track_index: Option<usize>,
    part_index: Option<usize>,
) -> Result<ExportPlan, Error> {
    project.validate()?;
    if part_index.is_some() && track_index.is_none() {
        return Err(Error::InvalidSelection(
            "selecting an arrangement part also requires selecting its track".to_owned(),
        ));
    }
    if let Some(index) = track_index {
        validate_track(project, index)?;
    }
    if let (Some(track), Some(part)) = (track_index, part_index)
        && project.tracks[track].parts.get(part).is_none()
    {
        return Err(Error::InvalidSelection(format!(
            "arrangement part index {part} does not exist on track index {track}"
        )));
    }

    let tracks: Vec<_> = match track_index {
        Some(index) => vec![index],
        None => (0..project.tracks.len()).collect(),
    };
    let mut plan = ExportPlan {
        exports: Vec::new(),
        skipped: 0,
    };
    for track in tracks {
        let parts: Vec<_> = match part_index {
            Some(index) => vec![index],
            None => (0..project.tracks[track].parts.len()).collect(),
        };
        for part in parts {
            let scope = RenderScope::TrackPart {
                track_index: track,
                part_index: part,
            };
            if scope_has_notes(project, scope) {
                let part_data = project.tracks[track].parts[part];
                let pattern = &project.patterns[part_data.pattern];
                plan.exports.push(ExportSpec {
                    scope,
                    file_name: part_file_name(
                        track,
                        &project.tracks[track].name,
                        part,
                        &pattern.name,
                    ),
                });
            } else {
                plan.skipped += 1;
            }
        }
    }
    if (track_index.is_some() || part_index.is_some()) && plan.exports.is_empty() {
        return Err(Error::EmptySelection(match (track_index, part_index) {
            (Some(track), Some(part)) => format!(
                "arrangement part index {part} on track index {track} has no notes to export"
            ),
            (Some(track), None) => {
                format!("track index {track} has no note-containing arrangement parts")
            }
            _ => unreachable!("an explicit selection was checked above"),
        }));
    }
    Ok(plan)
}

/// File names use one-based original indices: tracks are at least two digits and parts four.
/// Thus original track 0 / part 2 is named `01-…__part-0003-….wav`, independent of batch filtering.
pub fn stem_file_name(track_index: usize, track_name: &str) -> String {
    format!(
        "{:02}-{}.wav",
        track_index + 1,
        ascii_slug(track_name, "track")
    )
}

/// The part index is the original zero-based `Track::parts` position, displayed one-based.
pub fn part_file_name(
    track_index: usize,
    track_name: &str,
    part_index: usize,
    pattern_name: &str,
) -> String {
    format!(
        "{:02}-{}__part-{:04}-{}.wav",
        track_index + 1,
        ascii_slug(track_name, "track"),
        part_index + 1,
        ascii_slug(pattern_name, "part")
    )
}

pub(crate) fn validate_track(project: &Project, track_index: usize) -> Result<(), Error> {
    if project.tracks.get(track_index).is_none() {
        return Err(Error::InvalidSelection(format!(
            "track index {track_index} does not exist"
        )));
    }
    Ok(())
}

pub(crate) fn scope_has_notes(project: &Project, scope: RenderScope) -> bool {
    match scope {
        RenderScope::FullMix => project.to_performance().is_ok_and(|p| p.note_count() > 0),
        RenderScope::Track { track_index } => {
            let track = &project.tracks[track_index];
            !track.notes.is_empty()
                || track
                    .parts
                    .iter()
                    .any(|part| part_has_notes(&project.patterns[part.pattern], *part))
        }
        RenderScope::TrackPart {
            track_index,
            part_index,
        } => {
            let part = project.tracks[track_index].parts[part_index];
            part_has_notes(&project.patterns[part.pattern], part)
        }
    }
}

fn part_has_notes(pattern: &evil_project::Pattern, part: evil_project::Part) -> bool {
    part.length_ticks > 0
        && pattern
            .notes
            .iter()
            .any(|note| note.start_tick < part.length_ticks)
}

fn ascii_slug(input: &str, fallback: &str) -> String {
    const MAX_SLUG_BYTES: usize = 48;

    let mut slug = String::new();
    let mut separator = false;
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
        if slug.len() == MAX_SLUG_BYTES {
            break;
        }
    }
    slug.truncate(MAX_SLUG_BYTES);
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Component, Path};

    use evil_project::{Note, Part, Pattern, Track};

    use super::*;

    #[test]
    fn names_are_bounded_collision_safe_ascii_with_original_one_based_indices() {
        let name = part_file_name(
            0,
            "../Lead / VERY LONG NAME THAT IS TRUNCATED WITHOUT PATH CHARACTERS 123456789",
            2,
            "../../Verse #1",
        );
        assert_eq!(
            name,
            "01-lead-very-long-name-that-is-truncated-without-pa__part-0003-verse-1.wav"
        );
        assert!(name.len() < 120);
        assert!(name.is_ascii());
        assert_eq!(Path::new(&name).components().count(), 1);
        assert!(matches!(
            Path::new(&name).components().next(),
            Some(Component::Normal(_))
        ));
        assert_ne!(
            stem_file_name(0, "Same",),
            stem_file_name(1, "Same",),
            "original indices prevent collisions between duplicate names"
        );
        assert_eq!(stem_file_name(0, "///"), "01-track.wav");
    }

    #[test]
    fn plans_skip_noteless_batches_but_reject_explicit_empty_selections() {
        let mut project = Project::new("Plans");
        project.add_track(Track::default()).unwrap();
        project.add_track(Track::default()).unwrap();
        project.tracks[1].name = "Drums".to_owned();
        project.tracks[1].notes.push(Note::default());

        let stems = stem_export_plan(&project, None).unwrap();
        assert_eq!(stems.exports.len(), 1);
        assert_eq!(stems.skipped, 1);
        assert_eq!(stems.exports[0].file_name, "02-drums.wav");
        assert!(matches!(
            stem_export_plan(&project, Some(0)),
            Err(Error::EmptySelection(_))
        ));
        assert!(matches!(
            stem_export_plan(&project, Some(9)),
            Err(Error::InvalidSelection(_))
        ));

        let pattern = project.add_pattern(Pattern::default()).unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: 480,
                    length_ticks: 480,
                },
            )
            .unwrap();
        project.patterns[pattern].notes.push(Note::default());
        let cropped = project
            .add_pattern(Pattern {
                name: "Cropped".to_owned(),
                length_ticks: 480,
                notes: vec![Note {
                    start_tick: 240,
                    ..Note::default()
                }],
                ..Pattern::default()
            })
            .unwrap();
        project
            .add_part(
                0,
                Part {
                    pattern: cropped,
                    start_tick: 960,
                    length_ticks: 120,
                },
            )
            .unwrap();
        let parts = part_export_plan(&project, None, None).unwrap();
        assert_eq!(parts.exports.len(), 1);
        assert_eq!(parts.skipped, 1);
        assert!(matches!(
            part_export_plan(&project, Some(0), Some(1)),
            Err(Error::EmptySelection(_))
        ));
        assert_eq!(
            parts.exports[0].file_name,
            "01-track__part-0001-pattern.wav"
        );
        assert!(matches!(
            part_export_plan(&project, Some(1), None),
            Err(Error::EmptySelection(_))
        ));
        assert!(matches!(
            part_export_plan(&project, None, Some(0)),
            Err(Error::InvalidSelection(_))
        ));
        assert!(matches!(
            part_export_plan(&project, Some(0), Some(2)),
            Err(Error::InvalidSelection(_))
        ));
    }
}
