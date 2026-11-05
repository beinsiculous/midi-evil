//! Maxwell's pad instrument with portable banks and song-instrument linking.
use std::{path::PathBuf, time::Instant};

use eframe::egui::{self, Key, Rect, RichText, TextureHandle, vec2};
use evil_beats::{DrumFilter, DrumKit, DrumPad, PAD_COUNT, SampleDirection};
use evil_synth::InsertEffect;

mod bank;
mod drawing;
use drawing::{paint_pad, waveform};

use crate::{
    beats_player::BeatsPlayer,
    effect_ui::{self, EffectType},
    theme,
};

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
#[derive(Clone, Copy, PartialEq)]
enum Action {
    New,
    Starter,
    Open,
    Save,
    Import,
    Export,
    Crop,
    Normalize,
    ApplyEffect,
    UndoSample,
    Clear,
}

struct SampleUndo {
    slot: usize,
    samples: Vec<f32>,
    playback_start_frame: usize,
    playback_end_frame: Option<usize>,
    label: &'static str,
}

impl SampleUndo {
    fn capture(slot: usize, pad: &DrumPad, label: &'static str) -> Self {
        Self {
            slot,
            samples: pad.samples.clone(),
            playback_start_frame: pad.playback_start_frame,
            playback_end_frame: pad.playback_end_frame,
            label,
        }
    }

    fn restore(self, pad: &mut DrumPad) {
        pad.samples = self.samples;
        pad.playback_start_frame = self.playback_start_frame;
        pad.playback_end_frame = self.playback_end_frame;
    }
}

#[derive(Clone, Copy)]
pub struct PadHit {
    pub slot: u8,
    pub velocity: u8,
    pub at: Instant,
}

pub struct BeatsPage {
    kit: DrumKit,
    path: Option<PathBuf>,
    saved_path: Option<PathBuf>,
    track_index: Option<usize>,
    dirty: bool,
    selected: usize,
    page: Page,
    velocity: u8,
    repeat: bool,
    rate: u8,
    held: u16,
    midi_held: u16,
    midi_velocity: u8,
    blocked: u16,
    last_hit: [f64; PAD_COUNT],
    player: Option<BeatsPlayer>,
    message: String,
    badge: TextureHandle,
    peaks: [Vec<f32>; PAD_COUNT],
    sample_undo: Option<SampleUndo>,
    sample_effect: InsertEffect,
    tempo_bpm: f32,
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
            saved_path: None,
            track_index: None,
            dirty: false,
            selected: 0,
            page: Page::Play,
            velocity: 100,
            repeat: false,
            rate: 16,
            held: 0,
            midi_held: 0,
            midi_velocity: 100,
            blocked: 0,
            last_hit: [f64::NEG_INFINITY; PAD_COUNT],
            player: None,
            message,
            badge: crate::art::load_beats(context),
            peaks,
            sample_undo: None,
            sample_effect: InsertEffect::Bitcrush(evil_synth::Bitcrush::default()),
            tempo_bpm: 120.0,
        }
    }

    pub fn stop(&mut self) {
        self.player = None;
        self.blocked |= self.held;
        self.held = 0;
        self.midi_held = 0;
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    pub fn load_instrument(&mut self, kit: DrumKit, path: Option<PathBuf>, track_index: usize) {
        self.stop();
        self.kit = kit;
        self.path = path;
        self.saved_path = None;
        self.track_index = Some(track_index);
        self.dirty = false;
        self.selected = 0;
        self.sample_undo = None;
        self.message = format!("Linked to song track {:02}", track_index + 1);
        self.peaks = kit_peaks(&self.kit);
    }

    pub fn clear_instrument_link(&mut self) {
        self.track_index = None;
        self.saved_path = None;
    }

    pub fn take_saved_bank(&mut self) -> Option<(usize, PathBuf)> {
        let path = self.saved_path.take()?;
        self.track_index.map(|track| (track, path))
    }

    pub fn midi_input(&mut self, slot: u8, on: bool, velocity: u8) {
        let mask = 1_u16 << slot.min(15);
        if on {
            self.midi_held |= mask;
            self.midi_velocity = velocity;
            self.selected = usize::from(slot.min(15));
        } else {
            self.midi_held &= !mask;
        }
    }

    pub fn save(&mut self) {
        self.action(Action::Save);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tempo: f32) -> Vec<PadHit> {
        self.tempo_bpm = tempo;
        let mut hits = Vec::new();
        let wall_now = Instant::now();
        let now = ui.input(|i| i.time);
        let accepts =
            !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.focused && !i.modifiers.any());
        let mut panic = ui.input(|i| !i.focused || (accepts && i.key_pressed(Key::Escape)));
        let mut action = None;
        let mut pad_changed = false;
        let mut bus_changed = false;
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
        let mut desired = self.midi_held;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(left_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(left_width);
                egui::ScrollArea::vertical().id_salt("beats-controls").max_height(body_height).auto_shrink([false,false]).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let width = if body_height < 500.0 { 105.0 } else { 150.0 };
                        ui.image((self.badge.id(), vec2(width, width * 0.6)));
                    });
                    let bank_label = match (self.track_index, self.dirty) {
                        (Some(track), true) => format!("BANK / TRACK {:02} / UNSAVED", track + 1),
                        (Some(track), false) => format!("BANK / TRACK {:02}", track + 1),
                        (None, true) => "BANK / UNSAVED CHANGES".to_owned(),
                        (None, false) => "BANK".to_owned(),
                    };
                    theme::label(ui, bank_label);
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
                                let frame_count = pad.voice.samples.len();
                                let mut trim_start = pad.voice.playback_start_frame;
                                let mut trim_end = pad.voice.playback_range().end;
                                let trim_changed = ui.horizontal(|ui| {
                                    let start_changed = ui.add(egui::DragValue::new(&mut trim_start).range(0..=frame_count - 1).prefix("Trim ")).changed();
                                    let end_changed = ui.add(egui::DragValue::new(&mut trim_end).range(1..=frame_count).prefix("to ")).changed();
                                    start_changed || end_changed
                                }).inner;
                                if trim_changed {
                                    trim_start = trim_start.min(frame_count - 1);
                                    trim_end = trim_end.clamp(trim_start + 1, frame_count);
                                    pad.voice.playback_start_frame = trim_start;
                                    pad.voice.playback_end_frame = (trim_end < frame_count).then_some(trim_end);
                                    pad_changed = true;
                                }
                                ui.horizontal(|ui| {
                                    ui.label("Direction");
                                    egui::ComboBox::from_id_salt("pad-direction")
                                        .selected_text(match pad.voice.direction { SampleDirection::Forward => "Forward", SampleDirection::Reverse => "Reverse" })
                                        .show_ui(ui, |ui| {
                                            pad_changed |= ui.selectable_value(&mut pad.voice.direction, SampleDirection::Forward, "Forward").changed();
                                            pad_changed |= ui.selectable_value(&mut pad.voice.direction, SampleDirection::Reverse, "Reverse").changed();
                                        });
                                });
                                if ui.checkbox(&mut pad.voice.loop_enabled, "Loop trimmed range").changed() {
                                    if pad.voice.loop_enabled && pad.gate_seconds.is_none() {
                                        pad.gate_seconds = Some(1.0);
                                    }
                                    pad_changed = true;
                                }
                                ui.horizontal(|ui| {
                                    ui.label("Choke");
                                    egui::ComboBox::from_id_salt("pad-choke")
                                        .selected_text(if pad.choke_group == 0 { "Off".to_owned() } else { format!("Group {}", pad.choke_group) })
                                        .show_ui(ui, |ui| {
                                            for (group, label) in [(0, "Off"), (1, "Group 1"), (2, "Group 2")] {
                                                pad_changed |= ui.selectable_value(&mut pad.choke_group, group, label).changed();
                                            }
                                        });
                                });
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.tuning_semitones,-48.0..=48.0).text("Tune / st")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.attack_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Attack / s")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.decay_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Decay / s")).changed();
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.sustain_level,0.0..=1.0).text("Sustain")).changed();
                                let mut gated = pad.gate_seconds.is_some() || pad.voice.loop_enabled;
                                let gate_label = if pad.voice.loop_enabled { "Loop gate (required)" } else { "Timed gate" };
                                if ui.add_enabled(!pad.voice.loop_enabled, egui::Checkbox::new(&mut gated, gate_label)).changed() { pad.gate_seconds = gated.then_some(0.1); pad_changed = true; }
                                if let Some(gate) = &mut pad.gate_seconds {
                                    pad_changed |= ui.add(egui::Slider::new(gate,0.001..=30.0).logarithmic(true).clamping(egui::SliderClamping::Edits).text("Gate / s")).changed();
                                    pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.envelope.release_seconds,0.0..=60.0).logarithmic(true).smallest_positive(0.001).text("Release / s")).changed();
                                }
                                pad_changed |= filter_controls(ui, &mut pad.voice.filter, "pad-filter");
                            }
                            pad_changed |= ui.add(egui::Slider::new(&mut pad.voice.gain,0.0..=1.0).text("Pad gain")).changed();
                            if self.page == Page::Mix {
                                ui.label(&pad.voice.name);
                                pad_changed |= ui.add(egui::Slider::new(&mut pad.pan,-1.0..=1.0).text("Pan")).changed();
                                pad_changed |= ui.checkbox(&mut pad.muted,"Mute pad").changed();
                                for (index, send) in pad.bus_sends.iter_mut().enumerate() {
                                    pad_changed |= ui.add(egui::Slider::new(send,0.0..=1.0).text(format!("Bus {} send", index + 1))).changed();
                                }
                                ui.separator();
                                theme::label(ui,"BANK OUTPUT");
                                self.dirty |= ui.add(egui::Slider::new(&mut self.kit.gain,0.0..=1.0).text("Master")).changed();
                                ui.label(RichText::new("Stereo / default audio device").small().color(theme::MUTED));
                                ui.separator();
                                theme::label(ui,"TRG BUSES");
                                for (index, bus) in self.kit.buses.iter_mut().enumerate() {
                                    theme::label(ui,format!("BUS {}",index + 1));
                                    bus_changed |= ui.add(egui::Slider::new(&mut bus.gain,0.0..=1.0).text("Gain")).changed();
                                    bus_changed |= ui.add(egui::Slider::new(&mut bus.pan,-1.0..=1.0).text("Pan")).changed();
                                    bus_changed |= ui.checkbox(&mut bus.muted,"Mute bus").changed();
                                    bus_changed |= filter_controls(ui,&mut bus.filter,&format!("bus-filter-{index}"));
                                }
                            }
                        } else {
                            ui.label("Empty pad. Load a WAV to give it a voice.");
                        }
                        if self.page == Page::Edit {
                            ui.horizontal(|ui| {
                                if ui.button("Load WAV").clicked() { action = Some(Action::Import); }
                                if ui.add_enabled(self.kit.pads[self.selected].is_some(), egui::Button::new("Save WAV")).clicked() { action = Some(Action::Export); }
                            });
                            ui.horizontal_wrapped(|ui| {
                                let has_pad = self.kit.pads[self.selected].is_some();
                                if ui.add_enabled(has_pad, egui::Button::new("Crop to trim")).clicked() { action = Some(Action::Crop); }
                                if ui.add_enabled(has_pad, egui::Button::new("Normalize")).clicked() { action = Some(Action::Normalize); }
                                if let Some(undo) = &self.sample_undo
                                    && ui.button(format!("Undo {} / pad {:02}", undo.label, undo.slot + 1)).clicked()
                                {
                                    action = Some(Action::UndoSample);
                                }
                            });
                            theme::label(ui, "SAMPLE EFFECT / DESTRUCTIVE");
                            egui::ComboBox::from_id_salt("pad-sample-effect")
                                .selected_text(EffectType::of(&self.sample_effect).label())
                                .show_ui(ui, |ui| {
                                    for kind in EffectType::ALL {
                                        if ui.selectable_label(false, kind.label()).clicked() {
                                            self.sample_effect = kind.default_effect();
                                            ui.close();
                                        }
                                    }
                                });
                            effect_ui::effect_controls(ui, &mut self.sample_effect);
                            if ui.add_enabled(self.kit.pads[self.selected].is_some(), egui::Button::new("Apply effect to sample")).clicked() {
                                action = Some(Action::ApplyEffect);
                            }
                            ui.label(RichText::new("Effect tails are kept; trim and playback modes are preserved. One-step undo is available.").small().color(theme::MUTED));
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
                                if self.midi_held & (1 << slot) == 0 {
                                    hits.push(PadHit {
                                        slot: slot as u8,
                                        velocity: self.velocity,
                                        at: wall_now,
                                    });
                                }
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
        if bus_changed {
            self.dirty = true;
            if let Some(player) = &self.player
                && let Err(e) = player.update_buses(&self.kit)
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
                if self.midi_held != 0 {
                    self.midi_velocity
                } else {
                    self.velocity
                },
                self.repeat.then_some(self.rate),
                tempo,
                self.kit.gain,
            );
        }
        self.held = desired;
        if !panic && let Some(player) = &self.player {
            hits.extend(player.repeated_hits().into_iter().map(|hit| PadHit {
                slot: hit.slot,
                velocity: hit.velocity,
                at: hit.at,
            }));
        }
        if let Some(action) = action {
            self.action(action);
        }
        hits
    }
}

fn filter_controls(ui: &mut egui::Ui, filter: &mut Option<DrumFilter>, id: &str) -> bool {
    ui.push_id(id, |ui| {
        let mut changed = false;
        let mut enabled = filter.is_some();
        if ui.checkbox(&mut enabled, "Filter").changed() {
            *filter = enabled.then(DrumFilter::default);
            changed = true;
        }
        if let Some(settings) = filter {
            ui.horizontal_wrapped(|ui| {
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut settings.cutoff_hz)
                            .range(1.0..=96_000.0)
                            .prefix("Cutoff Hz "),
                    )
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut settings.resonance, 0.0..=1.0).text("Resonance"))
                    .changed();
            });
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(match settings.mode {
                        evil_synth::FilterMode::LowPass => "Low-pass",
                        evil_synth::FilterMode::BandPass => "Band-pass",
                        evil_synth::FilterMode::HighPass => "High-pass",
                    })
                    .show_ui(ui, |ui| {
                        for (mode, label) in [
                            (evil_synth::FilterMode::LowPass, "Low-pass"),
                            (evil_synth::FilterMode::BandPass, "Band-pass"),
                            (evil_synth::FilterMode::HighPass, "High-pass"),
                        ] {
                            changed |= ui
                                .selectable_value(&mut settings.mode, mode, label)
                                .changed();
                        }
                    });
                egui::ComboBox::from_id_salt("slope")
                    .selected_text(format!("{} dB/oct", settings.stages * 6))
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(&mut settings.stages, 2, "12 dB/oct")
                            .changed();
                        changed |= ui
                            .selectable_value(&mut settings.stages, 4, "24 dB/oct")
                            .changed();
                    });
            });
        }
        changed
    })
    .inner
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_undo_restores_audio_and_trim_without_reverting_other_controls() {
        let mut pad = DrumPad::new(vec![0.0, 0.25, -0.5, 0.75], 48_000);
        pad.playback_start_frame = 1;
        pad.playback_end_frame = Some(3);
        let original_samples = pad.samples.clone();
        let undo = SampleUndo::capture(4, &pad, "crop");
        pad.crop_to_playback_range().unwrap();
        pad.normalize().unwrap();
        pad.apply_effect(
            &evil_synth::InsertEffect::Bitcrush(evil_synth::Bitcrush {
                bits: 1,
                downsample: 2,
                mix: 1.0,
            }),
            120.0,
        )
        .unwrap();
        pad.direction = SampleDirection::Reverse;
        pad.gain = 0.25;

        assert_eq!(undo.slot, 4);
        undo.restore(&mut pad);
        assert_eq!(pad.samples, original_samples);
        assert_eq!(pad.playback_range(), 1..3);
        assert_eq!(pad.direction, SampleDirection::Reverse);
        assert_eq!(pad.gain, 0.25);
    }
}
