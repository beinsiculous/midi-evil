//! Mechanism 1's bridge: two elements beside the canvas, written through `web-sys` with the
//! strings the native accessibility nodes carry.
//!
//! - `#midi-evil-current` holds the description of whatever egui has focused — the roll's name,
//!   the transport's readout, a standard control's `WidgetInfo` — and is the canvas's accessible
//!   description, so it is read when focus returns to the canvas and by a "where am I" command,
//!   and stays readable while focus is elsewhere on the page.
//! - `#midi-evil-live` is a polite live region that receives each change as one string.
//! - `#midi-evil-text-agent` is eframe's own text entry, adopted: while egui edits text the page's
//!   focus is there, not on the canvas, so it carries what the canvas carries — a name (the
//!   control being edited) and the current element as its description — and it is in the
//!   accessibility tree exactly while it may hold focus.
//!
//! In a frame where both change, the current element is written first and the live region
//! second, in the same task, so they reach the accessibility tree as one update and the
//! announcement never describes a state the readable element does not yet hold. An outcome's
//! notice and the name it changed are never two writes to the live region — the second would
//! replace the first before it was spoken, which is the native finding in browser form — but one
//! string, the notice first, as the native roll orders them.

use wasm_bindgen::{JsCast, prelude::*};

/// What the current element says when egui has nothing focused.
pub const NOTHING_FOCUSED: &str = "Nothing in the editor has focus; Tab or F6 moves to a control";

/// A repeated announcement is made different by a trailing no-break space, because a live region
/// whose text is set to what it already holds changes nothing and is not spoken.
const REPEAT_MARK: char = '\u{a0}';

/// The id the adopted text entry carries, so the page, the focus boundary and the probe find it.
pub const TEXT_ENTRY_ID: &str = "midi-evil-text-agent";

pub struct Bridge {
    canvas: web_sys::HtmlCanvasElement,
    current_element: web_sys::Element,
    live_element: web_sys::Element,
    text_entry: Option<web_sys::HtmlElement>,
    /// The entry's name while it is exposed; `None` while it is hidden.
    text_entry_name: Option<String>,
    current: String,
    last_announcement: String,
    repeat_marked: bool,
    attributes: Vec<(&'static str, String)>,
}

impl Bridge {
    pub fn attach(
        document: &web_sys::Document,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<Self, JsValue> {
        let find = |id: &str| {
            document
                .get_element_by_id(id)
                .ok_or_else(|| JsValue::from_str(&format!("no #{id}")))
        };
        Ok(Self {
            canvas,
            current_element: find("midi-evil-current")?,
            live_element: find("midi-evil-live")?,
            text_entry: None,
            text_entry_name: None,
            current: String::new(),
            last_announcement: String::new(),
            repeat_marked: false,
            attributes: Vec::new(),
        })
    }

    /// Writes one frame's state. `focus_moved` is a change of the widget egui has focused;
    /// `announce` is false while the canvas itself does not have the page's focus, because the
    /// screen reader is then elsewhere and reads the description when focus comes back.
    pub fn publish(
        &mut self,
        current: String,
        notice: Option<String>,
        focus_moved: bool,
        announce: bool,
    ) {
        let current_changed = current != self.current;
        if current_changed {
            self.current_element.set_text_content(Some(&current));
            self.current = current;
        }
        if !announce {
            return;
        }
        let mut parts = Vec::new();
        parts.extend(notice);
        if current_changed || focus_moved {
            parts.push(self.current.clone());
        }
        if parts.is_empty() {
            return;
        }
        let mut announcement = parts.join(" ");
        if announcement == self.last_announcement {
            self.repeat_marked = !self.repeat_marked;
        } else {
            self.repeat_marked = false;
        }
        self.last_announcement = announcement.clone();
        if self.repeat_marked {
            announcement.push(REPEAT_MARK);
        }
        self.live_element.set_text_content(Some(&announcement));
    }

    /// Exposes the text entry under `name` while egui edits text, and hides it otherwise. Called
    /// before eframe moves the page's focus to it, so it is never focused while hidden.
    pub fn set_text_entry(&mut self, name: Option<String>) {
        let Some(entry) = &self.text_entry else {
            return;
        };
        if name == self.text_entry_name {
            return;
        }
        match &name {
            Some(name) => {
                // Moved beside the canvas when first used, not when adopted: a refused focus
                // leaves the browser's Tab starting point where the entry was, and at load that
                // must be the end of the page, from which Tab goes to the page's first stop.
                let beside_canvas = self
                    .canvas
                    .next_sibling()
                    .is_some_and(|sibling| sibling == ***entry);
                if !beside_canvas && let Some(parent) = self.canvas.parent_node() {
                    let _ = parent.insert_before(entry, self.canvas.next_sibling().as_ref());
                }
                let _ = entry.set_attribute("aria-label", name);
                let _ = entry.remove_attribute("aria-hidden");
            }
            None => {
                let _ = entry.set_attribute("aria-hidden", "true");
                let _ = entry.remove_attribute("aria-label");
            }
        }
        self.text_entry_name = name;
    }

    /// Takes over eframe's text entry, a nameless `<input>` it appends to the end of `<body>`
    /// with `autofocus` for IME and phone keyboards. Left as eframe made it, three things go
    /// wrong: the page opens with focus in the editor (the input takes the autofocus and eframe
    /// hands focus on to the canvas); the input is a Tab stop after the page's last element, so
    /// Tab past the end of the page lands back in the editor; and while egui edits text — a value
    /// box reached by Tab is enough — the page's focus sits on an element with no name, outside
    /// the canvas's description. Adopted, it is out of the Tab order, described by the current
    /// element, hidden from the accessibility tree until egui edits text and then placed right
    /// after the canvas so it reads where the editor is, and it refuses any focus that arrives
    /// while it is hidden — the load-time autofocus, which removing the attribute does not
    /// withdraw, among them.
    pub fn adopt_text_entry(&mut self, document: &web_sys::Document) -> Result<(), JsValue> {
        let entry = document
            .query_selector("body > input[autocapitalize=off]")?
            .ok_or("eframe's text entry is not in the page")?
            .dyn_into::<web_sys::HtmlElement>()?;
        entry.set_id(TEXT_ENTRY_ID);
        entry.remove_attribute("autofocus")?;
        entry.set_attribute("tabindex", "-1")?;
        entry.set_attribute("aria-describedby", "midi-evil-current")?;
        entry.set_attribute("aria-hidden", "true")?;
        let guarded = entry.clone();
        let refuse_while_hidden = Closure::<dyn FnMut()>::new(move || {
            if guarded.get_attribute("aria-hidden").as_deref() == Some("true") {
                let _ = guarded.blur();
            }
        });
        entry.add_event_listener_with_callback(
            "focus",
            refuse_while_hidden.as_ref().unchecked_ref(),
        )?;
        // The listener lives as long as the page.
        refuse_while_hidden.forget();
        self.text_entry = Some(entry);
        Ok(())
    }

    /// State the probe reads off the canvas and a screen reader does not: plain data attributes.
    pub fn set_attribute(&mut self, name: &'static str, value: String) {
        let unchanged = self
            .attributes
            .iter()
            .any(|(known, previous)| *known == name && *previous == value);
        if unchanged {
            return;
        }
        let _ = self.canvas.set_attribute(name, &value);
        self.attributes.retain(|(known, _)| *known != name);
        self.attributes.push((name, value));
    }
}
