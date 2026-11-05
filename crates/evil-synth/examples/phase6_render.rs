use std::path::{Path, PathBuf};

use evil_synth::{MonoTrack, Patch, SynthNote, mix_stereo, preset, render_notes_dry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut preset_name = "nano-303".to_owned();
    let mut output_dir = PathBuf::from("target/phase6/native");
    let mut patch_json = None::<PathBuf>;
    let mut dump_patch = None::<PathBuf>;
    let mut cases = None::<String>;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or("missing option value")?;
        match argument.as_str() {
            "--preset" => preset_name = value,
            "--output-dir" => output_dir = PathBuf::from(value),
            "--patch-json" => patch_json = Some(PathBuf::from(value)),
            "--dump-patch" => dump_patch = Some(PathBuf::from(value)),
            "--cases" => cases = Some(value),
            _ => return Err(format!("unknown option: {argument}").into()),
        }
    }
    let patch = match patch_json {
        Some(path) => Patch::from_json(&std::fs::read_to_string(path)?)?,
        None => preset(&preset_name)?,
    };
    if let Some(path) = dump_patch {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, patch.to_json_pretty()?)?;
    }
    std::fs::create_dir_all(&output_dir)?;
    let selected = cases.as_deref().unwrap_or(match preset_name.as_str() {
        "glocken" => "c2,c3,c4,c5",
        _ => "c2,c3,c4,c5,soft,accent,glide",
    });
    for case in selected.split(',').filter(|case| !case.is_empty()) {
        let (pitch, velocity) = match case {
            "c2" => (36, 110),
            "c3" => (48, 110),
            "c4" => (60, 110),
            "c5" => (72, 110),
            "soft" => (60, 64),
            "accent" => (60, 127),
            "glide" => (60, 100),
            _ => return Err(format!("unknown case: {case}").into()),
        };
        let notes = if case == "glide" {
            vec![
                SynthNote {
                    start_seconds: 0.0,
                    duration_seconds: 1.0,
                    pitch,
                    velocity,
                },
                SynthNote {
                    start_seconds: 1.0,
                    duration_seconds: 1.0,
                    pitch: 64,
                    velocity: 120,
                },
            ]
        } else {
            vec![SynthNote {
                start_seconds: 0.0,
                duration_seconds: 2.0,
                pitch,
                velocity,
            }]
        };
        write_case(&patch, &notes, &output_dir.join(format!("{case}.wav")))?;
    }
    Ok(())
}

fn write_case(
    patch: &Patch,
    notes: &[SynthNote],
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sound = render_notes_dry(patch, notes)?;
    let stereo = mix_stereo(
        &[MonoTrack {
            sound: &sound,
            gain: 1.0,
            pan: 0.0,
            waveshaper: patch.waveshaper.as_ref(),
            delay: patch.delay.as_ref(),
            controllers: &[],
            automation: &[],
            inserts: &[],
            send_gains: [0.0; evil_synth::SEND_COUNT],
            dc_block: patch.uses_sync(),
        }],
        1.0,
    )?;
    stereo.write_wav(path)?;
    Ok(())
}
