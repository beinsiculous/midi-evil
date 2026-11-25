//! The arranger's cursor: a track and, on it, either its unique timeline or one of its parts.
//! Along a track the cursor walks the timeline first and then the parts in the order they play;
//! between tracks it lands on the part playing at the same point in the song. Parts have no ids,
//! so a part is its index in the track's list, and every edit that changes the list says where
//! the cursor goes.

use evil_project::{Project, Track};

/// Where the arranger's cursor is. `part` is `None` on the track's unique timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SongCursor {
    pub track: usize,
    pub part: Option<usize>,
}

/// What a move of the cursor found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walk {
    Moved(SongCursor),
    AlreadyFirst,
    AlreadyLast,
    NoTrackAbove,
    NoTrackBelow,
}

/// The cursor kept inside the project: a track that is gone becomes the last one, a part that is
/// gone becomes the track's timeline. `None` when there are no tracks.
pub fn kept_inside(project: &Project, cursor: SongCursor) -> Option<SongCursor> {
    let last_track = project.tracks.len().checked_sub(1)?;
    let track = cursor.track.min(last_track);
    let part = cursor
        .part
        .filter(|part| *part < project.tracks[track].parts.len());
    Some(SongCursor { track, part })
}

/// A track's parts in the order they play: by start, then by their place in the list.
pub fn parts_in_time_order(track: &Track) -> Vec<usize> {
    let mut order: Vec<usize> = (0..track.parts.len()).collect();
    order.sort_by_key(|&index| (track.parts[index].start_tick, index));
    order
}

/// How many parts, on any track, play this pattern.
pub fn parts_sharing(project: &Project, pattern: usize) -> usize {
    project
        .tracks
        .iter()
        .flat_map(|track| &track.parts)
        .filter(|part| part.pattern == pattern)
        .count()
}

/// The cursor's stops along a track: the timeline, then each part in time order.
fn row(track: &Track) -> Vec<Option<usize>> {
    std::iter::once(None)
        .chain(parts_in_time_order(track).into_iter().map(Some))
        .collect()
}

fn place(track: &Track, part: Option<usize>) -> usize {
    row(track)
        .iter()
        .position(|stop| *stop == part)
        .unwrap_or(0)
}

pub fn previous(project: &Project, cursor: SongCursor) -> Walk {
    let track = &project.tracks[cursor.track];
    match place(track, cursor.part) {
        0 => Walk::AlreadyFirst,
        place => Walk::Moved(SongCursor {
            part: row(track)[place - 1],
            ..cursor
        }),
    }
}

pub fn next(project: &Project, cursor: SongCursor) -> Walk {
    let track = &project.tracks[cursor.track];
    let row = row(track);
    match row.get(place(track, cursor.part) + 1) {
        Some(part) => Walk::Moved(SongCursor {
            part: *part,
            ..cursor
        }),
        None => Walk::AlreadyLast,
    }
}

/// Home: the track's timeline, the start of its row.
pub fn first(cursor: SongCursor) -> Walk {
    match cursor.part {
        None => Walk::AlreadyFirst,
        Some(_) => Walk::Moved(SongCursor {
            part: None,
            ..cursor
        }),
    }
}

/// End: the part that plays last on the track.
pub fn last(project: &Project, cursor: SongCursor) -> Walk {
    let last = row(&project.tracks[cursor.track]).last().copied().flatten();
    if last == cursor.part {
        Walk::AlreadyLast
    } else {
        Walk::Moved(SongCursor {
            part: last,
            ..cursor
        })
    }
}

pub fn above(project: &Project, cursor: SongCursor) -> Walk {
    match cursor.track.checked_sub(1) {
        Some(track) => Walk::Moved(landing(project, cursor, track)),
        None => Walk::NoTrackAbove,
    }
}

pub fn below(project: &Project, cursor: SongCursor) -> Walk {
    let track = cursor.track + 1;
    if track < project.tracks.len() {
        Walk::Moved(landing(project, cursor, track))
    } else {
        Walk::NoTrackBelow
    }
}

/// Where the cursor lands on another track: from a timeline, on its timeline; from a part, on
/// the last part there that starts no later, which is the one playing at that point when any is,
/// and on the timeline when none starts that early.
fn landing(project: &Project, from: SongCursor, track: usize) -> SongCursor {
    let start = from
        .part
        .map(|part| project.tracks[from.track].parts[part].start_tick);
    let other = &project.tracks[track];
    let part = start.and_then(|start| {
        parts_in_time_order(other)
            .into_iter()
            .rfind(|&index| other.parts[index].start_tick <= start)
    });
    SongCursor { track, part }
}

/// Where the cursor goes when its part is removed: the part that plays next, else the one
/// before, else the timeline — as an index into the list the removal leaves.
pub fn after_removal(track: &Track, removed: usize) -> Option<usize> {
    let order = parts_in_time_order(track);
    let place = order.iter().position(|&index| index == removed)?;
    order
        .get(place + 1)
        .or_else(|| place.checked_sub(1).and_then(|before| order.get(before)))
        .map(|&other| if other > removed { other - 1 } else { other })
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::{Part, Pattern};

    /// Two tracks: the first with parts at bars 3, 1 and 2 (stored out of order), the second
    /// with one part at bar 2.
    fn arrangement() -> Project {
        let mut project = Project::new("Cursor");
        for _ in 0..2 {
            project.add_track(evil_project::Track::default()).unwrap();
        }
        let pattern = project
            .add_pattern(Pattern {
                length_ticks: 1_920,
                ..Pattern::default()
            })
            .unwrap();
        for (track, bar) in [(0, 2), (0, 0), (0, 1), (1, 1)] {
            project
                .add_part(
                    track,
                    Part {
                        pattern,
                        start_tick: bar * 1_920,
                        length_ticks: 1_920,
                    },
                )
                .unwrap();
        }
        project
    }

    fn at(track: usize, part: Option<usize>) -> SongCursor {
        SongCursor { track, part }
    }

    #[test]
    fn along_a_track_the_cursor_walks_the_timeline_then_the_parts_in_the_order_they_play() {
        let project = arrangement();
        let mut cursor = at(0, None);
        let mut walked = Vec::new();
        while let Walk::Moved(next) = next(&project, cursor) {
            walked.push(next.part);
            cursor = next;
        }
        assert_eq!(walked, [Some(1), Some(2), Some(0)]);
        assert_eq!(next(&project, cursor), Walk::AlreadyLast);
        assert_eq!(previous(&project, cursor), Walk::Moved(at(0, Some(2))));
        assert_eq!(previous(&project, at(0, None)), Walk::AlreadyFirst);
        assert_eq!(first(at(0, Some(2))), Walk::Moved(at(0, None)));
        assert_eq!(first(at(0, None)), Walk::AlreadyFirst);
        assert_eq!(last(&project, at(0, None)), Walk::Moved(at(0, Some(0))));
        assert_eq!(last(&project, at(0, Some(0))), Walk::AlreadyLast);
    }

    #[test]
    fn between_tracks_the_cursor_lands_on_the_part_playing_at_that_point() {
        let project = arrangement();
        assert_eq!(below(&project, at(0, Some(0))), Walk::Moved(at(1, Some(0))));
        assert_eq!(below(&project, at(0, Some(1))), Walk::Moved(at(1, None)));
        assert_eq!(below(&project, at(0, None)), Walk::Moved(at(1, None)));
        assert_eq!(above(&project, at(1, Some(0))), Walk::Moved(at(0, Some(2))));
        assert_eq!(above(&project, at(0, None)), Walk::NoTrackAbove);
        assert_eq!(below(&project, at(1, None)), Walk::NoTrackBelow);
    }

    #[test]
    fn a_removed_part_hands_the_cursor_to_the_next_in_time_else_the_one_before() {
        let project = arrangement();
        let track = &project.tracks[0];
        assert_eq!(
            after_removal(track, 1),
            Some(1),
            "bar 2's part, shifted down"
        );
        assert_eq!(
            after_removal(track, 0),
            Some(1),
            "the last; bar 2's before it"
        );
        assert_eq!(after_removal(&project.tracks[1], 0), None, "the timeline");
        assert_eq!(parts_sharing(&project, 0), 4);
    }

    #[test]
    fn a_cursor_outside_the_project_is_brought_inside_it() {
        let project = arrangement();
        assert_eq!(kept_inside(&project, at(5, Some(9))), Some(at(1, None)));
        assert_eq!(kept_inside(&project, at(0, Some(2))), Some(at(0, Some(2))));
        assert_eq!(kept_inside(&Project::new("Empty"), at(0, None)), None);
    }
}
