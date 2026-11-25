//! MIDI Evil, the workstation, as a library: the desktop binary is a `main` that calls
//! `run_native`, and a browser build constructs `StudioApp` itself. The public surface is
//! that constructor and the types it names; every module stays private to the crate.

mod accessibility;
mod announcer;
mod app;
mod arranger;
mod art;
mod automation;
mod beats;
mod beats_player;
mod custom_editors;
mod dialogs;
mod effect_ui;
mod history;
mod instance;
mod instruments;
mod key_help;
mod keyboard;
mod launch;
mod live_player;
mod midi_input;
mod mixer;
mod mixer_automation;
mod patch_lab;
mod piano_roll;
mod preview;
mod recording;
mod recovery;
mod regions;
mod render_job;
#[cfg(not(target_arch = "wasm32"))]
mod sample_recorder;
#[cfg(target_arch = "wasm32")]
#[path = "sample_recorder_unavailable.rs"]
mod sample_recorder;
mod speech;
mod theme;
mod transport;
mod workspace;

pub use app::StudioApp;
pub use dialogs::Dialogs;
pub use instance::SingleInstance;
pub use launch::LaunchOptions;

/// The desktop application: reads the command line, hands the launch to a running copy when
/// there is one, and otherwise opens the window and runs until it closes.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> Result<(), Box<dyn std::error::Error>> {
    use instance::InstanceStart;

    let options = LaunchOptions::parse()?;
    if options.help {
        println!(
            "midi-evil [--project <dir>] [--track <name>] [--track-index <n>] <file.mid|project.evil>"
        );
        return Ok(());
    }
    let instance = match SingleInstance::start(&options)? {
        InstanceStart::Primary(instance) => instance,
        InstanceStart::Secondary => return Ok(()),
    };
    let dialogs = Box::new(dialogs::DesktopDialogs);
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        accessibility::WINDOW_TITLE,
        native_options,
        Box::new(move |context| {
            Ok(Box::new(StudioApp::new(
                context,
                options,
                Some(instance),
                dialogs,
            )))
        }),
    )?;
    Ok(())
}
