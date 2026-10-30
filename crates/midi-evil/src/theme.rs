use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(19, 17, 25);
pub const PANEL: Color32 = Color32::from_rgb(30, 26, 38);
pub const RAISED: Color32 = Color32::from_rgb(43, 36, 53);
pub const BORDER: Color32 = Color32::from_rgb(68, 57, 79);
pub const TEXT: Color32 = Color32::from_rgb(238, 231, 215);
pub const MUTED: Color32 = Color32::from_rgb(164, 153, 177);
pub const IVORY: Color32 = Color32::from_rgb(249, 226, 159);
pub const RED: Color32 = Color32::from_rgb(216, 73, 91);
pub const GREEN: Color32 = Color32::from_rgb(131, 194, 161);
pub const PURPLE: Color32 = Color32::from_rgb(166, 128, 208);

pub fn install(context: &egui::Context) {
    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = RAISED;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = Color32::from_rgb(91, 55, 88);
    visuals.selection.stroke = Stroke::new(1.0, IVORY);
    visuals.hyperlink_color = IVORY;
    visuals.warn_fg_color = IVORY;
    visuals.error_fg_color = RED;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = 5.into();
    visuals.menu_corner_radius = 3.into();
    visuals.slider_trailing_fill = true;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = 3.into();
        widget.fg_stroke = Stroke::new(1.0, TEXT);
        widget.bg_stroke = Stroke::new(1.0, BORDER);
    }
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.bg_fill = RAISED;
    visuals.widgets.inactive.weak_bg_fill = RAISED;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(65, 48, 73);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(65, 48, 73);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, PURPLE);
    visuals.widgets.active.bg_fill = Color32::from_rgb(105, 49, 70);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(105, 49, 70);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, RED);
    visuals.widgets.open.bg_fill = RAISED;
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.slider_width = 110.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(21.0));
    context.set_theme(egui::Theme::Dark);
    context.set_style_of(egui::Theme::Dark, style);
}

pub fn panel() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .inner_margin(12)
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn label(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(11.0).strong().color(MUTED));
}

pub fn section(ui: &mut egui::Ui, title: &str, accent: Color32, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(4)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 1.0).max(0.0));
            ui.label(RichText::new(title).size(12.0).strong().color(accent));
            ui.separator();
            body(ui);
        });
}

pub fn track_color(index: usize) -> Color32 {
    [
        RED,
        PURPLE,
        IVORY,
        GREEN,
        Color32::from_rgb(111, 166, 199),
        Color32::from_rgb(212, 146, 110),
    ][index % 6]
}
