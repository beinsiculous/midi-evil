//! The page's half of what the workstation says: the element that describes the canvas, the live
//! region, and eframe's own text entry, adopted. Every word written here is the library's; this
//! module decides only where and when.
//!
//! - The current element holds the focused control in full, every frame, so a screen reader that
//!   comes back to the canvas, or asks where it is, reads what is true now.
//! - The live region is polite and atomic, written at most once a frame and only while the page's
//!   focus is in the editor: on the canvas, or on the text entry while a field is edited. The
//!   current element is written first, in the same task, so an announcement never describes a
//!   state the readable element does not yet hold.
//! - The text entry is the `<input>` eframe types through. While a field is edited it is named for
//!   that field, described by the current element and placed right after the canvas; otherwise it
//!   is hidden from the tree and refuses focus.

use wasm_bindgen::{JsCast, prelude::*};

/// A live region whose text is set to what it already holds announces nothing, so a repeated
/// announcement ends in a no-break space, and the next repeat drops it again: each of any number
/// of identical announcements differs from the one before.
const REPEAT_MARK: char = '\u{a0}';

/// The id the adopted text entry carries, so the page, the keyboard and the gate find it.
pub const TEXT_ENTRY_ID: &str = "midi-evil-text-entry";

pub struct Bridge {
    canvas: web_sys::HtmlCanvasElement,
    current_element: web_sys::Element,
    live_element: web_sys::Element,
    text_entry: Option<web_sys::HtmlElement>,
    /// The text entry's name while it is exposed.
    text_entry_name: Option<String>,
    current: String,
    last_written: String,
    repeat_marked: bool,
    /// Whether the page's focus was in the editor when the last frame was published.
    page_focus_was_in_editor: bool,
    attributes: Vec<(&'static str, String)>,
}

impl Bridge {
    pub fn attach(
        document: &web_sys::Document,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<Self, JsValue> {
        let find = |id: &str| {
            document.get_element_by_id(id).ok_or_else(|| {
                JsValue::from_str(&format!("the page has no element with the id {id}"))
            })
        };
        Ok(Self {
            canvas,
            current_element: find("midi-evil-current")?,
            live_element: find("midi-evil-live")?,
            text_entry: None,
            text_entry_name: None,
            current: String::new(),
            last_written: String::new(),
            repeat_marked: false,
            page_focus_was_in_editor: false,
            attributes: Vec::new(),
        })
    }

    /// Whether the page's focus is on the canvas or the text entry.
    pub fn page_focus_in_editor(&self) -> bool {
        let active = self
            .canvas
            .owner_document()
            .and_then(|document| document.active_element());
        active.is_some_and(|active| {
            let canvas: &web_sys::Element = self.canvas.unchecked_ref();
            active == *canvas || active.id() == TEXT_ENTRY_ID
        })
    }

    /// Writes one frame. `entered` is a frame in which focus came into the editor from the
    /// page: a screen reader reads the canvas's name and description by itself, so nothing is
    /// announced.
    pub fn publish(
        &mut self,
        current: &str,
        announcement: Option<&str>,
        entered: bool,
        carries_an_outcome: bool,
    ) {
        if current != self.current {
            self.current_element.set_text_content(Some(current));
            current.clone_into(&mut self.current);
        }
        let in_editor = self.page_focus_in_editor();
        let returned = in_editor && !self.page_focus_was_in_editor;
        self.page_focus_was_in_editor = in_editor;
        let Some(announcement) = announcement else {
            return;
        };
        // Arriving is not announced: the screen reader reads the canvas's description itself.
        // But a click from outside the editor arrives and acts in one frame, and what it did
        // has been told to nobody yet.
        let only_arrived = (returned || entered) && !carries_an_outcome;
        if !in_editor || only_arrived {
            return;
        }
        self.repeat_marked = announcement == self.last_written && !self.repeat_marked;
        announcement.clone_into(&mut self.last_written);
        let mut written = announcement.to_owned();
        if self.repeat_marked {
            written.push(REPEAT_MARK);
        }
        self.live_element.set_text_content(Some(&written));
    }

    /// Exposes the text entry under `name` while a field is edited, and hides it otherwise; when
    /// an edit ends with the entry still focused, the page's focus goes back to the canvas, so
    /// the next key reaches the application. Called before eframe moves the page's focus to the
    /// entry, so it is never focused while hidden.
    pub fn set_text_entry(&mut self, name: Option<&str>) {
        let Some(entry) = &self.text_entry else {
            return;
        };
        if name == self.text_entry_name.as_deref() {
            return;
        }
        match name {
            Some(name) => {
                // Moved beside the canvas when first used, not when adopted: a refused focus
                // leaves the browser's Tab starting point where the entry was.
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
                let focused_here = entry
                    .owner_document()
                    .and_then(|document| document.active_element())
                    .is_some_and(|active| active.id() == TEXT_ENTRY_ID);
                if focused_here {
                    let _ = self.canvas.focus();
                }
            }
        }
        self.text_entry_name = name.map(str::to_owned);
    }

    /// Takes over eframe's text entry, a nameless `<input>` it appends to the end of `<body>`
    /// with `autofocus`. Left as eframe made it, the page opens with focus in the editor, Tab
    /// past the end of the page lands back in it, and while a field is edited the page's focus
    /// sits on an element with no name. Adopted, it is out of the Tab order, described by the
    /// current element, hidden from the tree until a field is edited, and it refuses any focus
    /// that arrives while it is hidden — the load-time autofocus, which removing the attribute
    /// does not withdraw, among them.
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

    /// State a gate reads off the canvas and a screen reader does not: plain data attributes.
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
