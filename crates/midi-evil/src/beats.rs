//! Maxwell's pad instrument. Bank persistence is independent of the song document.
use std::path::PathBuf;

use eframe::egui::{self, Key, Rect, RichText, TextureHandle, vec2};
use evil_beats::{DrumKit, PAD_COUNT};

mod bank;
mod drawing;
use drawing::{paint_pad, waveform};

use crate::{beats_player::BeatsPlayer, theme};

const KEYS: [(Key, &str); PAD_COUNT] = [
    (Key::Z, "Z"),
    (Key::X, "X"),
    (Key::C, "C"),
    (Key::V, "V"),
    (Key::A, "A"),
    (Key::S, "S"),
    (Key::D, "D"),
    (Key::F, "F"),
    (Key::Q, "Q"),
    (Key::W, "W"),
    (Key::E, "E"),
    (Key::R, "R"),
    (Key::Num1, "1"),
    (Key::Num2, "2"),
    (Key::Num3, "3"),
    (Key::Num4, "4"),
];

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Play,
    Edit,
    Mix,
}
enum Action {
    New,
    Starter,
    Open,
    Save,
    Import,
    Export,
    Clear,
}

pub struct BeatsPage {
    kit: DrumKit,
    path: Option<PathBuf>,
    dirty: bool,
    selected: usize,
    page: Page,
    velocity: u8,
    repeat: bool,
    rate: u8,
    held: u16,
    blocked: u16,
    last_hit: [f64; PAD_COUNT],
    player: Option<BeatsPlayer>,
    message: String,
    badge: TextureHandle,
    peaks: [Vec<f32>; PAD_COUNT],
}

impl BeatsPage {
    pub fn new(context: &egui::Context) -> Self {
        let (kit, message) = match evil_beats::maxwell_kit() {
            Ok(kit) => (
                kit,
                "Original synthesized percussion / ready to play".into(),
            ),
            Err(e) => (DrumKit::empty("Untitled Kit"), e.to_string()),
        };
        let peaks = kit_peaks(&kit);
        Self {
            kit,
            path: None,
            dirty: false,
            selected: 0,
            page: Page::Play,
            velocity: 100,
            repeat: false,
            rate: 16,
            held: 0,
            blocked: 0,
            last_hit: [f64::NEG_INFINITY; PAD_COUNT],
            player: None,
            message,
            badge: crate::art::load_beats(context),
            peaks,
        }
    }

    pub fn stop(&mut self) {
        self.player = None;
        self.blocked |= self.held;
        self.held = 0;
    }

    pub fn save(&mut self) {
        self.action(Action::Save);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tempo: f32) {
        let now = ui.input(|i| i.time);
        let accepts =
            !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.focused && !i.modifiers.any());
        let mut panic = ui.input(|i| !i.focused || (accepts && i.key_pressed(Key::Escape)));
        let mut action = None;
        let mut pad_changed = false;
        let edited_slot = self.selected;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("EVIL BEATS")
                    .size(23.0)
                    .strong()
                    .color(theme::IVORY),
            );
            theme::label(ui, "DR. MAXWELL'S RHYTHM LAB");
            ui.separator();
            for (page, label) in [
                (Page::Play, "PLAY"),
                (Page::Edit, "EDIT PAD"),
                (Page::Mix, "OUTPUT"),
            ] {
                ui.selectable_value(&mut self.page, page, label);
            }
            panic |= ui.button("Silence all / Esc").clicked();
        });
        ui.add_space(6.0);
        let body_height = (ui.available_height() - 32.0).max(290.0);
        let left_width = if ui.available_width() < 1050.0 {
            220.0
        } else {
            260.0
        };
        let grid_width = ui.available_width() - left_width - 16.0;
        let mut desired = 0_u16;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(left_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(left_width);
                egui::ScrollArea::vertical().id_salt("beats-controls").max_height(body_height).auto_shrink([false,false]).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let width = if body_height < 500.0 { 105.0 } else { 150.0 };
                        ui.image((self.badge.id(), vec2(width, width * 0.6)));
                    });
                    theme::label(ui, if self.dirty { "BANK / UNSAVED CHANGES" } else { "BANK" });
                    self.dirty |= ui.text_edit_singleline(&mut self.kit.name).changed();
                    ui.horizontal(|ui| {
                        if ui.button("Open bank").clicked() { action = Some(Action::Open); }
                        if ui.button("Save bank").clicked() { action = Some(Action::Save); }
                    });
                    if self.page == Page::Play {
                        ui.horizontal(|ui| {
                            if ui.button("New bank").clicked() { action = Some(Action::New); }
                            if ui.button("Lab kit").clicked() { action = Some(Action::Starter); }
                        });
                        ui.add_space(8.0);
                        theme::section(ui, "AUTO BEAT", theme::RED, |ui| {
                            ui.toggle_value(&mut self.repeat, "Repeat held pads");
                            ui.label(RichText::new(format!("{tempo:.1} BPM / song tempo")).small().color(theme::MUTED));
                            for row in [[1,2], [4,8], [16,32]] {
                                ui.horizontal(|ui| {
                                    let width = (ui.available_width() - 8.0) / 2.0;
                                    for rate in row {
                                        if ui.add_sized([width, if body_height < 500.0 { 28.0 } else { 40.0 }], egui::Button::new(format!("1/{rate}")).selected(self.rate == rate)).clicked() { self.rate = rate; }
                                    }
                                });
                            }
                        });
                        ui.add_space(8.0);
                        ui.add(egui::Slider::new(&mut self.velocity,1..=127).text("Velocity"));
                        self.dirty |= ui.add(egui::Slider::new(&mut self.kit.gain,0.0..=1.0).text("Output")).changed();
                        ui.label(RichText::new("Hold a pad for rolls.\nRight-click a pad to edit its sound.").small().color(theme::MUTED));
                    } else {
                        ui.add_space(8.0);
                        theme::label(ui, format!("PAD {:02} / {}", self.selected + 1, if self.page == Page::Edit { "SOUND" } else { "STEREO OUTPUT" }));
                        if let Some(pad) = &mut self.kit.pads[self.selected] {
                            if self.page == Page::Edit {
                                self.dirty |= ui.text_edit_singleline(&mut pad.voice.name).changed();
                                waveform(ui, &self.peaks[self.selected], 54.0, theme::track_color(self.selected / 4));
                                ui.label(RichText::new(format!("{:.2} s / {} Hz / mono", pad.voice.samples.len() as f32 / pad.voice.sample_rate as f32, pad.voice.sample_rate)).small().color(theme::MUTED));
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.tuning_semitones,-48.0..=48.0).text("Tune / st")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.attack_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Attack / s")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.decay_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Decay / s")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.sustain_level,0.0..=1.0).text("Sustain")).changed();
                                let mut gated = pad.gate_seconds.is_some();
                                if ui.checkbox(&mut gated,"Short gate").changed() { pad.gate_seconds = gated.then_some(0.1); pad_changed = true; }
                                if let Some(gate) = &mut pad.gate_seconds {
                                    pad_changed |= ui.add(egui::Slider::new(gate,0.001..=30.0).logarithmic(true).clamping(egui::SliderClamping::Edits).text("Gate / s")).changed();
                                    pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.release_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Release / s")).changed();
                                }
                            }
                            pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.gain,0.0..=1.0).text("Pad gain")).changed();
                            if self.page == Page::Mix {
                                ui.label(&pad.voice.name);
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.pan,-1.0..=1.0).text("Pan")).changed();
                                pad_changed |= ui.checkbox(&mut pad.muted,"Mute pad").changed();
                                ui.separator();
                                theme::label(ui,"BANK OUTPUT");
                                self.dirty |= ui.add(egui::Slider::new(&mut self.kit.gain,0.0..=1.0).text("Master")).changed();
                                ui.label(RichText::new("Stereo / default audio device").small().color(theme::MUTED));
                            }
                        } else {
                            ui.label("Empty pad. Load a WAV to give it a voice.");
                        }
                        if self.page == Page::Edit {
                            ui.horizontal(|ui| {
                                if ui.button("Load WAV").clicked() { action = Some(Action::Import); }
                                if ui.add_enabled(self.kit.pads[self.selected].is_some(), egui::Button::new("Save WAV")).clicked() { action = Some(Action::Export); }
                            });
                            if ui.add_enabled(self.kit.pads[self.selected].is_some(), egui::Button::new("Clear pad")).clicked() { action = Some(Action::Clear); }
                        }
                    }
                });
            });
            ui.add_space(8.0);
            let size = vec2((grid_width - 30.0)/4.0, (body_height - 30.0)/4.0);
            ui.vertical(|ui| {
                let origin = ui.cursor().min;
                let grid = Rect::from_min_size(origin, vec2(grid_width,body_height));
                let mouse = ui.input(|i| i.focused && i.pointer.primary_down()
                    && i.pointer.press_origin().is_some_and(|p| grid.contains(p)));
                for row in (0..4).rev() {
                    ui.horizontal(|ui| {
                        for column in 0..4 {
                            let slot = row * 4 + column;
                            let key_down = accepts && ui.input(|i| i.key_down(KEYS[slot].0));
                            let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
                            let pointer_down = mouse && ui.input(|i| i.pointer.interact_pos().is_some_and(|p| response.rect.contains(p)));
                            let down = key_down || pointer_down;
                            if down { desired |= 1 << slot; }
                            if down && self.held & (1 << slot) == 0 && self.blocked & (1 << slot) == 0 {
                                self.selected = slot;
                                self.last_hit[slot] = now;
                            }
                            if response.secondary_clicked() || response.double_clicked() {
                                self.selected = slot;
                                self.page = Page::Edit;
                            }
                            let lit = self.blocked & (1 << slot) == 0 && (down || now - self.last_hit[slot] < 0.13);
                            paint_pad(&painter,response.rect,slot,self.kit.pads[slot].as_ref(),&self.peaks[slot],self.selected == slot,lit);
                        }
                    });
                    if row > 0 { ui.add_space(3.0); }
                }
            });
        });
        ui.add_space(5.0);
        ui.horizontal_wrapped(|ui| {
            theme::label(ui, "Z X C V / A S D F / Q W E R / 1 2 3 4");
            ui.label(RichText::new(&self.message).small().color(theme::MUTED));
        });
        self.blocked &= desired;
        if panic {
            self.blocked |= desired;
            desired = 0;
            if let Some(player) = &mut self.player {
                player.stop();
            }
        } else {
            desired &= !self.blocked;
        }
        if pad_changed {
            self.dirty = true;
            if let Some(player) = &self.player
                && let Err(e) = player.update_pad(&self.kit, edited_slot)
            {
                self.message = e;
            }
        }
        if desired != 0 && self.player.is_none() {
            match BeatsPlayer::new(&self.kit) {
                Ok(player) => self.player = Some(player),
                Err(e) => {
                    self.message = format!("Audio unavailable: {e}");
                    self.blocked |= desired;
                    desired = 0;
                }
            }
        }
        if let Some(player) = &mut self.player {
            player.input(
                desired,
                self.velocity,
                self.repeat.then_some(self.rate),
                tempo,
                self.kit.gain,
            );
        }
        self.held = desired;
        if let Some(action) = action {
            self.action(action);
        }
    }
}

fn kit_peaks(kit: &DrumKit) -> [Vec<f32>; PAD_COUNT] {
    std::array::from_fn(|slot| {
        kit.pads[slot].as_ref().map_or_else(Vec::new, |pad| {
            pad.voice
                .samples
                .chunks(pad.voice.samples.len().div_ceil(100).max(1))
                .map(|chunk| {
                    chunk
                        .iter()
                        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
                })
                .collect()
        })
    })
}
