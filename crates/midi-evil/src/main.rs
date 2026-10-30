mod app;
mod art;
mod automation;
mod beats;
mod beats_player;
mod history;
mod instance;
mod keyboard;
mod launch;
mod live_player;
mod patch_lab;
mod piano_roll;
mod preview;
mod render_job;
mod theme;
mod transport;
mod workspace;

use std::process::ExitCode;

use app::StudioApp;
use instance::{InstanceStart, SingleInstance};
use launch::LaunchOptions;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = LaunchOptions::parse()?;
    if options.help {
        println!("midi-evil [--project <dir>] [--track <name>] <file.mid>");
        return Ok(());
    }
    let instance = match SingleInstance::start(&options)? {
        InstanceStart::Primary(instance) => instance,
        InstanceStart::Secondary => return Ok(()),
    };
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MIDI Evil",
        native_options,
        Box::new(move |context| Ok(Box::new(StudioApp::new(context, options, Some(instance))))),
    )?;
    Ok(())
}
