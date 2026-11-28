//! MIDI Evil in a browser: one entry, run when the module is instantiated, that finds the page's
//! canvas and starts the workstation on it, with what it says written into the page beside the
//! canvas and the page's keys and focus kept apart from the browser's. On any other target this
//! crate is empty.
#![cfg(target_arch = "wasm32")]

mod audio;
mod bridge;
#[cfg(feature = "gate")]
mod gate;
mod keyboard;
mod shell;

use std::rc::Rc;

use wasm_bindgen::{JsCast, prelude::*};

/// The id of the canvas the page gives the workstation to draw on.
const CANVAS_ID: &str = "midi-evil-canvas";

/// What the bundle was built from, as `scripts/build_identity.sh` names it; `scripts/build_wasm.sh`
/// sets it for both bundles, so the browser gate can refuse a pair built from different trees.
const BUILD_IDENTITY: &str = match option_env!("MIDI_EVIL_BUILD_IDENTITY") {
    Some(identity) => identity,
    None => "midi-evil-build-unidentified",
};

/// Writes the `log` records eframe and the workstation raise to the console, warnings and worse.
/// An error is written as a console error: eframe's own web logger writes it as a warning, which
/// nothing watching for errors would see.
struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = format!("[{}] {}", record.target(), record.args()).into();
        if record.level() == log::Level::Error {
            web_sys::console::error_1(&message);
        } else {
            web_sys::console::warn_1(&message);
        }
    }

    fn flush(&self) {}
}

static CONSOLE_LOGGER: ConsoleLogger = ConsoleLogger;

/// Without the panic hook and the console logger, a panic or a failure inside eframe in a
/// browser leaves no trace at all; both write to the console as errors.
#[wasm_bindgen(start)]
fn start() {
    std::panic::set_hook(Box::new(|panic| {
        web_sys::console::error_1(&format!("MIDI Evil panicked: {panic}").into());
    }));
    match log::set_logger(&CONSOLE_LOGGER) {
        Ok(()) => log::set_max_level(log::LevelFilter::Warn),
        Err(error) => {
            web_sys::console::error_1(&format!("MIDI Evil's console logger: {error}").into());
        }
    }
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = start_on_canvas().await {
            web_sys::console::error_2(&"MIDI Evil did not start:".into(), &error);
        }
    });
}

/// Starts the workstation with the launch a desktop gets with no arguments: no file, no single
/// instance to answer to, dialogs that choose nothing, and previews played on the page's own
/// audio.
async fn start_on_canvas() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("the page has no window")?;
    let document = window.document().ok_or("the page has no document")?;
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| format!("the page has no element with the id {CANVAS_ID}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("the element with the id {CANVAS_ID} is not a canvas"))?;
    canvas.set_attribute("data-build-identity", BUILD_IDENTITY)?;
    let mut bridge = bridge::Bridge::attach(&document, canvas.clone())?;
    let shared = Rc::new(keyboard::Shared::default());
    // Installed before eframe's own listeners, which it adds to the canvas when it starts; a
    // window listener in the capture phase runs before them all the same.
    keyboard::install(&window, &canvas, &shared)?;
    #[cfg(feature = "gate")]
    let gate_canvas = canvas.clone();
    // The runner's event handlers hold their own references to it, so it keeps running after
    // this handle is dropped.
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |creation| {
                // eframe has made its text entry by now, and has not yet focused it.
                bridge
                    .adopt_text_entry(&document)
                    .map_err(|error| format!("{error:?}"))?;
                // The frame's own accessibility tree is what the page is told from; with no
                // adapter attached, eframe builds none unless asked.
                creation.egui_ctx.enable_accesskit();
                #[cfg(not(feature = "gate"))]
                let app = midi_evil::StudioApp::new(
                    creation,
                    midi_evil::LaunchOptions::default(),
                    None,
                    Box::new(()),
                    Box::new(audio::PageOutput),
                );
                #[cfg(feature = "gate")]
                let app = gate::application(creation, &gate_canvas)
                    .map_err(|error| format!("{error:?}"))?;
                Ok(Box::new(shell::Shell::new(app, bridge, shared)))
            }),
        )
        .await
}
