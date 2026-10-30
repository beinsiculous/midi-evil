use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [command, path] if command == "inspect" => match evil_recovery::analyze_file(path) {
            Ok(analysis) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&analysis).expect("analysis is serializable")
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        [command, source, output, options @ ..] if command == "draft" => {
            let mut bpm: Option<f64> = None;
            let mut grid: u8 = 16;
            let mut index = 0;
            let mut valid = true;
            while index < options.len() {
                match options[index].as_str() {
                    "--bpm" => {
                        bpm = options.get(index + 1).and_then(|value| value.parse().ok());
                        index += 1;
                    }
                    "--grid" => {
                        grid = options
                            .get(index + 1)
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0);
                        index += 1;
                    }
                    _ => valid = false,
                }
                index += 1;
            }
            if !valid {
                eprintln!(
                    "Usage: evil-recovery draft INPUT.wav OUTPUT.evil [--bpm N] [--grid 1|2|4|8|16|32]"
                );
                return ExitCode::FAILURE;
            }
            match evil_recovery::create_draft(source, output, bpm, grid) {
                Ok(report) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report)
                            .expect("draft report is serializable")
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("Usage: evil-recovery inspect INPUT.wav");
            eprintln!(
                "       evil-recovery draft INPUT.wav OUTPUT.evil [--bpm N] [--grid 1|2|4|8|16|32]"
            );
            ExitCode::FAILURE
        }
    }
}
