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

    /// While the control is disabled, says why.
    fn unavailable_because(self, reason: Unavailable) -> Self;
}

impl Accessible for egui::Response {
    fn named(self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_label(name));
        self
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
}

/// A control added unavailable while `refused` holds a reason, with what its activation means.
pub struct Refusable {
    pub response: egui::Response,
    refused: Option<Unavailable>,
    /// Refused and still reachable: the hover text is the reason, and stays the reason.
    reachable: bool,
}

impl Refusable {
    /// Added while unavailable: on the desktop, disabled and saying why, as every disabled control
    /// is. In the browser a canvas offers a screen reader no review of the window to find a
    /// disabled control by, and egui takes one out of the Tab order, so there it stays a Tab
    /// stop, looks as a disabled one does, is described as unavailable with its reason, and does
    /// nothing when activated but say the reason again.
    pub fn add(ui: &mut egui::Ui, refused: Option<Unavailable>, widget: impl egui::Widget) -> Self {
        Self::add_with(ui, refused, |ui, enabled| ui.add_enabled(enabled, widget))
    }

    /// The same, at exactly this size, as `Ui::add_sized` lays a widget out.
    pub fn add_sized(
        ui: &mut egui::Ui,
        size: [f32; 2],
        refused: Option<Unavailable>,
        widget: impl egui::Widget,
    ) -> Self {
        Self::add_with(ui, refused, |ui, enabled| {
            if enabled {
                ui.add_sized(size, widget)
            } else {
                ui.add_enabled_ui(false, |ui| ui.add_sized(size, widget))
                    .inner
            }
        })
    }

    /// `add` is told whether to add the widget enabled.
    fn add_with(
        ui: &mut egui::Ui,
        refused: Option<Unavailable>,
        add: impl FnOnce(&mut egui::Ui, bool) -> egui::Response,
    ) -> Self {
        let reachable = refused.is_some()
            && crate::platform::of(ui.ctx()).keeps_unavailable_controls_reachable();
        let response = match refused {
            Some(reason) if reachable => {
                // Faded as `add_enabled` fades a disabled widget, without a scope of its own,
                // so the widget's id is the one it has on the desktop.
                let opacity = ui.opacity();
                ui.multiply_opacity(ui.visuals().disabled_alpha());
                let response = add(ui, true);
                ui.set_opacity(opacity);
                let text = speech::unavailable_reason(reason);
                response.ctx.accesskit_node_builder(response.id, |node| {
                    node.set_disabled();
                    node.set_description(text);
                });
                response.on_hover_text(text)
            }
            Some(reason) => add(ui, false).unavailable_because(reason),
            None => add(ui, true),
        };
        Self {
            response,
            refused,
            reachable,
        }
    }

    /// Activated while available.
    pub fn clicked(&self) -> bool {
        self.refused.is_none() && self.response.clicked()
    }

    /// Why the control did nothing, when it was activated while unavailable.
    pub fn refusal(&self) -> Option<&'static str> {
        self.refused
            .filter(|_| self.response.clicked())
            .map(speech::unavailable_reason)
    }

    /// The control's own hover text. While it is refused in the browser, the reason it was
    /// given stays what the hover says: a second tooltip would replace it with, say, the chord
    /// that does nothing now.
    pub fn on_hover_text(mut self, text: impl Into<egui::WidgetText>) -> Self {
        if !self.reachable {
            self.response = self.response.on_hover_text(text);
        }
        self
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.response = self.response.named(name);
        self
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

/// Names the popup a menu button opened, which egui leaves an unnamed Tab stop: "Project menu".
pub fn name_menu(context: &egui::Context, button: &egui::Response, name: &str) {
    let popup = egui::Popup::default_response_id(button);
    for node in [popup, popup.with("move")] {
        if drawn_this_frame(context, node) {
            context.accesskit_node_builder(node, |builder| {
                builder.set_role(accesskit::Role::Menu);
                builder.set_label(name);
            });
        }
    }
}

/// Names a movable window's title bar, a Tab stop that only a pointer can drag.
pub fn name_window_title_bar(context: &egui::Context, window: egui::Id, title: &str) {
    let bar = window.with("__title_click");
    if drawn_this_frame(context, bar) {
        context.accesskit_node_builder(bar, |node| {
            node.set_label(format!(
                "{title}, title bar, dragged with a pointer to move it"
            ));
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

    /// Every text painted in the frame.
    fn painted_text(output: &egui::FullOutput) -> String {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_refused_control_in_the_browser_hovers_its_reason_not_its_chord() {
        let context = egui::Context::default();
        crate::platform::install(&context, crate::platform::Platform::Browser);
        let mut rect = egui::Rect::NOTHING;
        let mut painted = String::new();
        for frame in 0..6 {
            // The pointer comes to rest on the button, and the tooltip's delay passes.
            let events = if frame == 1 {
                vec![egui::Event::PointerMoved(rect.center())]
            } else {
                Vec::new()
            };
            let input = egui::RawInput {
                time: Some(f64::from(frame)),
                events,
                ..Default::default()
            };
            let output = context.run_ui(input, |ui| {
                let undo = Refusable::add(
                    ui,
                    Some(Unavailable::NothingToUndo),
                    egui::Button::new("Undo"),
                )
                .on_hover_text("Ctrl/Cmd+Z");
                rect = undo.response.rect;
            });
            painted = painted_text(&output);
        }
        assert!(painted.contains("Nothing to undo."), "{painted:?}");
        assert!(!painted.contains("Ctrl/Cmd+Z"), "{painted:?}");
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
