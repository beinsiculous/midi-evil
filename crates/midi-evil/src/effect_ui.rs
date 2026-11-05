use eframe::egui::{ComboBox, DragValue, Slider, Ui};
use evil_synth::{
    Bitcrush, Chorus, Compressor, Delay, DelayMode, Equalizer, Flanger, InsertEffect, Reverb,
    Waveshaper,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectType {
    Waveshaper,
    Delay,
    Bitcrush,
    Equalizer,
    Compressor,
    Chorus,
    Flanger,
    Reverb,
}

impl EffectType {
    pub const ALL: [Self; 8] = [
        Self::Waveshaper,
        Self::Delay,
        Self::Bitcrush,
        Self::Equalizer,
        Self::Compressor,
        Self::Chorus,
        Self::Flanger,
        Self::Reverb,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Waveshaper => "Waveshaper",
            Self::Delay => "Delay",
            Self::Bitcrush => "Bitcrush",
            Self::Equalizer => "Equalizer",
            Self::Compressor => "Compressor",
            Self::Chorus => "Chorus",
            Self::Flanger => "Flanger",
            Self::Reverb => "Reverb",
        }
    }

    pub fn of(effect: &InsertEffect) -> Self {
        match effect {
            InsertEffect::Waveshaper(_) => Self::Waveshaper,
            InsertEffect::Delay(_) => Self::Delay,
            InsertEffect::Bitcrush(_) => Self::Bitcrush,
            InsertEffect::Equalizer(_) => Self::Equalizer,
            InsertEffect::Compressor(_) => Self::Compressor,
            InsertEffect::Chorus(_) => Self::Chorus,
            InsertEffect::Flanger(_) => Self::Flanger,
            InsertEffect::Reverb(_) => Self::Reverb,
        }
    }

    pub fn default_effect(self) -> InsertEffect {
        match self {
            Self::Waveshaper => InsertEffect::Waveshaper(Waveshaper::default()),
            Self::Delay => InsertEffect::Delay(Delay::default()),
            Self::Bitcrush => InsertEffect::Bitcrush(Bitcrush::default()),
            Self::Equalizer => InsertEffect::Equalizer(Equalizer::default()),
            Self::Compressor => InsertEffect::Compressor(Compressor::default()),
            Self::Chorus => InsertEffect::Chorus(Chorus::default()),
            Self::Flanger => InsertEffect::Flanger(Flanger::default()),
            Self::Reverb => InsertEffect::Reverb(Reverb::default()),
        }
    }
}

pub fn optional_effect_selector(ui: &mut Ui, id: &'static str, effect: &mut Option<InsertEffect>) {
    let mut selected = effect.as_ref().map(EffectType::of);
    ComboBox::from_id_salt(id)
        .selected_text(selected.map_or("None", EffectType::label))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, None, "None");
            for kind in EffectType::ALL {
                ui.selectable_value(&mut selected, Some(kind), kind.label());
            }
        });
    let selected_type = effect.as_ref().map(EffectType::of);
    if selected != selected_type {
        *effect = selected.map(EffectType::default_effect);
    }
}

pub fn effect_controls(ui: &mut Ui, effect: &mut InsertEffect) {
    match effect {
        InsertEffect::Waveshaper(effect) => waveshaper_controls(ui, effect),
        InsertEffect::Delay(effect) => delay_controls(ui, effect),
        InsertEffect::Bitcrush(effect) => {
            ui.horizontal_wrapped(|ui| {
                ui.add(Slider::new(&mut effect.bits, 1..=16).text("Bits"));
                ui.add(Slider::new(&mut effect.downsample, 1..=64).text("Downsample"));
                ui.add(Slider::new(&mut effect.mix, 0.0..=1.0).text("Wet mix"));
            });
        }
        InsertEffect::Equalizer(effect) => {
            ui.horizontal_wrapped(|ui| {
                ui.add(Slider::new(&mut effect.low_gain_db, -24.0..=24.0).text("Low dB"));
                ui.add(Slider::new(&mut effect.mid_gain_db, -24.0..=24.0).text("Mid dB"));
                ui.add(Slider::new(&mut effect.high_gain_db, -24.0..=24.0).text("High dB"));
                ui.add(Slider::new(&mut effect.mix, 0.0..=1.0).text("Wet mix"));
            });
        }
        InsertEffect::Compressor(effect) => {
            ui.horizontal_wrapped(|ui| {
                ui.add(Slider::new(&mut effect.threshold_db, -60.0..=0.0).text("Threshold dB"));
                ui.add(Slider::new(&mut effect.ratio, 1.0..=20.0).text("Ratio"));
                ui.add(Slider::new(&mut effect.makeup_gain_db, -12.0..=24.0).text("Makeup dB"));
                ui.add(Slider::new(&mut effect.mix, 0.0..=1.0).text("Wet mix"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    DragValue::new(&mut effect.attack_seconds)
                        .range(0.000_1..=1.0)
                        .speed(0.001)
                        .suffix(" s attack"),
                );
                ui.add(
                    DragValue::new(&mut effect.release_seconds)
                        .range(0.001..=2.0)
                        .speed(0.005)
                        .suffix(" s release"),
                );
            });
        }
        InsertEffect::Chorus(effect) => modulation_controls(
            ui,
            &mut effect.rate_hz,
            &mut effect.depth_seconds,
            &mut effect.delay_seconds,
            &mut effect.feedback,
            &mut effect.mix,
        ),
        InsertEffect::Flanger(effect) => modulation_controls(
            ui,
            &mut effect.rate_hz,
            &mut effect.depth_seconds,
            &mut effect.delay_seconds,
            &mut effect.feedback,
            &mut effect.mix,
        ),
        InsertEffect::Reverb(effect) => {
            ui.horizontal_wrapped(|ui| {
                ui.add(Slider::new(&mut effect.room_size, 0.0..=1.0).text("Room size"));
                ui.add(Slider::new(&mut effect.damping, 0.0..=1.0).text("Damping"));
                ui.add(Slider::new(&mut effect.mix, 0.0..=1.0).text("Wet mix"));
            });
        }
    }
}

pub fn waveshaper_controls(ui: &mut Ui, effect: &mut Waveshaper) {
    ui.horizontal_wrapped(|ui| {
        ui.add(Slider::new(&mut effect.drive, 1.0..=32.0).text("Drive"));
        ui.add(Slider::new(&mut effect.contour, -1.0..=1.0).text("Contour"));
        ui.add(Slider::new(&mut effect.level, 0.0..=1.0).text("Level"));
    });
}

pub fn delay_controls(ui: &mut Ui, effect: &mut Delay) {
    let mut beat_sync = effect.beats_per_cycle.is_some();
    if ui.checkbox(&mut beat_sync, "Beat sync").changed() {
        effect.beats_per_cycle = beat_sync.then_some(1.0);
    }
    ui.horizontal_wrapped(|ui| {
        if let Some(beats) = &mut effect.beats_per_cycle {
            ui.add(
                DragValue::new(beats)
                    .range(0.001..=64.0)
                    .speed(0.05)
                    .suffix(" beats"),
            );
        }
        ui.add_enabled(
            !beat_sync,
            DragValue::new(&mut effect.time_seconds)
                .range(0.001..=2.0)
                .speed(0.005)
                .suffix(" s"),
        );
        ui.add(Slider::new(&mut effect.feedback, 0.0..=0.95).text("Feedback"));
        ui.add(Slider::new(&mut effect.mix, 0.0..=1.0).text("Wet mix"));
    });
    ComboBox::from_label("Delay mode")
        .selected_text(match effect.mode {
            DelayMode::Mono => "Mono / dual mono",
            DelayMode::StereoCross => "Stereo cross",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut effect.mode, DelayMode::Mono, "Mono / dual mono");
            ui.selectable_value(&mut effect.mode, DelayMode::StereoCross, "Stereo cross");
        });
}

fn modulation_controls(
    ui: &mut Ui,
    rate_hz: &mut f32,
    depth_seconds: &mut f32,
    delay_seconds: &mut f32,
    feedback: &mut f32,
    mix: &mut f32,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add(Slider::new(rate_hz, 0.01..=20.0).text("Rate Hz"));
        ui.add(Slider::new(depth_seconds, 0.0..=0.03).text("Depth s"));
        ui.add(Slider::new(delay_seconds, 0.001..=0.1).text("Delay s"));
        ui.add(Slider::new(feedback, 0.0..=0.95).text("Feedback"));
        ui.add(Slider::new(mix, 0.0..=1.0).text("Wet mix"));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_selectable_effect_has_valid_defaults_and_a_stable_type() {
        for kind in EffectType::ALL {
            let effect = kind.default_effect();
            effect.validate().unwrap();
            assert_eq!(EffectType::of(&effect), kind);
        }
    }
}
