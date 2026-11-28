//! What the page does with a key before eframe sees it, and with focus that arrives from the page.
//!
//! eframe cancels the browser's default for every key it is handed that it has a use for, and
//! wraps Tab inside the canvas, so three things are decided here, in a listener that runs before
//! eframe's because it listens on the window in the capture phase:
//!
//! - **Tab leaves at the ends.** At egui's last stop a Tab, and at its first a Shift+Tab, is kept
//!   from eframe, and the browser moves focus on.
//! - **A chord the application does not take is the browser's.** One with Control, Alt or the
//!   logo key, or a function key, that this frame's table does not hold is kept from eframe and
//!   not cancelled, so the browser does what it does with it. Plain keys and Shift are left alone,
//!   so typing is.
//! - **A key the application takes is not the browser's too.** Outside a text field, its default
//!   is cancelled, so Home, End, Enter, Delete and the region chord, which eframe lets through,
//!   scroll nothing.
//!
//! And because a chord now leaves the page, the page asks before it is left with unsaved work: when
//! the frame said so, or when any input has reached the editor since the last frame ended, so an
//! edit and a chord that leaves, pressed together, cannot slip through the frame between them.
//!
//! Focus that arrives from the page comes through a sentinel before or after the canvas, and
//! enters at egui's first or last stop by the direction it came from.
//!
//! Every key and every pointer press, wherever on the page, wakes the page's audio first: a
//! browser lets a page start its sound only inside such an event's own handler.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use eframe::egui;
use wasm_bindgen::{JsCast, prelude::*};

use crate::bridge::TEXT_ENTRY_ID;

/// Which end of Tab's walk focus arriving from the page enters at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    First,
    Last,
}

/// What the last frame said that the listeners act on, and what they leave for the next frame.
#[derive(Default)]
pub struct Shared {
    pub keys: RefCell<Option<midi_evil::KeyTable>>,
    pub at_first_stop: Cell<bool>,
    pub at_last_stop: Cell<bool>,
    pub editing_text: Cell<bool>,
    pub unsaved: Cell<bool>,
    /// Set by any input that reaches the editor, cleared when a frame has taken it in.
    pub input_since_frame: Cell<bool>,
    pub entering: Cell<Option<Entry>>,
}

impl Shared {
    /// Records what a finished frame said.
    pub fn frame_finished(&self, frame: &midi_evil::SpokenFrame) {
        self.keys.replace(Some(frame.keys.clone()));
        self.at_first_stop.set(frame.at_first_stop);
        self.at_last_stop.set(frame.at_last_stop);
        self.editing_text.set(
            frame
                .focused
                .as_ref()
                .is_some_and(|control| control.is_text_entry),
        );
        self.unsaved.set(frame.unsaved);
        self.input_since_frame.set(false);
    }

    fn unsaved_now(&self) -> bool {
        self.unsaved.get() || self.input_since_frame.get()
    }
}

/// The element ids the keyboard and the focus boundary use.
const ENTER_FIRST_ID: &str = "midi-evil-enter-first";
const ENTER_LAST_ID: &str = "midi-evil-enter-last";
const LEAVE_ID: &str = "midi-evil-leave";
const HEADING_ID: &str = "midi-evil-heading";

fn in_editor(element: Option<web_sys::Element>, canvas: &web_sys::HtmlCanvasElement) -> bool {
    let canvas: &web_sys::Element = canvas.unchecked_ref();
    element.is_some_and(|element| element == *canvas || element.id() == TEXT_ENTRY_ID)
}

fn is_function_key(key: egui::Key) -> bool {
    use egui::Key;
    matches!(
        key,
        Key::F1
            | Key::F2
            | Key::F3
            | Key::F4
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12
            | Key::F13
            | Key::F14
            | Key::F15
            | Key::F16
            | Key::F17
            | Key::F18
            | Key::F19
            | Key::F20
    )
}

/// Every listener lives as long as the page.
fn listen<Event: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    capture: bool,
    handler: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let closure = Closure::<dyn FnMut(Event)>::new(handler);
    target.add_event_listener_with_callback_and_bool(
        name,
        closure.as_ref().unchecked_ref(),
        capture,
    )?;
    closure.forget();
    Ok(())
}

pub fn install(
    window: &web_sys::Window,
    canvas: &web_sys::HtmlCanvasElement,
    shared: &Rc<Shared>,
) -> Result<(), JsValue> {
    let document = window.document().ok_or("the page has no document")?;
    install_keys(window, &document, canvas, shared)?;
    install_input_marks(window, &document, canvas, shared)?;
    install_leaving(window, shared)?;
    install_sentinels(&document, canvas, shared)
}

fn install_keys(
    window: &web_sys::Window,
    document: &web_sys::Document,
    canvas: &web_sys::HtmlCanvasElement,
    shared: &Rc<Shared>,
) -> Result<(), JsValue> {
    let (document, canvas, shared) = (document.clone(), canvas.clone(), Rc::clone(shared));
    listen(
        window,
        "keydown",
        true,
        move |event: web_sys::KeyboardEvent| {
            crate::audio::wake();
            let active = document.active_element();
            if !in_editor(active.clone(), &canvas) {
                return;
            }
            let in_text_entry = active.is_some_and(|active| active.id() == TEXT_ENTRY_ID);
            let (control, alt, logo, shift) = (
                event.ctrl_key(),
                event.alt_key(),
                event.meta_key(),
                event.shift_key(),
            );
            let key = event.key();
            if key == "Tab" && !control && !alt && !logo {
                let leaving = if shift {
                    shared.at_first_stop.get()
                } else {
                    shared.at_last_stop.get()
                };
                if leaving {
                    event.stop_immediate_propagation();
                    // The browser finds the next stop from the element focused when the key's
                    // default runs; the text entry sits right after the canvas, so from there
                    // Shift+Tab would land on the canvas. Leaving from the canvas works both ways.
                    if in_text_entry {
                        let _ = canvas.focus();
                    }
                } else {
                    shared.input_since_frame.set(true);
                }
                return;
            }
            let Some(egui_key) = egui::Key::from_name(&key) else {
                shared.input_since_frame.set(true);
                return;
            };
            // Outside a field the table is for a Control keyboard; in one, the command key is
            // read as eframe reads it, Control or the logo key, so a Mac's Command+A selects.
            let editing = shared.editing_text.get();
            let modifiers = egui::Modifiers {
                alt,
                ctrl: control,
                shift,
                mac_cmd: logo,
                command: control || (logo && editing),
            };
            let held = shared
                .keys
                .borrow()
                .as_ref()
                .is_some_and(|keys| keys.holds(egui_key, modifiers));
            if held {
                shared.input_since_frame.set(true);
                if !shared.editing_text.get() {
                    event.prevent_default();
                }
            } else if control || alt || logo || is_function_key(egui_key) {
                event.stop_immediate_propagation();
            } else {
                shared.input_since_frame.set(true);
            }
        },
    )
}

/// A paste, a composed character and the pointer reach the editor without a key the table would
/// see. A pointer press anywhere on the page also wakes the page's audio, as any key does.
fn install_input_marks(
    window: &web_sys::Window,
    document: &web_sys::Document,
    canvas: &web_sys::HtmlCanvasElement,
    shared: &Rc<Shared>,
) -> Result<(), JsValue> {
    for name in ["paste", "cut", "input", "compositionend"] {
        let (document, canvas, shared) = (document.clone(), canvas.clone(), Rc::clone(shared));
        listen(window, name, true, move |_: web_sys::Event| {
            if in_editor(document.active_element(), &canvas) {
                shared.input_since_frame.set(true);
            }
        })?;
    }
    // A value dragged or a part moved by the pointer is an edit with no key and no input event.
    // eframe runs a frame inside a press and a release, which takes a click in at once; a move
    // with a button held it takes in at the next frame, so a drag arms the mark until then. The
    // press lands on the canvas before it has the page's focus, hence the target.
    for name in ["pointerdown", "pointermove", "pointerup"] {
        let (canvas, shared) = (canvas.clone(), Rc::clone(shared));
        listen(window, name, true, move |event: web_sys::MouseEvent| {
            if name == "pointerdown" {
                crate::audio::wake();
            }
            let target = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok());
            let dragging = name != "pointermove" || event.buttons() != 0;
            if dragging && in_editor(target, &canvas) {
                shared.input_since_frame.set(true);
            }
        })?;
    }
    Ok(())
}

fn install_leaving(window: &web_sys::Window, shared: &Rc<Shared>) -> Result<(), JsValue> {
    let shared = Rc::clone(shared);
    listen(
        window,
        "beforeunload",
        false,
        move |event: web_sys::BeforeUnloadEvent| {
            if shared.unsaved_now() {
                event.prevent_default();
                // What older Chromium reads instead; its text is never shown.
                event.set_return_value("unsaved");
            }
        },
    )
}

/// The sentinels before and after the canvas, and the standing control after them. The
/// sentinels are Tab stops only while focus is outside the editor, so Tab leaving the editor
/// passes them by and Tab arriving from the page lands on one.
fn install_sentinels(
    document: &web_sys::Document,
    canvas: &web_sys::HtmlCanvasElement,
    shared: &Rc<Shared>,
) -> Result<(), JsValue> {
    let find = |id: &str| {
        document
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("the page has no element with the id {id}")))
    };
    let sentinels = [find(ENTER_FIRST_ID)?, find(ENTER_LAST_ID)?];
    for (sentinel, entry) in sentinels.iter().zip([Entry::First, Entry::Last]) {
        let (canvas, shared) = (canvas.clone(), Rc::clone(shared));
        listen(sentinel, "focus", false, move |_: web_sys::FocusEvent| {
            shared.entering.set(Some(entry));
            let _ = canvas.focus();
        })?;
    }
    let canvas = canvas.clone();
    listen(
        document,
        "focusin",
        false,
        move |event: web_sys::FocusEvent| {
            let target = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok());
            let index = if in_editor(target, &canvas) {
                "-1"
            } else {
                "0"
            };
            for sentinel in &sentinels {
                let _ = sentinel.set_attribute("tabindex", index);
            }
        },
    )?;
    let heading = find(HEADING_ID)?.dyn_into::<web_sys::HtmlElement>()?;
    let leave: web_sys::EventTarget = find(LEAVE_ID)?.into();
    listen(&leave, "click", false, move |_: web_sys::MouseEvent| {
        let _ = heading.focus();
    })
}
