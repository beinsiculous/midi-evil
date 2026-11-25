//! MIDI Evil in a browser: one entry, run when the module is instantiated, that finds the page's
//! canvas and starts the workstation on it. On any other target this crate is empty.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen::{JsCast, prelude::*};

/// The id of the canvas the page gives the workstation to draw on.
const CANVAS_ID: &str = "midi-evil-canvas";

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
/// instance to answer to, and dialogs that choose nothing.
async fn start_on_canvas() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("the page has no document")?;
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| format!("the page has no element with the id {CANVAS_ID}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("the element with the id {CANVAS_ID} is not a canvas"))?;
    // The runner's event handlers hold their own references to it, so it keeps running after
    // this handle is dropped.
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|creation| {
                Ok(Box::new(midi_evil::StudioApp::new(
                    creation,
                    midi_evil::LaunchOptions::default(),
                    None,
                    Box::new(()),
                )))
            }),
        )
        .await
}
