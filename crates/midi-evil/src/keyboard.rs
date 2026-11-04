//! Performance-page input and drawing. Audio lives in `live_player`.
use std::collections::BTreeSet;

use eframe::egui::{
    self, Align2, Color32, FontId, Key, Pos2, Rect, RichText, Stroke, TextureHandle, Vec2, pos2,
    vec2,
};
use evil_synth::{Delay, Filter, Patch};

use crate::theme;

const KEY_MAP: [(Key, &str); 18] = [
    (Key::A, "A"),
    (Key::W, "W"),
    (Key::S, "S"),
    (Key::E, "E"),
    (Key::D, "D"),
    (Key::F, "F"),
    (Key::T, "T"),
    (Key::G, "G"),
    (Key::Y, "Y"),
    (Key::H, "H"),
    (Key::U, "U"),
    (Key::J, "J"),
    (Key::K, "K"),
    (Key::O, "O"),
    (Key::L, "L"),
    (Key::P, "P"),
    (Key::Semicolon, ";"),
    (Key::Quote, "'"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteEvent {
    On(u8, u8),
    Off(u8),
    Panic,
}

pub struct KeyboardState {
    pub velocity: u8,
    pub sustain: bool,
    pub bend: f32,
    base: u8,
    held: BTreeSet<u8>,
    suppressed: BTreeSet<u8>,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            velocity: 100,
            sustain: false,
            bend: 0.0,
            base: 48,
            held: BTreeSet::new(),
            suppressed: BTreeSet::new(),
        }
    }
}

impl KeyboardState {
    pub fn reset(&mut self) {
        self.held.clear();
        self.suppressed.clear();
        self.sustain = false;
        self.bend = 0.0;
    }

    fn silence(&mut self, desired: BTreeSet<u8>) -> Vec<NoteEvent> {
        self.suppressed.extend(&self.held);
        self.suppressed.extend(desired);
        self.held.clear();
        self.sustain = false;
        self.bend = 0.0;
        vec![NoteEvent::Panic]
    }

    fn reconcile(&mut self, mut desired: BTreeSet<u8>) -> Vec<NoteEvent> {
        self.suppressed.retain(|pitch| desired.contains(pitch));
        desired.retain(|pitch| !self.suppressed.contains(pitch));
        let mut events: Vec<_> = self
            .held
            .difference(&desired)
            .copied()
            .map(NoteEvent::Off)
            .collect();
        events.extend(
            desired
                .difference(&self.held)
                .map(|&pitch| NoteEvent::On(pitch, self.velocity)),
        );
        self.held = desired;
        events
    }
}

pub fn show(
    ui: &mut egui::Ui,
    patch: &mut Patch,
    state: &mut KeyboardState,
    badge: &TextureHandle,
    bend_range: f32,
) -> Vec<NoteEvent> {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("EVIL KEYS")
                .size(23.0)
                .strong()
                .color(theme::IVORY),
        );
        ui.label(RichText::new(&patch.name).monospace().color(theme::PURPLE));
        theme::label(ui, "DR. MAXWELL'S PERFORMANCE LAB");
    });
    let panic = performance_controls(ui, state);
    ui.add_space(8.0);
    let deck_height = (ui.available_height() * 0.43).clamp(164.0, 310.0);
    let plate_width = if ui.available_width() < 800.0 {
        112.0
    } else {
        150.0
    };
    let pad_width = ((ui.available_width() - plate_width - 20.0) / 2.0).max(140.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(plate_width, deck_height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let width = (plate_width - 12.0).min(deck_height * 0.47);
                ui.image((badge.id(), vec2(width, width * 0.6)));
                theme::label(ui, "PITCH");
                let height = (deck_height - width * 0.6 - 55.0).max(48.0);
                pitch_wheel(ui, &mut state.bend, height);
                ui.label(
                    RichText::new(format!("{:+.2} st", state.bend * bend_range))
                        .monospace()
                        .color(theme::IVORY),
                );
            },
        );
        tone_pad(ui, patch, vec2(pad_width, deck_height));
        space_pad(ui, patch, vec2(pad_width, deck_height));
    });
    ui.add_space(10.0);
    key_hint(ui);
    let height = (ui.available_height() - 3.0).clamp(100.0, 340.0);
    piano(ui, state, height, panic)
}

/// The same performance input as the full keyboard page, without its XY deck.
pub fn show_compact(ui: &mut egui::Ui, state: &mut KeyboardState) -> Vec<NoteEvent> {
    let panic = performance_controls(ui, state);
    let narrow = ui.available_width() < 700.0;
    if !narrow {
        key_hint(ui);
    }
    piano(ui, state, if narrow { 84.0 } else { 100.0 }, panic)
}

fn accepts_keys(ui: &egui::Ui) -> bool {
    !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.focused && !i.modifiers.any())
}

fn performance_controls(ui: &mut egui::Ui, state: &mut KeyboardState) -> bool {
    let accepts_keys = accepts_keys(ui);
    let mut panic = ui.input(|i| !i.focused || (accepts_keys && i.key_pressed(Key::Escape)));
    ui.horizontal_wrapped(|ui| {
        theme::label(ui, "OCTAVE");
        if ui
            .add_enabled(state.base >= 12, egui::Button::new("−"))
            .clicked()
            || (accepts_keys && ui.input(|i| i.key_pressed(Key::Z)))
        {
            state.base = state.base.saturating_sub(12);
        }
        ui.monospace(format!(
            "C{} – B{}",
            i16::from(state.base) / 12 - 1,
            i16::from(state.base) / 12 + 1
        ));
        if ui
            .add_enabled(state.base < 84, egui::Button::new("+"))
            .clicked()
            || (accepts_keys && ui.input(|i| i.key_pressed(Key::X)))
        {
            state.base = (state.base + 12).min(84);
        }
        ui.separator();
        ui.add(egui::Slider::new(&mut state.velocity, 1..=127).text("Velocity"));
        ui.toggle_value(&mut state.sustain, "Sustain")
            .on_hover_text("Hold released notes until sustain is turned off");
        panic |= ui
            .button("All notes off")
            .on_hover_text("Escape · clear held notes and delay tails")
            .clicked();
    });
    panic
}

fn key_hint(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        theme::label(ui, "PLAY / THREE OCTAVES");
        ui.label(
            RichText::new("A W S E D F T G Y H U J K…  ·  Z / X octave  ·  Esc silence")
                .size(11.0)
                .color(theme::MUTED),
        );
    });
}

fn piano(ui: &mut egui::Ui, state: &mut KeyboardState, height: f32, panic: bool) -> Vec<NoteEvent> {
    let (response, painter) = ui.allocate_painter(
        vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    let geometry = key_geometry(response.rect.shrink(2.0), state.base);
    let mut desired = BTreeSet::new();
    if accepts_keys(ui) {
        ui.input(|input| {
            for (offset, (key, _)) in KEY_MAP.iter().enumerate() {
                if input.key_down(*key) {
                    desired.insert(state.base + offset as u8);
                }
            }
        });
    }
    if ui.input(|i| {
        i.focused
            && i.pointer.primary_down()
            && i.pointer
                .press_origin()
                .is_some_and(|origin| response.rect.contains(origin))
    }) && let Some(position) = ui.input(|i| i.pointer.interact_pos())
        && let Some(pitch) = key_at(&geometry, position)
    {
        desired.insert(pitch);
    }
    let events = if panic {
        state.silence(desired)
    } else {
        state.reconcile(desired)
    };
    for key in &geometry {
        draw_key(&painter, key, state.held.contains(&key.pitch), state.base);
    }
    events
}

fn pitch_wheel(ui: &mut egui::Ui, bend: &mut f32, height: f32) {
    let (response, painter) =
        ui.allocate_painter(vec2(46.0, height), egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 5.0, theme::BACKGROUND);
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    if response.is_pointer_button_down_on()
        && let Some(position) = response.interact_pointer_pos()
    {
        *bend = (1.0 - 2.0 * (position.y - rect.top()) / rect.height()).clamp(-1.0, 1.0);
    } else {
        *bend = 0.0;
    }
    for i in 1..12 {
        let y = rect.top() + rect.height() * i as f32 / 12.0;
        painter.line_segment(
            [pos2(rect.left() + 7.0, y), pos2(rect.right() - 7.0, y)],
            Stroke::new(1.0, theme::BORDER),
        );
    }
    let y = rect.center().y - *bend * (height / 2.0 - 6.0);
    painter.rect_filled(
        Rect::from_center_size(pos2(rect.center().x, y), vec2(32.0, 8.0)),
        2.0,
        theme::RED,
    );
    response.on_hover_text("Hold and drag to bend pitch · release to return to center");
}

fn tone_pad(ui: &mut egui::Ui, patch: &mut Patch, size: Vec2) {
    let filter = patch.filter.clone().unwrap_or_default();
    let mut point = vec2(
        (filter.cutoff_hz / 20.0).ln() / 1000.0_f32.ln(),
        filter.resonance,
    );
    let label = format!("{:.0} Hz   /   Q {:.2}", filter.cutoff_hz, filter.resonance);
    if xy_pad(
        ui,
        "01 / TONE",
        "X: CUTOFF   /   Y: RESONANCE",
        &label,
        patch.filter.is_some(),
        &mut point,
        size,
        theme::RED,
    ) {
        let cutoff = (20.0 * 1000.0_f32.powf(point.x)).min(patch.sample_rate as f32 * 0.49);
        patch.filter = Some(Filter {
            cutoff_hz: cutoff,
            resonance: point.y,
            ..filter
        });
    }
}

fn space_pad(ui: &mut egui::Ui, patch: &mut Patch, size: Vec2) {
    let delay = patch.delay.clone().unwrap_or_default();
    let mut point = vec2((delay.time_seconds - 0.02) / 0.98, delay.mix);
    let label = format!(
        "{:.0} ms   /   WET {:.0}%",
        delay.time_seconds * 1000.0,
        delay.mix * 100.0
    );
    if xy_pad(
        ui,
        "02 / SPACE",
        "X: DELAY TIME   /   Y: WET MIX",
        &label,
        patch.delay.is_some(),
        &mut point,
        size,
        theme::PURPLE,
    ) {
        patch.delay = Some(Delay {
            time_seconds: 0.02 + point.x * 0.98,
            mix: point.y,
            ..delay
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn xy_pad(
    ui: &mut egui::Ui,
    title: &str,
    axes: &str,
    value: &str,
    enabled: bool,
    point: &mut Vec2,
    size: Vec2,
    color: Color32,
) -> bool {
    let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
    let outer = response.rect;
    painter.rect_filled(outer, 4.0, theme::PANEL);
    painter.rect_stroke(
        outer,
        4.0,
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    painter.text(
        outer.min + vec2(12.0, 14.0),
        Align2::LEFT_CENTER,
        title,
        FontId::monospace(12.0),
        color,
    );
    let pad = Rect::from_min_max(outer.min + vec2(12.0, 34.0), outer.max - vec2(12.0, 48.0));
    painter.rect_filled(pad, 2.0, theme::BACKGROUND);
    for i in 1..8 {
        let fraction = i as f32 / 8.0;
        let x = pad.left() + pad.width() * fraction;
        let y = pad.top() + pad.height() * fraction;
        painter.line_segment(
            [pos2(x, pad.top()), pos2(x, pad.bottom())],
            Stroke::new(0.5, theme::BORDER.gamma_multiply(0.5)),
        );
        painter.line_segment(
            [pos2(pad.left(), y), pos2(pad.right(), y)],
            Stroke::new(0.5, theme::BORDER.gamma_multiply(0.5)),
        );
    }
    let mut changed = false;
    if response.is_pointer_button_down_on()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && pad.contains(origin)
        && let Some(position) = response.interact_pointer_pos()
    {
        *point = vec2(
            ((position.x - pad.left()) / pad.width()).clamp(0.0, 1.0),
            ((pad.bottom() - position.y) / pad.height()).clamp(0.0, 1.0),
        );
        changed = true;
    }
    let cursor = pos2(
        pad.left() + point.x.clamp(0.0, 1.0) * pad.width(),
        pad.bottom() - point.y.clamp(0.0, 1.0) * pad.height(),
    );
    let clipped = painter.with_clip_rect(pad.intersect(painter.clip_rect()));
    for radius in [28.0, 19.0, 11.0] {
        clipped.circle_filled(
            cursor,
            radius,
            color.gamma_multiply(if enabled || changed { 0.09 } else { 0.025 }),
        );
    }
    clipped.line_segment(
        [pos2(cursor.x, pad.top()), pos2(cursor.x, pad.bottom())],
        Stroke::new(1.0, color.gamma_multiply(0.4)),
    );
    clipped.line_segment(
        [pos2(pad.left(), cursor.y), pos2(pad.right(), cursor.y)],
        Stroke::new(1.0, color.gamma_multiply(0.4)),
    );
    clipped.circle_stroke(cursor, 5.0, Stroke::new(2.0, color));
    painter.text(
        pos2(outer.left() + 12.0, outer.bottom() - 31.0),
        Align2::LEFT_CENTER,
        axes,
        FontId::proportional(10.0),
        theme::MUTED,
    );
    painter.text(
        pos2(outer.left() + 12.0, outer.bottom() - 13.0),
        Align2::LEFT_CENTER,
        if enabled || changed {
            value
        } else {
            "Drag to enable"
        },
        FontId::monospace(11.0),
        theme::IVORY,
    );
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
    changed
}

struct PianoKey {
    pitch: u8,
    rect: Rect,
    black: bool,
}

fn key_geometry(rect: Rect, base: u8) -> Vec<PianoKey> {
    let width = rect.width() / 21.0;
    let mut keys = Vec::with_capacity(36);
    let mut whites = 0;
    for offset in 0..36 {
        let pitch = base + offset;
        let black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
        let key = if black {
            Rect::from_min_size(
                pos2(
                    rect.left() + whites as f32 * width - width * 0.31,
                    rect.top(),
                ),
                vec2(width * 0.62, rect.height() * 0.63),
            )
        } else {
            let r = Rect::from_min_size(
                pos2(rect.left() + whites as f32 * width, rect.top()),
                vec2(width, rect.height()),
            );
            whites += 1;
            r
        };
        keys.push(PianoKey {
            pitch,
            rect: key,
            black,
        });
    }
    keys.sort_by_key(|key| key.black);
    keys
}

fn key_at(keys: &[PianoKey], position: Pos2) -> Option<u8> {
    keys.iter()
        .rev()
        .find(|key| key.rect.contains(position))
        .map(|key| key.pitch)
}

fn draw_key(painter: &egui::Painter, key: &PianoKey, down: bool, base: u8) {
    let rect = key.rect.shrink2(vec2(0.8, 1.0));
    let fill = if down {
        theme::RED
    } else if key.black {
        Color32::from_rgb(39, 32, 46)
    } else {
        Color32::from_rgb(229, 218, 193)
    };
    painter.rect_filled(rect.translate(vec2(1.0, 3.0)), 3.0, theme::BACKGROUND);
    painter.rect_filled(rect, 3.0, fill);
    let bevel = Rect::from_min_max(
        pos2(rect.left() + 2.0, rect.bottom() - 7.0),
        rect.max - vec2(2.0, 1.0),
    );
    painter.rect_filled(
        bevel,
        2.0,
        if down {
            Color32::from_rgb(147, 44, 63)
        } else if key.black {
            theme::BACKGROUND
        } else {
            Color32::from_rgb(166, 151, 137)
        },
    );
    painter.line_segment(
        [
            rect.left_top() + vec2(2.0, 2.0),
            rect.right_top() + vec2(-2.0, 2.0),
        ],
        Stroke::new(
            2.0,
            if key.black {
                theme::BORDER
            } else {
                theme::IVORY
            },
        ),
    );
    let text_color = if key.black || down {
        theme::TEXT
    } else {
        theme::PANEL
    };
    if key.pitch.is_multiple_of(12) {
        painter.text(
            rect.center_bottom() - vec2(0.0, 21.0),
            Align2::CENTER_BOTTOM,
            format!("C{}", i16::from(key.pitch) / 12 - 1),
            FontId::monospace(12.0),
            text_color,
        );
    }
    if let Some((_, label)) = KEY_MAP.get(usize::from(key.pitch - base)) {
        painter.text(
            rect.center_bottom()
                - vec2(
                    0.0,
                    if key.pitch.is_multiple_of(12) {
                        43.0
                    } else {
                        21.0
                    },
                ),
            Align2::CENTER_BOTTOM,
            label,
            FontId::monospace(10.0),
            text_color.gamma_multiply(0.7),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_keyboard_plays_chords_and_silences_on_focus_loss() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut state = KeyboardState::default();
        let mut frame = |events, focused| {
            let mut notes = Vec::new();
            let _ = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 220.0))),
                    events,
                    focused,
                    ..Default::default()
                },
                |ui| {
                    notes = show_compact(ui, &mut state);
                },
            );
            notes
        };
        frame(vec![], true);
        let key = |key, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(
            frame(vec![key(Key::A, true), key(Key::D, true)], true),
            vec![NoteEvent::On(48, 100), NoteEvent::On(52, 100)]
        );
        assert!(frame(vec![], true).is_empty());
        assert_eq!(
            frame(vec![key(Key::A, false)], true),
            vec![NoteEvent::Off(48)]
        );
        assert_eq!(frame(vec![], false), vec![NoteEvent::Panic]);
    }

    #[test]
    fn black_keys_win_overlap_and_all_keys_are_reachable() {
        let keys = key_geometry(Rect::from_min_size(Pos2::ZERO, vec2(840.0, 240.0)), 48);
        for key in &keys {
            let point = if key.black {
                key.rect.center()
            } else {
                key.rect.center_bottom() - vec2(0.0, 12.0)
            };
            assert_eq!(key_at(&keys, point), Some(key.pitch));
        }
        assert_eq!(key_at(&keys, pos2(-1.0, 100.0)), None);
    }

    #[test]
    fn held_keys_emit_edges_not_repeated_triggers_and_glissando_releases_first() {
        let mut state = KeyboardState::default();
        assert_eq!(
            state.reconcile(BTreeSet::from([60, 64])),
            vec![NoteEvent::On(60, 100), NoteEvent::On(64, 100)]
        );
        assert!(state.reconcile(BTreeSet::from([60, 64])).is_empty());
        assert_eq!(
            state.reconcile(BTreeSet::from([62, 64])),
            vec![NoteEvent::Off(60), NoteEvent::On(62, 100)]
        );
        assert_eq!(
            state.reconcile(BTreeSet::new()),
            vec![NoteEvent::Off(62), NoteEvent::Off(64)]
        );
    }

    #[test]
    fn panic_requires_held_keys_to_be_released_before_retriggering() {
        let mut state = KeyboardState::default();
        state.reconcile(BTreeSet::from([60, 64]));
        state.sustain = true;
        assert_eq!(
            state.silence(BTreeSet::from([60, 64])),
            vec![NoteEvent::Panic]
        );
        assert!(!state.sustain);
        assert!(state.reconcile(BTreeSet::from([60, 64])).is_empty());
        assert!(state.reconcile(BTreeSet::from([64])).is_empty());
        assert_eq!(
            state.reconcile(BTreeSet::from([60, 64])),
            vec![NoteEvent::On(60, 100)]
        );
    }
}
