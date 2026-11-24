//! Native project interchange and isolated note rendering through the public renderer.
use evil_project::{Note, Project, Track};
use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() < 4 {
        return Err(
            "usage: project_audio midi|wav|note PROJECT OUTPUT [instrument key ticks]".into(),
        );
    }
    let expected = match arguments[1].as_str() {
        "midi" | "wav" => 4,
        "note" => 7,
        _ => return Err("unknown operation".into()),
    };
    if arguments.len() != expected {
        return Err(
            "usage: project_audio midi|wav|note PROJECT OUTPUT [instrument key ticks]".into(),
        );
    }
    let path = Path::new(&arguments[2]);
    let mut project = Project::load(path)?;
    project.validate()?;
    let base = path.parent().ok_or("project has no parent")?;
    match arguments[1].as_str() {
        "midi" => {
            evil_exports::export_native_project_to_path(&project, base, Path::new(&arguments[3]))?;
        }
        "wav" => {
            let rendered = evil_render::render_project(&project, base)?;
            diagnostics(&rendered);
            rendered.sound.write_wav(&arguments[3])?;
        }
        "note" => {
            let instrument: usize = arguments[4].parse()?;
            project.tracks = vec![Track {
                name: "Isolated note".into(),
                instrument,
                notes: vec![Note {
                    pitch: arguments[5].parse()?,
                    velocity: 127,
                    start_tick: 0,
                    duration_ticks: arguments[6].parse()?,
                }],
                ..Track::default()
            }];
            project.patterns.clear();
            project.validate()?;
            if project.to_performance()?.duration_seconds() > 10.0 {
                return Err("isolated notes must not exceed 10 seconds".into());
            }
            let output = evil_render::render_project(&project, base)?;
            if output.skipped_drum_notes != 0 {
                return Err("isolated note is not mapped by the drum kit".into());
            }
            diagnostics(&output);
            let rendered = output.sound;
            let mono = evil_synth::RenderedSound {
                sample_rate: rendered.sample_rate,
                samples: rendered
                    .left
                    .iter()
                    .zip(&rendered.right)
                    .map(|(left, right)| (left + right) * 0.5)
                    .collect(),
            };
            mono.write_wav(&arguments[3])?;
        }
        _ => return Err("unknown operation".into()),
    }
    println!("wrote {}", arguments[3]);
    Ok(())
}

fn diagnostics(output: &evil_render::RenderOutput) {
    println!(
        "frames={} rate={} notes={} skipped_drums={} unsupported_controllers={} unsupported_mixer_automation={}",
        output.sound.left.len(),
        output.sound.sample_rate,
        output.note_count,
        output.skipped_drum_notes,
        output.unsupported_controller_events,
        output.unsupported_mixer_automation_events
    );
}
