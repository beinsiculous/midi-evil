//! What a standard egui control is called and how it is described, from the `WidgetInfo` egui
//! raises for it. A control egui leaves unnamed is said to be unnamed, never given the words of
//! the control before it.

use eframe::egui::WidgetInfo;

/// "drag value", "button", "slider": the control's kind in egui's own words, or empty for a
/// label or an unclassified widget.
fn kind(info: &WidgetInfo) -> String {
    WidgetInfo::new(info.typ).description()
}

fn unnamed(info: Option<&WidgetInfo>) -> String {
    match info.map(kind).filter(|kind| !kind.is_empty()) {
        Some(kind) => format!("unnamed {kind}"),
        None => "unnamed control".to_owned(),
    }
}

/// The current description of a focused control: egui's own, or "unnamed …" when egui gives
/// none — or when no event described the control at all.
pub fn description(info: Option<&WidgetInfo>) -> String {
    match info.map(WidgetInfo::description) {
        Some(described) if !described.is_empty() => described,
        _ => unnamed(info),
    }
}

/// The accessible name for the text entry while it edits this control: its label, or "unnamed
/// …".
pub fn name(info: Option<&WidgetInfo>) -> String {
    match info.and_then(|info| info.label.clone()) {
        Some(label) if !label.trim().is_empty() => label,
        _ => unnamed(info),
    }
}
