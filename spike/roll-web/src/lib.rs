//! W0-web's scratch harness: the spike's piano roll and transport readout on eframe's web
//! runner, fed by a harness-side state struct, with their strings written into the page beside
//! the canvas. Throwaway, like the spike it includes.

// The spike's own files, included unchanged so the browser runs the same roll, the same
// keyboard model and the same strings the native gate was judged on. Anything they use that this
// crate does not is unused here, not dead there.
#[allow(dead_code)]
#[path = "../../../crates/midi-evil/src/description.rs"]
mod description;
#[allow(dead_code)]
#[path = "../../../crates/midi-evil/src/piano_roll.rs"]
mod piano_roll;
#[allow(dead_code)]
#[path = "../../../crates/midi-evil/src/theme.rs"]
mod theme;
#[allow(dead_code)]
#[path = "../../../crates/midi-evil/src/transport_readout.rs"]
mod transport_readout;

#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(target_arch = "wasm32")]
mod control_text;
mod fixture;
#[cfg(target_arch = "wasm32")]
mod focus_boundary;
#[cfg(target_arch = "wasm32")]
mod harness;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use wasm_bindgen::{JsCast, prelude::*};

    use crate::{bridge::Bridge, fixture::Fixture, focus_boundary::FocusBoundary, harness};

    thread_local! {
        static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    }

    /// Starts the harness on the page's canvas. The page carries the canvas, the live region and
    /// the current-description element by id. `?fixture=empty` or `?fixture=full` chooses an
    /// edge fixture instead of the Bach invention; `?edge=value` puts a value box at egui's first
    /// stop; `?agent=eframe` leaves eframe's text entry as eframe made it, to show what it does.
    #[wasm_bindgen]
    pub async fn start() -> Result<(), JsValue> {
        eframe::WebLogger::init(log::LevelFilter::Warn).ok();
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id("midi-evil-canvas")
            .ok_or("no #midi-evil-canvas")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;
        let query = window.location().search().unwrap_or_default();
        let fixture = Fixture::from_query(&query);
        let edge_value = query.contains("edge=value");
        let keeps_text_agent = query.contains("agent=eframe");
        let loaded = crate::fixture::load(fixture).map_err(|error| JsValue::from_str(&error))?;
        let mut bridge = Bridge::attach(&document, canvas.clone())?;
        let boundary = FocusBoundary::install(&window, canvas.clone())?;

        let runner = eframe::WebRunner::new();
        runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |creation| {
                    // eframe has made its text entry by now, and has not yet focused it.
                    if keeps_text_agent {
                        log::warn!("the text entry is left as eframe made it");
                    } else {
                        bridge
                            .adopt_text_entry(&document)
                            .map_err(|error| format!("{error:?}"))?;
                    }
                    crate::theme::install(&creation.egui_ctx);
                    Ok(Box::new(harness::Harness::new(
                        loaded, edge_value, bridge, boundary,
                    )))
                }),
            )
            .await?;
        RUNNER.with(|slot| *slot.borrow_mut() = Some(runner));
        Ok(())
    }
}
