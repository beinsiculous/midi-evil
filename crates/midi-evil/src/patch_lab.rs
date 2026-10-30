use std::path::Path;

use eframe::egui::{self, ComboBox, DragValue, Ui};
use evil_synth::{Delay, DelayMode, FilterMode, Lfo, LfoDestination, LfoWaveform, Patch, Waveform};

pub struct PatchLab {
    pub patch: Patch,
    pub file_name: String,
    pub track_index: usize,
    original_patch: Patch,
}

impl PatchLab {
    pub fn new(patch: Patch, file_name: String, track_index: usize) -> Self {
        Self {
            original_patch: patch.clone(),
            patch,
            file_name,
            track_index,
        }
    }

    pub fn new_preset(track_index: usize) -> Self {
        Self::new(Patch::default(), "untitled.json".to_owned(), track_index)
    }

    pub fn duplicate(&mut self) {
        self.file_name = duplicate_file_name(&self.file_name);
        self.original_patch = self.patch.clone();
    }

    pub fn reset(&mut self) {
        self.patch = self.original_patch.clone();
    }

    pub fn mark_saved(&mut self, file_name: String) {
        self.file_name = file_name;
        self.original_patch = self.patch.clone();
    }
}

fn duplicate_file_name(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled");
    let duplicate = format!("{stem}-copy.json");
    path.with_file_name(duplicate)
        .to_string_lossy()
        .into_owned()
}

pub fn show(ui: &mut Ui, patch: &mut Patch) {
    ui.horizontal(|ui| {
        ui.label("Patch name");
        ui.text_edit_singleline(&mut patch.name);
        ui.label("Sample rate");
        ui.add(DragValue::new(&mut patch.sample_rate).range(8_000..=192_000));
    });
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut patch.gain, 0.0..=1.0).text("Gain"));
        ui.checkbox(&mut patch.pitch_tracking, "Pitch tracking");
        ui.label("Duration");
        ui.add(DragValue::new(&mut patch.duration_seconds).range(0.001..=60.0));
    });
    ui.horizontal(|ui| {
        let mut has_program = patch.midi_program.is_some();
        if ui
            .checkbox(&mut has_program, "General MIDI program")
            .changed()
        {
            patch.midi_program = has_program.then_some(80);
        }
        if let Some(program) = &mut patch.midi_program {
            ui.add(DragValue::new(program).range(0..=127));
        }
        ui.label("Bend range");
        ui.add(DragValue::new(&mut patch.pitch_bend_range_semitones).range(0.01..=24.0));
    });

    ui.heading("Oscillators");
    for (index, oscillator) in patch.oscillators.iter_mut().enumerate() {
        ui.collapsing(format!("Oscillator {}", index + 1), |ui| {
            ui.horizontal(|ui| {
                ui.label("Waveform");
                ComboBox::from_id_salt(("waveform", index))
                    .selected_text(format!("{:?}", oscillator.waveform))
                    .show_ui(ui, |ui| {
                        for (label, waveform) in [
                            ("Sine", Waveform::Sine),
                            ("Square", Waveform::Square),
                            ("Square (naive)", Waveform::SquareNaive),
                            ("Saw", Waveform::Saw),
                            ("Triangle", Waveform::Triangle),
                            ("Noise", Waveform::Noise),
                        ] {
                            ui.selectable_value(&mut oscillator.waveform, waveform, label);
                        }
                    });
                ui.label("Amplitude");
                ui.add(egui::Slider::new(&mut oscillator.amplitude, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Start Hz");
                ui.add(DragValue::new(&mut oscillator.start_frequency_hz).range(0.01..=24_000.0));
                ui.label("End Hz");
                ui.add(DragValue::new(&mut oscillator.end_frequency_hz).range(0.01..=24_000.0));
            });
            ui.horizontal(|ui| {
                ui.label("Detune cents");
                ui.add(DragValue::new(&mut oscillator.detune_cents).range(-2_400..=2_400));
                if matches!(
                    oscillator.waveform,
                    Waveform::Square | Waveform::SquareNaive
                ) {
                    ui.label("Pulse width");
                    ui.add(egui::Slider::new(&mut oscillator.pulse_width, 0.01..=0.99));
                }
            });
        });
    }
    ui.horizontal(|ui| {
        if patch.oscillators.len() < 8 && ui.button("Add oscillator").clicked() {
            patch.oscillators.push(Default::default());
        }
        if patch.oscillators.len() > 1 && ui.button("Remove last oscillator").clicked() {
            patch.oscillators.pop();
        }
    });

    ui.heading("Oscillator sync");
    let mut sync_enabled = patch.sync.is_some();
    if ui.checkbox(&mut sync_enabled, "Enable sync").changed() {
        patch.sync = sync_enabled.then(Default::default);
    }
    if let Some(sync) = &mut patch.sync {
        ui.add(egui::Slider::new(&mut sync.ratio, 0.01..=16.0).text("Base ratio"));
    }

    ui.heading("Unison");
    let mut unison_enabled = patch.unison.is_some();
    if ui.checkbox(&mut unison_enabled, "Enable unison").changed() {
        patch.unison = unison_enabled.then(Default::default);
    }
    if let Some(unison) = &mut patch.unison {
        ui.horizontal(|ui| {
            ui.label("Voices");
            ui.add(DragValue::new(&mut unison.voices).range(1..=8));
            ui.add(egui::Slider::new(&mut unison.detune_cents, 0.0..=100.0).text("Detune cents"));
        });
    }

    ui.heading("Envelope");
    ui.horizontal(|ui| {
        ui.label("Attack");
        ui.add(DragValue::new(&mut patch.envelope.attack_seconds).range(0.0..=60.0));
        ui.label("Decay");
        ui.add(DragValue::new(&mut patch.envelope.decay_seconds).range(0.0..=60.0));
        ui.label("Sustain");
        ui.add(egui::Slider::new(
            &mut patch.envelope.sustain_level,
            0.0..=1.0,
        ));
        ui.label("Release");
        ui.add(DragValue::new(&mut patch.envelope.release_seconds).range(0.0..=60.0));
    });

    ui.heading("Filter");
    let mut filter_enabled = patch.filter.is_some();
    if ui.checkbox(&mut filter_enabled, "Enable filter").changed() {
        patch.filter = filter_enabled.then(Default::default);
    }
    if let Some(filter) = &mut patch.filter {
        ui.horizontal(|ui| {
            ui.label("Cutoff Hz");
            ui.add(DragValue::new(&mut filter.cutoff_hz).range(1.0..=100_000.0));
            ui.label("Envelope octaves");
            ui.add(DragValue::new(&mut filter.envelope_amount).range(-12.0..=12.0));
        });
        ComboBox::from_label("Mode")
            .selected_text(format!("{:?}", filter.mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filter.mode, FilterMode::LowPass, "Low-pass");
                ui.selectable_value(&mut filter.mode, FilterMode::HighPass, "High-pass");
            });
        ui.horizontal(|ui| {
            ui.label("Slope stages");
            ui.add(DragValue::new(&mut filter.stages).range(1..=4));
            ui.add(egui::Slider::new(&mut filter.resonance, 0.0..=1.0).text("Resonance"));
            ui.checkbox(&mut filter.key_tracking, "Track keys");
        });
    }

    ui.heading("LFO modulation");
    let mut remove_lfo = None;
    for index in 0..patch.lfo_count() {
        ui.collapsing(format!("LFO {}", index + 1), |ui| {
            if let Some(lfo) = patch.lfo_mut(index) {
                show_lfo(ui, lfo, index);
            }
            if ui.button("Remove lane").clicked() {
                remove_lfo = Some(index);
            }
        });
    }
    if patch.lfo_count() < 8 && ui.button("Add LFO lane").clicked() {
        patch.add_lfo();
    }
    if let Some(index) = remove_lfo {
        patch.remove_lfo(index);
    }

    ui.heading("Delay");
    let mut delay_enabled = patch.delay.is_some();
    if ui.checkbox(&mut delay_enabled, "Enable delay").changed() {
        patch.delay = delay_enabled.then_some(Delay::default());
    }
    if let Some(delay) = &mut patch.delay {
        ui.horizontal(|ui| {
            ui.label("Seconds");
            ui.add(DragValue::new(&mut delay.time_seconds).range(0.001..=2.0));
            ui.add(egui::Slider::new(&mut delay.feedback, 0.0..=0.95).text("Feedback"));
            ui.add(egui::Slider::new(&mut delay.mix, 0.0..=1.0).text("Wet mix"));
        });
        ComboBox::from_label("Delay mode")
            .selected_text(format!("{:?}", delay.mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut delay.mode, DelayMode::Mono, "Mono / dual mono");
                ui.selectable_value(&mut delay.mode, DelayMode::StereoCross, "Stereo cross");
            });
    }
}

fn show_lfo(ui: &mut Ui, lfo: &mut Lfo, index: usize) {
    ui.horizontal(|ui| {
        ComboBox::from_id_salt(("lfo-destination", index))
            .selected_text(format!("{:?}", lfo.destination))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut lfo.destination, LfoDestination::Pitch, "Pitch");
                ui.selectable_value(
                    &mut lfo.destination,
                    LfoDestination::PulseWidth,
                    "Pulse width",
                );
                ui.selectable_value(
                    &mut lfo.destination,
                    LfoDestination::SyncRatio,
                    "Sync ratio",
                );
                ui.selectable_value(
                    &mut lfo.destination,
                    LfoDestination::FilterCutoff,
                    "Filter cutoff",
                );
                ui.selectable_value(&mut lfo.destination, LfoDestination::Amplitude, "Amplitude");
                ui.selectable_value(&mut lfo.destination, LfoDestination::LfoRate, "LFO rate");
            });
        ComboBox::from_id_salt(("lfo-waveform", index))
            .selected_text(format!("{:?}", lfo.waveform))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut lfo.waveform, LfoWaveform::Sine, "Sine");
                ui.selectable_value(&mut lfo.waveform, LfoWaveform::SawDown, "Saw down");
                ui.selectable_value(&mut lfo.waveform, LfoWaveform::SawUp, "Saw up");
                ui.selectable_value(&mut lfo.waveform, LfoWaveform::Triangle, "Triangle");
                ui.selectable_value(&mut lfo.waveform, LfoWaveform::Square, "Square");
            });
    });
    ui.horizontal(|ui| {
        ui.label("Rate Hz");
        ui.add(DragValue::new(&mut lfo.rate_hz).range(0.0..=100.0));
        match lfo.destination {
            LfoDestination::Pitch => {
                ui.label("Depth cents");
                ui.add(DragValue::new(&mut lfo.depth_cents).range(-2_400..=2_400));
            }
            LfoDestination::PulseWidth => {
                ui.add(
                    egui::Slider::new(&mut lfo.pulse_width_depth, -0.99..=0.99).text("Pulse depth"),
                );
            }
            LfoDestination::SyncRatio => {
                ui.add(egui::Slider::new(&mut lfo.sync_ratio_depth, -4.0..=4.0).text("Sync depth"));
            }
            LfoDestination::FilterCutoff => {
                ui.add(
                    egui::Slider::new(&mut lfo.filter_cutoff_depth, -4.0..=4.0)
                        .text("Cutoff octaves"),
                );
            }
            LfoDestination::Amplitude => {
                ui.add(
                    egui::Slider::new(&mut lfo.amplitude_depth, -1.0..=1.0).text("Amplitude depth"),
                );
            }
            LfoDestination::LfoRate => {
                ui.add(
                    egui::Slider::new(&mut lfo.lfo_rate_depth, -4.0..=4.0)
                        .text("Rate depth (relative)"),
                );
                if let Some(mut target) = lfo.target_lfo {
                    if ui
                        .add(
                            DragValue::new(&mut target)
                                .range(0..=7)
                                .prefix("Target LFO "),
                        )
                        .changed()
                    {
                        lfo.target_lfo = Some(target);
                    }
                } else {
                    lfo.target_lfo = Some(0);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_and_reset_preserve_identity_and_track_assignment() {
        let patch = Patch {
            name: "Preserved name".to_owned(),
            midi_program: Some(42),
            ..Patch::default()
        };
        let mut lab = PatchLab::new(patch.clone(), "patches/lead.json".to_owned(), 2);
        lab.patch.gain = 0.4;
        lab.duplicate();
        let duplicated = lab.patch.clone();

        assert_eq!(lab.file_name, "patches/lead-copy.json");
        assert_eq!(lab.patch.name, "Preserved name");
        assert_eq!(lab.patch.midi_program, Some(42));
        assert_eq!(lab.track_index, 2);

        lab.patch.name = "Temporary edit".to_owned();
        lab.patch.midi_program = None;
        lab.reset();
        assert_eq!(lab.patch, duplicated);
        assert_eq!(lab.file_name, "patches/lead-copy.json");
        assert_eq!(lab.track_index, 2);
        assert_ne!(lab.patch, patch);
    }

    #[test]
    fn saving_establishes_a_new_reset_point_without_changing_the_track() {
        let mut lab = PatchLab::new_preset(1);
        lab.patch.name = "Saved patch".to_owned();
        lab.mark_saved("patches/saved.json".to_owned());
        lab.patch.name = "Unsaved edit".to_owned();
        lab.reset();

        assert_eq!(lab.patch.name, "Saved patch");
        assert_eq!(lab.file_name, "patches/saved.json");
        assert_eq!(lab.track_index, 1);
    }
}
