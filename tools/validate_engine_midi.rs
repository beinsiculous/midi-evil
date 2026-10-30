use std::error::Error;
use std::fs;

use music_midi::SongData;

fn main() -> Result<(), Box<dyn Error>> {
    let mut allow_overlaps = false;
    let mut compare = false;
    let mut paths = Vec::new();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--allow-overlaps" => allow_overlaps = true,
            "--compare" => compare = true,
            _ => paths.push(argument),
        }
    }
    if compare {
        return compare_tracks(&paths);
    }
    for path in paths {
        let (song, _) = load(&path, allow_overlaps)?;
        println!(
            "{path}: {} tracks, {} notes, {} ticks/quarter, {} tempo changes, {} time signatures",
            song.tracks().len(),
            song.note_count(),
            song.ticks_per_quarter(),
            song.tempo_changes().len(),
            song.time_signatures().len(),
        );
    }
    Ok(())
}

fn compare_tracks(paths: &[String]) -> Result<(), Box<dyn Error>> {
    let left_path = paths.first().ok_or("missing original MIDI path")?;
    let right_path = paths.get(1).ok_or("missing comparison MIDI path")?;
    let (left, _) = load(left_path, false)?;
    let (right, _) = load(right_path, false)?;
    let differences: Vec<_> = left
        .tracks()
        .iter()
        .zip(right.tracks())
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(index, _)| index)
        .collect();
    if differences.len() != 1 {
        return Err(format!("expected one changed track, found {differences:?}").into());
    }
    let index = differences[0];
    println!(
        "Only track {index} changed: {:?} -> {:?}",
        left.tracks()[index].name(),
        right.tracks()[index].name()
    );
    let retained_differences = left
        .retained_tracks()
        .iter()
        .zip(right.retained_tracks())
        .filter(|(left, right)| left != right)
        .count();
    if retained_differences != 1 {
        return Err(format!("expected one changed retained track, found {retained_differences}").into());
    }
    Ok(())
}

fn load(path: &str, allow_overlaps: bool) -> Result<(SongData, music_midi::UnsupportedReport), Box<dyn Error>> {
    let data = fs::read(path)?;
    let (song, report) = SongData::from_smf(&data)?;
    if !allow_overlaps && (!report.unsupported.is_empty() || !report.same_key_overlaps.is_empty()) {
        return Err(format!("{path}: unsupported report: {report:?}").into());
    }
    if allow_overlaps && !report.same_key_overlaps.is_empty() {
        println!("{path}: engine overlap report: {report:?}");
    }
    Ok((song, report))
}
