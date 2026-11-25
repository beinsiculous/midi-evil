//! Names for what egui leaves anonymous, and reasons for what it leaves unexplained. A control's
//! accessible name is set on its own node, not left to a caption beside it, because on the web
//! that node is what names the text entry eframe edits through. A disabled button is reported to
//! a screen reader as available, so while it is disabled it carries why, as its description and
//! as its hover text, and a sighted person reads the same reason.

use eframe::egui::{self, accesskit};

use crate::speech::{self, Unavailable};

/// The window's title, which its accessibility tree's root carries too.
pub const WINDOW_TITLE: &str = "MIDI Evil";

pub trait Accessible: Sized {
    /// Sets the control's accessible name.
    fn named(self, name: impl Into<String>) -> Self;

    /// Sets the role and the name of a widget egui gives neither, such as a painted lane.
    fn named_as(self, role: accesskit::Role, name: impl Into<String>) -> Self;

    /// While the control is disabled, says why.
    fn unavailable_because(self, reason: Unavailable) -> Self;

    /// The first reason that holds, if any; a control enabled says nothing.
    fn unavailable_because_of(self, reasons: &[(bool, Unavailable)]) -> Self;
}

impl Accessible for egui::Response {
    fn named(self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_label(name));
        self
    }

    fn named_as(self, role: accesskit::Role, name: impl Into<String>) -> Self {
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_role(role));
        self.named(name)
    }

    fn unavailable_because(self, reason: Unavailable) -> Self {
        if self.enabled() {
            return self;
        }
        let text = speech::unavailable_reason(reason);
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_description(text));
        self.on_disabled_hover_text(text)
    }

    fn unavailable_because_of(self, reasons: &[(bool, Unavailable)]) -> Self {
        match reasons.iter().find(|(holds, _)| *holds) {
            Some((_, reason)) => self.unavailable_because(*reason),
            None => self,
        }
    }
}

/// Exposes the window's title on the tree's root, which egui leaves unnamed.
pub fn name_window(context: &egui::Context) {
    context.accesskit_node_builder(egui::accesskit_root_id(), |node| {
        node.set_label(WINDOW_TITLE);
    });
}

/// Names the scroll bars egui drew this frame for the scroll area with this id: "Piano roll,
/// vertical scroll bar". A bar not drawn this frame is left alone, so no empty node is made.
pub fn name_scroll_bars(context: &egui::Context, scroll_area: egui::Id, what: &str) {
    for (axis, direction) in [(0, "horizontal"), (1, "vertical")] {
        let bar = scroll_area.with(axis);
        if drawn_this_frame(context, bar) {
            context.accesskit_node_builder(bar, |node| {
                node.set_label(format!("{what}, {direction} scroll bar"));
            });
        }
    }
}

/// Names the resize handle egui draws on the edge of the panel with this id. It is a Tab stop
/// that only a pointer can drag, so the name says what it is for.
pub fn name_resize_handle(context: &egui::Context, panel: egui::Id, name: &str) {
    let handle = panel.with("__resize");
    if drawn_this_frame(context, handle) {
        context.accesskit_node_builder(handle, |node| {
            node.set_role(accesskit::Role::Splitter);
            node.set_label(name);
        });
    }
}

fn drawn_this_frame(context: &egui::Context, widget: egui::Id) -> bool {
    context.viewport(|viewport| viewport.this_pass.widgets.get(widget).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_of(output: &egui::FullOutput, id: egui::Id) -> Option<String> {
        output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(node_id, _)| *node_id == id.accesskit_id())
            .and_then(|(_, node)| node.label().map(str::to_owned))
    }

    fn description_of(output: &egui::FullOutput, id: egui::Id) -> Option<String> {
        output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(node_id, _)| *node_id == id.accesskit_id())
            .and_then(|(_, node)| node.description().map(str::to_owned))
    }

    #[test]
    fn a_disabled_button_says_why_and_an_enabled_one_says_nothing() {
        let context = egui::Context::default();
        context.enable_accesskit();
        for enabled in [false, true] {
            let mut button = egui::Id::NULL;
            let output = context.run_ui(egui::RawInput::default(), |ui| {
                button = ui
                    .add_enabled(enabled, egui::Button::new("Undo"))
                    .unavailable_because(Unavailable::NothingToUndo)
                    .id;
            });
            let wanted = (!enabled).then(|| "Nothing to undo.".to_owned());
            assert_eq!(
                description_of(&output, button),
                wanted,
                "enabled: {enabled}"
            );
        }
    }

    #[test]
    fn a_value_box_a_text_field_the_window_and_the_scroll_bars_are_named() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut value = 64.0;
        let mut text = String::from("Song");
        let (mut value_box, mut field, mut scroll_area) =
            (egui::Id::NULL, egui::Id::NULL, egui::Id::NULL);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            value_box = ui
                .add(egui::DragValue::new(&mut value))
                .named("Tempo in beats per minute")
                .id;
            field = ui
                .add(egui::TextEdit::singleline(&mut text))
                .named("Project name")
                .id;
            scroll_area = egui::ScrollArea::vertical()
                .max_height(20.0)
                .show(ui, |ui| {
                    for line in 0..20 {
                        ui.label(format!("line {line}"));
                    }
                })
                .id;
            name_scroll_bars(ui.ctx(), scroll_area, "Lines");
            name_window(ui.ctx());
        });
        assert_eq!(
            label_of(&output, value_box).as_deref(),
            Some("Tempo in beats per minute")
        );
        assert_eq!(label_of(&output, field).as_deref(), Some("Project name"));
        assert_eq!(
            label_of(&output, scroll_area.with(1)).as_deref(),
            Some("Lines, vertical scroll bar")
        );
        assert_eq!(
            label_of(&output, scroll_area.with(0)),
            None,
            "a bar never drawn gets no node"
        );
        assert_eq!(
            label_of(&output, egui::accesskit_root_id()).as_deref(),
            Some(WINDOW_TITLE)
        );
    }
}
