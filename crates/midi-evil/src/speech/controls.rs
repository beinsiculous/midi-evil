//! What a front end that speaks by writing text says about a control: its role and its states in
//! words, and the sentences it writes. A desktop screen reader composes these from the
//! accessibility tree itself and needs none of them; a page's live region cannot, so they are
//! written here, once.

use eframe::egui::accesskit::{Node, Role, Toggled};

use crate::key_help;
use crate::platform::Platform;

/// "button", "check box": a role as a person says it. The application's own editors and what
/// egui leaves unclassified get none, because their names say what they are.
pub fn role_word(node: &Node) -> &'static str {
    match node.role() {
        Role::Button if node.toggled().is_some() => "toggle button",
        Role::Button => "button",
        Role::CheckBox => "check box",
        Role::RadioButton => "radio button",
        Role::ComboBox => "combo box",
        Role::TextInput => "text field",
        Role::MultilineTextInput => "text area",
        Role::SpinButton => "spin button",
        Role::Slider => "slider",
        Role::Status => "status",
        Role::Menu => "menu",
        Role::MenuItem => "menu item",
        Role::Link => "link",
        Role::Splitter => "splitter",
        Role::ScrollBar => "scroll bar",
        Role::Tab => "tab",
        Role::Window => "window",
        _ => "",
    }
}

/// "checked", "unavailable", "menu open": a control's states in words, in the order they are
/// said.
pub fn state_words(node: &Node, menu_open: bool) -> Vec<&'static str> {
    let mut words = Vec::new();
    match (node.role(), node.toggled()) {
        (Role::CheckBox | Role::RadioButton, Some(Toggled::True)) => words.push("checked"),
        (Role::CheckBox | Role::RadioButton, Some(Toggled::False)) => words.push("not checked"),
        (_, Some(Toggled::Mixed)) => words.push("partly checked"),
        (_, Some(Toggled::True)) => words.push("pressed"),
        (_, Some(Toggled::False)) => words.push("not pressed"),
        (_, None) => {}
    }
    if node.is_selected() == Some(true) {
        words.push("selected");
    }
    if menu_open {
        words.push("menu open");
    }
    if node.is_read_only() {
        words.push("read only");
    }
    if node.is_disabled() {
        words.push("unavailable");
    }
    words
}

/// "0.8": a number as a control with no text of its own is read.
pub fn number(value: f64) -> String {
    super::decimal(value)
}

/// "Undo, button, unavailable": a control as it is announced when focus arrives — its name,
/// role, states and value, without its help.
pub fn describe_control(name: &str, role: &str, states: &[String], value: &str) -> String {
    let mut parts: Vec<&str> = vec![name];
    parts.push(role);
    parts.extend(states.iter().map(String::as_str));
    if value != name {
        parts.push(value);
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// "Undo, button, unavailable. Nothing to undo.": a control in full, as an element a person can
/// read again at any time holds it.
pub fn describe_control_in_full(
    name: &str,
    role: &str,
    states: &[String],
    value: &str,
    description: &str,
) -> String {
    let control = describe_control(name, role, states, value);
    if description.is_empty() {
        control
    } else {
        format!("{control}. {description}")
    }
}

/// "Tempo in beats per minute, 130": a value kept when a field's edit ended.
pub fn committed(name: &str, value: &str) -> String {
    describe_control(name, "", &[], value)
}

/// What is said with nothing in the editor focused, which is how a person finds a control.
pub fn nothing_focused(platform: Platform) -> String {
    format!(
        "Nothing in the editor has focus; Tab or {} moves to a control.",
        key_help::next_region_chord(platform)
    )
}

/// One action's announcement: what it did, then what it left focused, as one string, so that
/// neither replaces the other before it is heard.
pub fn announcement(outcomes: &[String], control: Option<String>) -> Option<String> {
    let parts: Vec<String> = outcomes.iter().cloned().chain(control).collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}
