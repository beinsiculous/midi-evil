//! Lets Tab leave the canvas. eframe's web input prevents the default of every Tab the canvas
//! receives (`should_prevent_default_for_key`, which no option overrides) and egui wraps its own
//! traversal at both ends, so without this a keyboard user who Tabs into the canvas never Tabs
//! out. When egui's focus is on its last focusable widget, a Tab is stopped at the window before
//! eframe sees it, and the browser's default moves focus to the page's next element; Shift+Tab
//! from the first widget likewise. The page's focus may be on the canvas or, while egui edits
//! text, on the adopted text entry.

use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, prelude::*};

use crate::bridge::TEXT_ENTRY_ID;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub at_first: bool,
    pub at_last: bool,
}

pub struct FocusBoundary {
    edges: Rc<Cell<Edges>>,
}

impl FocusBoundary {
    /// Listens on the window in the capture phase, so it runs before eframe's listener on the
    /// canvas and can keep the event from it.
    pub fn install(
        window: &web_sys::Window,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<Self, JsValue> {
        let edges = Rc::new(Cell::new(Edges::default()));
        let shared = Rc::clone(&edges);
        let document = window.document().ok_or("no document")?;
        let on_keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                if event.key() != "Tab" || event.ctrl_key() || event.alt_key() || event.meta_key() {
                    return;
                }
                let canvas_element: &web_sys::Element = canvas.unchecked_ref();
                let Some(active) = document.active_element() else {
                    return;
                };
                let in_text_entry = active.id() == TEXT_ENTRY_ID;
                if active != *canvas_element && !in_text_entry {
                    return;
                }
                let edges = shared.get();
                let leaving = if event.shift_key() {
                    edges.at_first
                } else {
                    edges.at_last
                };
                if !leaving {
                    return;
                }
                event.stop_immediate_propagation();
                // The browser's next stop is found from the element focused when the key's
                // default action runs. The text entry sits right after the canvas, so from there
                // Shift+Tab would land on the canvas; focus is put on the canvas first, and the
                // browser leaves from it in either direction.
                if in_text_entry {
                    let _ = canvas.focus();
                }
            },
        );
        window.add_event_listener_with_callback_and_bool(
            "keydown",
            on_keydown.as_ref().unchecked_ref(),
            true,
        )?;
        // The listener lives as long as the page.
        on_keydown.forget();
        Ok(Self { edges })
    }

    pub fn set(&self, edges: Edges) {
        self.edges.set(edges);
    }
}

/// The first and last widgets egui's Tab reaches, in the order it reaches them: egui offers focus
/// to widgets in the order they are created in the background layer.
pub fn tab_ends(
    context: &eframe::egui::Context,
) -> (Option<eframe::egui::Id>, Option<eframe::egui::Id>) {
    context.viewport(|viewport| {
        let mut focusable = viewport
            .this_pass
            .widgets
            .get_layer(eframe::egui::LayerId::background())
            .filter(|widget| widget.enabled && widget.sense.is_focusable())
            .map(|widget| widget.id);
        let first = focusable.next();
        let last = focusable.last().or(first);
        (first, last)
    })
}
