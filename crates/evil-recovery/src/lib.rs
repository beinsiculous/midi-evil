//! Repeatable analysis for assisted reconstruction of user-owned WAV recordings.

use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use evil_project::{Note, Project, Track};

const HOP: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioAnalysis {
    pub source: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_seconds: f64,
    pub active_start_seconds: Option<f64>,
    pub active_end_seconds: Option<f64>,
    pub peak: f32,
    pub rms: f32,
    pub onset_count: usize,
    pub onsets_seconds: Vec<f64>,
    pub onsets_truncated: bool,
    pub tempo_candidates: Vec<TempoCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TempoCandidate {
    pub bpm: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftReport {
    pub source: String,
    pub output: String,
    pub bpm: f64,
    pub grid: u8,
    pub quantization_origin_seconds: f64,
    pub note_count: usize,
    pub tempo_candidates: Vec<TempoCandidate>,
}

/// Quantize detected onsets into an editable guide track. This is intentionally
/// a starting point for review, not a claim that every onset is a drum note.
pub fn create_draft(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    bpm: Option<f64>,
    grid: u8,
) -> Result<DraftReport, Error> {
    let source = source.as_ref();
    let output = output.as_ref();
    let analysis = analyze_file(source)?;
    if analysis.onsets_seconds.is_empty() {
        return Err(Error::NoOnsets);
    }
    if !matches!(grid, 1 | 2 | 4 | 8 | 16 | 32) {
        return Err(Error::InvalidGrid(grid));
    }
    let bpm = bpm
        .or_else(|| {
            analysis
                .tempo_candidates
                .first()
                .map(|candidate| candidate.bpm)
        })
        .ok_or(Error::NoTempo)?;
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(Error::NoTempo);
    }
    let step_seconds = 60.0 / bpm * 4.0 / f64::from(grid);
    let origin = best_grid_origin(&analysis.onsets_seconds, step_seconds);
    let step_ticks = 480_u64 * 4 / u64::from(grid);
    let mut project = Project::new(format!(
        "{} recovery",
        source.file_stem().unwrap_or_default().to_string_lossy()
    ));
    project.tempo_bpm = bpm as f32;
    let mut notes = Vec::new();
    for time in &analysis.onsets_seconds {
        let relative = time - origin;
        let start_tick = (relative / step_seconds).round().max(0.0) as u64 * step_ticks;
        notes.push(Note {
            pitch: 36,
            velocity: 100,
            start_tick,
            duration_ticks: step_ticks,
        });
    }
    project.add_track(Track {
        name: "Recovery Onsets".to_owned(),
        patch: "click".to_owned(),
        notes,
        ..Track::default()
    })?;
    project.save(output)?;
    Ok(DraftReport {
        source: analysis.source,
        output: output.display().to_string(),
        bpm,
        grid,
        quantization_origin_seconds: origin,
        note_count: project.tracks[0].notes.len(),
        tempo_candidates: analysis.tempo_candidates,
    })
}

fn best_grid_origin(onsets: &[f64], step_seconds: f64) -> f64 {
    let first = onsets[0];
    (0..32)
        .map(|index| first + f64::from(index) * step_seconds / 32.0)
        .min_by(|left, right| {
            let score = |origin: f64| {
                onsets
                    .iter()
                    .take(512)
                    .map(|time| {
                        let residue = (time - origin).rem_euclid(step_seconds);
                        residue.min(step_seconds - residue).powi(2)
                    })
                    .sum::<f64>()
            };
            score(*left).total_cmp(&score(*right))
        })
        .expect("at least one grid origin")
}

pub fn read_wav(path: impl AsRef<Path>) -> Result<AudioBuffer, Error> {
    let path = path.as_ref();
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(Error::InvalidWav(
            "missing channels or sample rate".to_owned(),
        ));
    }
    let mut samples = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for (index, sample) in reader.samples::<f32>().enumerate() {
                let value = sample?;
                if index % spec.channels as usize == 0 {
                    samples.push(0.0);
                }
                *samples.last_mut().expect("a channel was just added") +=
                    value / f32::from(spec.channels);
            }
        }
        hound::SampleFormat::Int => {
            if spec.bits_per_sample <= 16 {
                let scale = (1_i32 << spec.bits_per_sample.saturating_sub(1)) as f32;
                for (index, sample) in reader.samples::<i16>().enumerate() {
                    let value = f32::from(sample?) / scale;
                    if index % spec.channels as usize == 0 {
                        samples.push(0.0);
                    }
                    *samples.last_mut().expect("a channel was just added") +=
                        value / f32::from(spec.channels);
                }
            } else {
                let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
                for (index, sample) in reader.samples::<i32>().enumerate() {
                    let value = sample? as f32 / scale;
                    if index % spec.channels as usize == 0 {
                        samples.push(0.0);
                    }
                    *samples.last_mut().expect("a channel was just added") +=
                        value / f32::from(spec.channels);
                }
            }
        }
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::InvalidWav("non-finite sample data".to_owned()));
    }
    Ok(AudioBuffer {
        samples,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
    })
}

pub fn analyze_file(path: impl AsRef<Path>) -> Result<AudioAnalysis, Error> {
    let path = path.as_ref();
    let buffer = read_wav(path)?;
    let analysis = analyze(&buffer);
    Ok(AudioAnalysis {
        source: path.display().to_string(),
        ..analysis
    })
}

pub fn analyze(buffer: &AudioBuffer) -> AudioAnalysis {
    let hop_rate = buffer.sample_rate as f64 / HOP as f64;
    let rms_windows = rms_windows(&buffer.samples);
    let peak = buffer
        .samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let rms = (buffer
        .samples
        .iter()
        .map(|sample| sample * sample)
        .sum::<f32>()
        / buffer.samples.len().max(1) as f32)
        .sqrt();
    let max_window = rms_windows.iter().copied().fold(0.0_f32, f32::max);
    let threshold = max_window * 10.0_f32.powf(-45.0 / 20.0);
    let active_start = rms_windows.iter().position(|value| *value >= threshold);
    let active_end = rms_windows.iter().rposition(|value| *value >= threshold);
    let onsets = onset_times(&rms_windows, hop_rate);
    let tempo_candidates = tempo_candidates(&rms_windows, hop_rate);
    AudioAnalysis {
        source: String::new(),
        sample_rate: buffer.sample_rate,
        channels: buffer.channels,
        duration_seconds: buffer.samples.len() as f64 / f64::from(buffer.sample_rate),
        active_start_seconds: active_start.map(|frame| frame as f64 / hop_rate),
        active_end_seconds: active_end.map(|frame| (frame + 1) as f64 / hop_rate),
        peak,
        rms,
        onset_count: onsets.len(),
        onsets_seconds: onsets,
        onsets_truncated: false,
        tempo_candidates,
    }
}

fn rms_windows(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks(HOP)
        .map(|chunk| {
            (chunk.iter().map(|sample| sample * sample).sum::<f32>() / chunk.len() as f32).sqrt()
        })
        .collect()
}

fn onset_times(windows: &[f32], hop_rate: f64) -> Vec<f64> {
    let flux: Vec<f32> = windows
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .collect();
    if flux.is_empty() {
        return Vec::new();
    }
    let mean = flux.iter().sum::<f32>() / flux.len() as f32;
    let deviation =
        (flux.iter().map(|value| (value - mean).powi(2)).sum::<f32>() / flux.len() as f32).sqrt();
    let threshold = mean + 2.5 * deviation;
    let minimum_gap = (0.09 * hop_rate) as usize;
    let mut onsets = Vec::new();
    for index in 0..flux.len().saturating_sub(1) {
        if flux[index] >= threshold
            && (index == 0 || flux[index] >= flux[index - 1])
            && flux[index] >= flux[index + 1]
            && onsets
                .last()
                .is_none_or(|previous: &usize| index - *previous >= minimum_gap)
        {
            onsets.push(index);
        }
    }
    onsets
        .iter()
        .map(|frame| (frame + 1) as f64 / hop_rate)
        .collect()
}

fn tempo_candidates(windows: &[f32], hop_rate: f64) -> Vec<TempoCandidate> {
    let flux: Vec<f32> = windows
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .collect();
    if flux.len() < 16 {
        return Vec::new();
    }
    let mean = flux.iter().sum::<f32>() / flux.len() as f32;
    let centered: Vec<f32> = flux.iter().map(|value| value - mean).collect();
    let energy = centered.iter().map(|value| value * value).sum::<f32>();
    if energy <= f32::EPSILON {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    for lag in 1..centered.len() / 2 {
        let bpm = 60.0 * hop_rate / lag as f64;
        if !(60.0..=200.0).contains(&bpm) {
            continue;
        }
        let score = centered[..centered.len() - lag]
            .iter()
            .zip(&centered[lag..])
            .map(|(left, right)| left * right)
            .sum::<f32>()
            / energy;
        candidates.push(TempoCandidate {
            bpm,
            score: f64::from(score),
        });
    }
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    let mut distinct = Vec::new();
    for candidate in candidates {
        if distinct
            .iter()
            .all(|other: &TempoCandidate| (candidate.bpm - other.bpm).abs() / other.bpm > 0.015)
        {
            distinct.push(candidate);
        }
        if distinct.len() == 3 {
            break;
        }
    }
    distinct
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Wav(#[from] hound::Error),
    #[error("invalid WAV: {0}")]
    InvalidWav(String),
    #[error("no onsets were detected")]
    NoOnsets,
    #[error("no usable tempo was detected; provide --bpm")]
    NoTempo,
    #[error("grid must be 1, 2, 4, 8, 16, or 32, got {0}")]
    InvalidGrid(u8),
    #[error(transparent)]
    Project(#[from] evil_project::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_activity_onsets_and_a_pulse_tempo() {
        let sample_rate = 48_000;
        let mut samples = vec![0.0; sample_rate as usize * 8];
        for beat in 0..16 {
            let start = beat * sample_rate as usize / 2;
            for offset in 0..800 {
                samples[start + offset] = 0.8;
            }
        }
        let analysis = analyze(&AudioBuffer {
            samples,
            sample_rate,
            channels: 1,
        });
        assert_eq!(analysis.active_start_seconds, Some(0.0));
        assert_eq!(analysis.onset_count, 15);
        assert!((analysis.tempo_candidates[0].bpm - 120.0).abs() < 1.0);
    }

    #[test]
    fn draft_quantizes_onsets_into_a_native_project() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("pulse.wav");
        let sample_rate = 48_000;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&source, spec).unwrap();
        for _beat in 0..16 {
            for frame in 0..sample_rate as usize / 2 {
                let value = if frame < 800 { 8_000 } else { 0 };
                writer.write_sample(value).unwrap();
            }
        }
        writer.finalize().unwrap();
        let output = directory.path().join("draft.evil");
        let report = create_draft(&source, &output, Some(120.0), 16).unwrap();
        let project = Project::load(output).unwrap();
        assert_eq!(report.note_count, 15);
        assert_eq!(project.tempo_bpm, 120.0);
        assert!(
            project.tracks[0]
                .notes
                .iter()
                .all(|note| note.start_tick % 120 == 0)
        );
    }

    #[test]
    fn empty_audio_does_not_panic() {
        let analysis = analyze(&AudioBuffer {
            samples: Vec::new(),
            sample_rate: 48_000,
            channels: 1,
        });
        assert_eq!(analysis.active_start_seconds, None);
        assert!(analysis.tempo_candidates.is_empty());
    }
}
