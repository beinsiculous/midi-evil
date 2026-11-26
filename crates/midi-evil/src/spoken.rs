//! What the workstation says, as one value a frame, for a front end that speaks by writing text —
//! a page's live region and the element that describes its canvas — rather than by handing a
//! screen reader an accessibility tree. Every word in it is the library's; a front end decides
//! only where and when to write it.

use eframe::egui;

pub use crate::key_help::KeyTable;

/// The control egui's focus is on, read from the node the frame built for it — the node a desktop
/// screen reader is given — with its role and states in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedControl {
    pub id: egui::Id,
    pub name: String,
    /// Help or, while the control is unavailable, why.
    pub description: String,
    /// What a field holds, the text being typed into it included.
    pub value: String,
    /// "button", "check box"; empty for the application's own editors, whose names say what
    /// they are.
    pub role: String,
    /// "checked", "unavailable", "menu open".
    pub states: Vec<String>,
    /// Whether egui is editing text in it, so a page's keys go to a text entry.
    pub is_text_entry: bool,
    /// Whether it holds text that can be selected and copied, which a text that cannot be
    /// edited — the keyboard notes — does as well as a field that can.
    pub holds_selectable_text: bool,
}

/// What happened this frame that a listener is told about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Happened {
    /// What a command did, as the status line says it; several in one action are one string.
    pub outcome: Option<String>,
    /// egui's focus is on another control than the one a listener was last told about.
    pub focus_moved: bool,
    /// The focused control's name, value or state changed with no outcome to say so.
    pub control_changed: bool,
    /// A text field's edit ended with Enter, keeping what was typed.
    pub text_committed: bool,
}

/// One frame, as a front end that writes text needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenFrame {
    pub focused: Option<FocusedControl>,
    /// What a page's current element holds: the focused control in full, or, with nothing
    /// focused, how to reach a control. True every frame, whatever is announced.
    pub current: String,
    pub happened: Happened,
    /// The one string to announce this frame, notice first: `None` while nothing is to be said,
    /// while a key is held, and while a command's focus move has not yet arrived.
    pub announcement: Option<String>,
    /// Whether egui's focus is on the first or the last control its Tab reaches this frame, so a
    /// Tab or a Shift+Tab leaves the editor.
    pub at_first_stop: bool,
    pub at_last_stop: bool,
    /// The first and last controls Tab reaches this frame, for focus that arrives from the page.
    pub first_stop: Option<egui::Id>,
    pub last_stop: Option<egui::Id>,
    /// Whether the document holds anything not yet saved.
    pub unsaved: bool,
    /// The chords the application takes this frame; a page hands every other chord with Control,
    /// Alt or the logo key, and every function key, to the browser.
    pub keys: KeyTable,
}
