//! What the roll opens on: the tracked Bach invention, as the native gate's sequences assume, or
//! one of the two shared-pattern edges the native gate reached through the arranger's buttons.

use evil_project::{Note, Part, Pattern, Project};

use crate::description::Meter;
use crate::piano_roll;

const BACH_INVENTION: &[u8] =
    include_bytes!("../../../examples/public-domain/bach-invention-02/bach-invention-02.mid");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// The invention's first track, a unique timeline.
    Bach,
    /// A new one-bar pattern with no notes, placed once on the first track.
    EmptyPattern,
    /// A one-bar pattern holding one note on every beat, placed twice on the first track, so
    /// there is no room after its last note and an edit reaches two parts.
    FullPattern,
}

impl Fixture {
    pub fn from_query(query: &str) -> Self {
        let wanted = query
            .trim_start_matches('?')
            .split('&')
            .find_map(|pair| pair.strip_prefix("fixture="));
        match wanted {
            Some("empty") => Self::EmptyPattern,
            Some("full") => Self::FullPattern,
            _ => Self::Bach,
        }
    }
}

/// The project, and the first track's part whose pattern the roll shows (`None`: the track's
/// unique timeline).
pub struct Loaded {
    pub project: Project,
    pub part: Option<usize>,
}

pub fn load(fixture: Fixture) -> Result<Loaded, String> {
    let performance =
        evil_imports::import_midi(BACH_INVENTION).map_err(|error| error.to_string())?;
    let mut project = Project::from_performance("bach-invention-02", &performance)
        .map_err(|error| error.to_string())?;
    let part_count = match fixture {
        Fixture::Bach => {
            return Ok(Loaded {
                project,
                part: None,
            });
        }
        Fixture::EmptyPattern => 1,
        Fixture::FullPattern => 2,
    };
    let bar_ticks = piano_roll::bar_ticks(&project);
    let beat_ticks = Meter {
        ticks_per_quarter: project.ticks_per_quarter,
        beats_per_bar: project.beats_per_bar,
        beat_unit: project.beat_unit,
    }
    .beat_ticks();
    let notes = if fixture == Fixture::FullPattern {
        (0..u64::from(project.beats_per_bar))
            .map(|beat| Note {
                pitch: 60,
                velocity: 100,
                start_tick: beat * beat_ticks,
                duration_ticks: beat_ticks,
            })
            .collect()
    } else {
        Vec::new()
    };
    let pattern = project
        .add_pattern(Pattern {
            name: "Pattern 1".to_owned(),
            length_ticks: bar_ticks,
            notes,
            ..Pattern::default()
        })
        .map_err(|error| error.to_string())?;
    let mut first_part = None;
    for occurrence in 0..part_count {
        let part = project
            .add_part(
                0,
                Part {
                    pattern,
                    start_tick: occurrence * bar_ticks,
                    length_ticks: bar_ticks,
                },
            )
            .map_err(|error| error.to_string())?;
        first_part.get_or_insert(part);
    }
    Ok(Loaded {
        project,
        part: first_part,
    })
}
