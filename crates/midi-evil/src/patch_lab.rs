use std::path::Path;

use crate::theme;
use eframe::egui::{self, ComboBox, DragValue, Ui};

use evil_synth::{
    AuxEnvelope, Delay, DelayMode, Envelope, FilterMode, Lfo, LfoDestination, LfoWaveform,
    Oscillator, OscillatorMode, Patch, SampleOscillator, SampleZone, VoiceMode, Waveform,
};

pub struct PatchLab {
    pub patch: Patch,
    pub file_name: String,
    pub track_index: usize,
    original_patch: Patch,
    modulation_page: bool,
}

impl PatchLab {
    pub fn new(patch: Patch, file_name: String, track_index: usize) -> Self {
        Self {
            original_patch: patch.clone(),
            patch,
            file_name,
            track_index,
            modulation_page: false,
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

pub fn show(ui: &mut Ui, lab: &mut PatchLab) -> Option<String> {
    let mut message = None;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut lab.modulation_page, false, "VOICE & FILTER");
        ui.selectable_value(&mut lab.modulation_page, true, "MODULATION & FX");
        ui.label(
            egui::RichText::new("Save As assigns the patch to its track")
                .small()
                .color(theme::MUTED),
        );
    });
    ui.add_space(8.0);
    let patch = &mut lab.patch;
    theme::section(ui, "PRESET / OUTPUT", theme::IVORY, |ui| {
        show_output(ui, patch);
    });
    ui.add_space(10.0);
    if ui.available_width() >= 750.0 {
        ui.columns(2, |columns| {
            if lab.modulation_page {
                theme::section(&mut columns[0], "01 / MODULATION", theme::PURPLE, |ui| {
                    show_motion(ui, patch)
                });
                theme::section(&mut columns[1], "02 / EFFECTS", theme::GREEN, |ui| {
                    show_effects(ui, patch)
                });
            } else {
                theme::section(&mut columns[0], "01 / OSCILLATOR BANK", theme::RED, |ui| {
                    message = show_voice(ui, patch);
                });
                theme::section(
                    &mut columns[1],
                    "02 / ENVELOPE & FILTER",
                    theme::IVORY,
                    |ui| show_shape(ui, patch),
                );
            }
        });
    } else if lab.modulation_page {
        theme::section(ui, "01 / MODULATION", theme::PURPLE, |ui| {
            show_motion(ui, patch)
        });
        ui.add_space(10.0);
        theme::section(ui, "02 / EFFECTS", theme::GREEN, |ui| {
            show_effects(ui, patch)
        });
    } else {
        theme::section(ui, "01 / OSCILLATOR BANK", theme::RED, |ui| {
            message = show_voice(ui, patch);
        });
        ui.add_space(10.0);
        theme::section(ui, "02 / ENVELOPE & FILTER", theme::IVORY, |ui| {
            show_shape(ui, patch)
        });
    }
    message
}

fn show_output(ui: &mut Ui, patch: &mut Patch) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Patch name");
        ui.add(egui::TextEdit::singleline(&mut patch.name).desired_width(200.0));
        ui.label("Sample rate");
        ui.add(DragValue::new(&mut patch.sample_rate).range(8_000..=192_000));
    });
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Slider::new(&mut patch.gain, 0.0..=1.0).text("Gain"));
        ui.checkbox(&mut patch.pitch_tracking, "Pitch tracking");
        ui.label("Duration");
        ui.add(DragValue::new(&mut patch.duration_seconds).range(0.001..=60.0));
    });
    ui.horizontal_wrapped(|ui| {
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
}

fn show_voice(ui: &mut Ui, patch: &mut Patch) -> Option<String> {
    let mut message = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Voice mode");
        ComboBox::from_id_salt("voice_mode")
            .selected_text(format!("{:?}", patch.voice_mode))
            .show_ui(ui, |ui| {
                for (label, mode) in [
                    ("Poly", VoiceMode::Poly),
                    ("Mono", VoiceMode::Mono),
                    ("Legato", VoiceMode::Legato),
                ] {
                    ui.selectable_value(&mut patch.voice_mode, mode, label);
                }
            });
        ui.label("Osc A/B mode");
        let mut mode = patch.effective_oscillator_mode();
        let previous_mode = mode;
        ComboBox::from_id_salt("oscillator_mode")
            .selected_text(format!("{mode:?}"))
            .show_ui(ui, |ui| {
                for (label, value) in [
                    ("Mix", OscillatorMode::Mix),
                    ("Sync", OscillatorMode::Sync),
                    ("Ring", OscillatorMode::Ring),
                ] {
                    ui.selectable_value(&mut mode, value, label);
                }
            });
        if mode != previous_mode {
            patch.oscillator_mode = mode;
            patch.sync =
                (mode == OscillatorMode::Sync).then(|| patch.sync.clone().unwrap_or_default());
            if mode != OscillatorMode::Mix && patch.oscillators.len() < 2 {
                patch.oscillators.push(Default::default());
            }
        }
    });
    ui.heading("Oscillators");
    let sample_oscillator = patch
        .oscillators
        .iter()
        .position(|oscillator| oscillator.waveform == Waveform::Sample);
    for (index, oscillator) in patch.oscillators.iter_mut().enumerate() {
        let label = match index {
            0 => "Osc A".to_owned(),
            1 => "Osc B".to_owned(),
            _ => format!("Auxiliary oscillator {}", index - 1),
        };
        egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
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
                            if (oscillator.sample.is_some() || !oscillator.sample_zones.is_empty())
                                && sample_oscillator
                                    .is_none_or(|sample_index| sample_index == index)
                            {
                                ui.selectable_value(
                                    &mut oscillator.waveform,
                                    Waveform::Sample,
                                    "Sample",
                                );
                            }
                        });
                    ui.label("Amplitude");
                    ui.add(egui::Slider::new(&mut oscillator.amplitude, 0.0..=1.0));
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Start Hz");
                    ui.add(
                        DragValue::new(&mut oscillator.start_frequency_hz).range(0.01..=24_000.0),
                    );
                    ui.label("End Hz");
                    ui.add(DragValue::new(&mut oscillator.end_frequency_hz).range(0.01..=24_000.0));
                });
                ui.horizontal_wrapped(|ui| {
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
                if oscillator.sample_zones.is_empty()
                    && sample_oscillator.is_none_or(|sample_index| sample_index == index)
                {
                    let button = if oscillator.sample.is_some() {
                        "Replace sample WAV"
                    } else {
                        "Load sample WAV"
                    };
                    if ui.button(button).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("WAV audio", &["wav"])
                            .pick_file()
                    {
                        message = Some(match import_sample(oscillator, index, &path) {
                            Ok(message) => message,
                            Err(error) => error.to_string(),
                        });
                    }
                }
                if oscillator.waveform == Waveform::Sample {
                    message = show_sample_mapping(ui, oscillator, index);
                }
            });
    }
    ui.horizontal_wrapped(|ui| {
        if patch.oscillators.len() < 8 && ui.button("Add oscillator").clicked() {
            patch.oscillators.push(Default::default());
        }
        if patch.oscillators.len() > 1
            && (patch.effective_oscillator_mode() != OscillatorMode::Ring
                || patch.oscillators.len() > 2)
            && ui.button("Remove last oscillator").clicked()
        {
            patch.oscillators.pop();
        }
    });

    if patch.uses_sync() {
        ui.heading("Oscillator sync");
        let sync = patch.sync.get_or_insert_with(Default::default);
        ui.add(egui::Slider::new(&mut sync.ratio, 0.01..=16.0).text("Base ratio"));
    }

    ui.heading("Unison");
    let mut unison_enabled = patch.unison.is_some();
    if ui.checkbox(&mut unison_enabled, "Enable unison").changed() {
        patch.unison = unison_enabled.then(Default::default);
    }
    if let Some(unison) = &mut patch.unison {
        ui.horizontal_wrapped(|ui| {
            ui.label("Voices");
            ui.add(DragValue::new(&mut unison.voices).range(1..=8));
            ui.add(egui::Slider::new(&mut unison.detune_cents, 0.0..=100.0).text("Detune cents"));
        });
    }
    message
}

fn import_sample(
    oscillator: &mut Oscillator,
    index: usize,
    path: &Path,
) -> Result<String, evil_synth::Error> {
    let sample = SampleOscillator::read_wav(path)?;
    let name = sample.name.clone();
    oscillator.sample = Some(sample);
    oscillator.waveform = Waveform::Sample;
    Ok(format!("Loaded WAV '{name}' into oscillator {}", index + 1))
}

fn show_sample_mapping(
    ui: &mut Ui,
    oscillator: &mut Oscillator,
    oscillator_index: usize,
) -> Option<String> {
    let mut message = None;
    if oscillator.sample_zones.is_empty() {
        if let Some(sample) = &mut oscillator.sample {
            show_sample_controls(ui, sample);
        }
        if ui.button("Add key zone").clicked() && add_sample_zone(oscillator) {
            message = Some("Split the full keyboard into two identical sample zones".to_owned());
        }
        return message;
    }

    oscillator
        .sample_zones
        .sort_unstable_by_key(|zone| (zone.low_key, zone.high_key));
    ui.label(
        egui::RichText::new("Inclusive MIDI key ranges · selected at note-on · gaps are silent")
            .small()
            .color(theme::MUTED),
    );
    let zone_count = oscillator.sample_zones.len();
    let mut remove_zone = None;
    for zone_index in 0..zone_count {
        let title = {
            let zone = &oscillator.sample_zones[zone_index];
            format!(
                "Zone {} · {} · keys {}–{}",
                zone_index + 1,
                zone.sample.name,
                zone.low_key,
                zone.high_key
            )
        };
        let minimum_low = zone_index
            .checked_sub(1)
            .map_or(0, |previous| oscillator.sample_zones[previous].high_key + 1);
        let maximum_high = oscillator
            .sample_zones
            .get(zone_index + 1)
            .map_or(127, |next| next.low_key - 1);
        egui::CollapsingHeader::new(title)
            .default_open(true)
            .show(ui, |ui| {
                let zone = &mut oscillator.sample_zones[zone_index];
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        DragValue::new(&mut zone.low_key)
                            .range(minimum_low..=zone.high_key)
                            .prefix("Low key "),
                    );
                    ui.add(
                        DragValue::new(&mut zone.high_key)
                            .range(zone.low_key..=maximum_high)
                            .prefix("High key "),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Load / replace zone WAV").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("WAV audio", &["wav"])
                            .pick_file()
                    {
                        message = Some(
                            match import_zone_sample(zone, oscillator_index, zone_index, &path) {
                                Ok(message) => message,
                                Err(error) => error.to_string(),
                            },
                        );
                    }
                    if zone_count > 1 && ui.button("Remove zone").clicked() {
                        remove_zone = Some(zone_index);
                    }
                });
                show_sample_controls(ui, &mut zone.sample);
            });
    }
    if let Some(index) = remove_zone {
        oscillator.sample_zones.remove(index);
        message = Some(format!("Removed sample zone {}", index + 1));
    }
    if zone_count < 128
        && ui.button("Add key zone (duplicate & split)").clicked()
        && add_sample_zone(oscillator)
    {
        message = Some("Duplicated and split a sample zone".to_owned());
    }
    message
}

fn add_sample_zone(oscillator: &mut Oscillator) -> bool {
    if oscillator.sample_zones.is_empty() {
        let Some(sample) = oscillator.sample.take() else {
            return false;
        };
        oscillator.sample_zones = vec![
            SampleZone::new(0, 59, sample.clone()),
            SampleZone::new(60, 127, sample),
        ];
        return true;
    }

    let zone_width = |zone: &SampleZone| u16::from(zone.high_key - zone.low_key) + 1;
    let mut widest = 0;
    for index in 1..oscillator.sample_zones.len() {
        if zone_width(&oscillator.sample_zones[index])
            > zone_width(&oscillator.sample_zones[widest])
        {
            widest = index;
        }
    }
    if zone_width(&oscillator.sample_zones[widest]) == 1 {
        return false;
    }
    let zone = &oscillator.sample_zones[widest];
    let high_key = zone.high_key;
    let midpoint = zone.low_key + (zone.high_key - zone.low_key) / 2;
    let sample = zone.sample.clone();
    oscillator.sample_zones[widest].high_key = midpoint;
    oscillator
        .sample_zones
        .insert(widest + 1, SampleZone::new(midpoint + 1, high_key, sample));
    true
}

fn import_zone_sample(
    zone: &mut SampleZone,
    oscillator_index: usize,
    zone_index: usize,
    path: &Path,
) -> Result<String, evil_synth::Error> {
    let sample = SampleOscillator::read_wav(path)?;
    let name = sample.name.clone();
    zone.sample = sample;
    Ok(format!(
        "Loaded WAV '{name}' into oscillator {} zone {}",
        oscillator_index + 1,
        zone_index + 1
    ))
}

fn show_sample_controls(ui: &mut Ui, sample: &mut SampleOscillator) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "{} · {} frames · {} Hz",
            sample.name,
            sample.samples.len(),
            sample.sample_rate
        ));
        ui.checkbox(&mut sample.loop_enabled, "Loop");
    });
    sample_waveform(ui, sample);

    let frame_count = sample.samples.len();
    let mut start = sample.loop_start_frame;
    let mut end = sample.loop_end_frame.unwrap_or(frame_count);
    let start_changed = ui
        .add(
            DragValue::new(&mut start)
                .range(0..=frame_count - 1)
                .prefix("Loop start frame "),
        )
        .changed();
    let end_changed = ui
        .add(
            DragValue::new(&mut end)
                .range(1..=frame_count)
                .prefix("Loop end frame (exclusive) "),
        )
        .changed();
    if start_changed {
        end = end.max(start + 1).min(frame_count);
    }
    if end_changed {
        start = start.min(end - 1);
    }
    if start_changed || end_changed {
        sample.loop_start_frame = start;
        sample.loop_end_frame = Some(end);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Full-source loop").clicked() {
            sample.loop_start_frame = 0;
            sample.loop_end_frame = None;
        }
        ui.label(
            egui::RichText::new("One-shots ignore loop points and play the full source")
                .small()
                .color(theme::MUTED),
        );
    });
    let maximum_root = sample.sample_rate as f32 / 2.0;
    ui.add(
        DragValue::new(&mut sample.root_frequency_hz)
            .range(0.01..=maximum_root)
            .prefix("Root Hz "),
    );
}

fn sample_waveform(ui: &mut Ui, sample: &SampleOscillator) {
    let (response, painter) =
        ui.allocate_painter(egui::vec2(ui.available_width(), 88.0), egui::Sense::hover());
    painter.rect_filled(response.rect, 3.0, theme::BACKGROUND);
    let plot = response.rect.shrink(8.0);
    let frame_count = sample.samples.len();
    if frame_count == 0 || plot.width() <= 0.0 {
        return;
    }
    let frame_x = |frame: usize| plot.left() + frame as f32 / frame_count as f32 * plot.width();
    let loop_range = sample.loop_range();
    if sample.loop_enabled && loop_range.start < loop_range.end && loop_range.end <= frame_count {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(frame_x(loop_range.start), plot.top()),
                egui::pos2(frame_x(loop_range.end), plot.bottom()),
            ),
            0.0,
            theme::PURPLE.gamma_multiply(0.12),
        );
    }
    painter.line_segment(
        [
            egui::pos2(plot.left(), plot.center().y),
            egui::pos2(plot.right(), plot.center().y),
        ],
        egui::Stroke::new(1.0, theme::BORDER.gamma_multiply(0.5)),
    );
    let bars = plot.width().min(180.0) as usize;
    for bar in 0..bars {
        let start = bar * frame_count / bars;
        let end = ((bar + 1) * frame_count / bars)
            .max(start + 1)
            .min(frame_count);
        let peak = sample.samples[start..end]
            .iter()
            .fold(0.0_f32, |peak, value| peak.max(value.abs()))
            .min(1.0);
        let x = plot.left() + bar as f32 / bars as f32 * plot.width();
        let height = (peak * plot.height() * 0.45).max(0.5);
        painter.line_segment(
            [
                egui::pos2(x, plot.center().y - height),
                egui::pos2(x, plot.center().y + height),
            ],
            egui::Stroke::new(1.0, theme::IVORY.gamma_multiply(0.75)),
        );
    }
    for (frame, label, color, alignment) in [
        (loop_range.start, "S", theme::GREEN, egui::Align2::LEFT_TOP),
        (loop_range.end, "E", theme::RED, egui::Align2::RIGHT_TOP),
    ] {
        if frame > frame_count {
            continue;
        }
        let color = if sample.loop_enabled {
            color
        } else {
            theme::MUTED
        };
        let x = frame_x(frame);
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            egui::Stroke::new(1.5, color),
        );
        painter.text(
            egui::pos2(x, plot.top() + 2.0),
            alignment,
            label,
            egui::FontId::monospace(11.0),
            color,
        );
    }
    response.on_hover_text(format!(
        "Embedded waveform · loop {}..{} frames (end exclusive){}",
        loop_range.start,
        loop_range.end,
        if sample.loop_enabled {
            ""
        } else {
            " · disabled; one-shot uses the full source"
        }
    ));
}

fn show_shape(ui: &mut Ui, patch: &mut Patch) {
    envelope_plot(ui, &patch.envelope, "Amplitude envelope");
    theme::label(ui, "AMPLITUDE ENVELOPE");
    show_envelope_controls(ui, &mut patch.envelope);

    ui.heading("Filter");
    let mut filter_enabled = patch.filter.is_some();
    if ui.checkbox(&mut filter_enabled, "Enable filter").changed() {
        patch.filter = filter_enabled.then(Default::default);
    }
    if let Some(filter) = &mut patch.filter {
        ui.horizontal_wrapped(|ui| {
            ui.label("Cutoff Hz");
            ui.add(DragValue::new(&mut filter.cutoff_hz).range(1.0..=100_000.0));
            ui.label("Envelope octaves");
            ui.add(DragValue::new(&mut filter.envelope_amount).range(-12.0..=12.0));
        });
        ComboBox::from_label("Mode")
            .selected_text(format!("{:?}", filter.mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filter.mode, FilterMode::LowPass, "Low-pass");
                ui.selectable_value(&mut filter.mode, FilterMode::BandPass, "Band-pass");
                ui.selectable_value(&mut filter.mode, FilterMode::HighPass, "High-pass");
            });
        ui.horizontal_wrapped(|ui| {
            ui.label("Slope (6 dB stages)");
            ui.add(DragValue::new(&mut filter.stages).range(1..=4))
                .on_hover_text(
                    "2 = 12 dB; 4 = 24 dB. Band-pass skirts are half the nominal slope.",
                );
            ui.add(egui::Slider::new(&mut filter.resonance, 0.0..=1.0).text("Resonance"));
            ui.checkbox(&mut filter.key_tracking, "Track keys");
        });
    }

    ui.heading("Filter envelope");
    let mut independent = patch.filter_envelope.is_some();
    if ui
        .checkbox(&mut independent, "Independent filter ADSR")
        .changed()
    {
        patch.filter_envelope = independent.then(|| patch.envelope.clone());
    }
    if let Some(envelope) = &mut patch.filter_envelope {
        envelope_plot(ui, envelope, "Filter envelope");
        show_envelope_controls(ui, envelope);
    } else {
        ui.label("Routed from the amplitude envelope");
    }
}

fn show_envelope_controls(ui: &mut Ui, envelope: &mut Envelope) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Attack");
        ui.add(DragValue::new(&mut envelope.attack_seconds).range(0.0..=60.0));
        ui.label("Decay");
        ui.add(DragValue::new(&mut envelope.decay_seconds).range(0.0..=60.0));
        ui.label("Sustain");
        ui.add(egui::Slider::new(&mut envelope.sustain_level, 0.0..=1.0));
        ui.label("Release");
        ui.add(DragValue::new(&mut envelope.release_seconds).range(0.0..=60.0));
    });
}

fn show_motion(ui: &mut Ui, patch: &mut Patch) {
    ui.heading("Auxiliary envelope / matrix");
    let mut aux_enabled = patch.aux_envelope.is_some();
    if ui
        .checkbox(&mut aux_enabled, "Enable auxiliary ADSR")
        .changed()
    {
        patch.aux_envelope = aux_enabled.then_some(AuxEnvelope::default());
    }
    if let Some(aux) = &mut patch.aux_envelope {
        envelope_plot(ui, &aux.envelope, "Auxiliary envelope");
        show_envelope_controls(ui, &mut aux.envelope);
        ui.label("Signed depths add to existing LFO and filter-envelope modulation");
        ui.add(egui::Slider::new(&mut aux.amplitude_depth, -1.0..=1.0).text("Amplitude depth"));
        ui.add(
            egui::Slider::new(&mut aux.pitch_depth_cents, -2_400.0..=2_400.0)
                .text("Pitch depth (cents)"),
        );
        ui.add(
            egui::Slider::new(&mut aux.filter_cutoff_depth_octaves, -12.0..=12.0)
                .text("Cutoff depth (octaves)"),
        );
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
}

fn show_effects(ui: &mut Ui, patch: &mut Patch) {
    ui.heading("Delay");
    let mut delay_enabled = patch.delay.is_some();
    if ui.checkbox(&mut delay_enabled, "Enable delay").changed() {
        patch.delay = delay_enabled.then_some(Delay::default());
    }
    if let Some(delay) = &mut patch.delay {
        ui.horizontal_wrapped(|ui| {
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

fn envelope_plot(ui: &mut Ui, envelope: &Envelope, label: &str) {
    let (response, painter) =
        ui.allocate_painter(egui::vec2(ui.available_width(), 88.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BACKGROUND);
    let plot = rect.shrink(12.0);
    let hold = 0.25;
    let total =
        (envelope.attack_seconds + envelope.decay_seconds + hold + envelope.release_seconds)
            .max(0.01);
    let point = |t: f32, level: f32| {
        egui::pos2(
            plot.left() + t / total * plot.width(),
            plot.bottom() - level * plot.height(),
        )
    };
    let sustain_at = envelope.attack_seconds + envelope.decay_seconds;
    let points = vec![
        point(0.0, 0.0),
        point(envelope.attack_seconds, 1.0),
        point(sustain_at, envelope.sustain_level),
        point(sustain_at + hold, envelope.sustain_level),
        point(total, 0.0),
    ];
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, theme::IVORY),
    ));
    response.on_hover_text(format!(
        "{label} · A / D / S / R (sustain shown for 250 ms)"
    ));
}

fn show_lfo(ui: &mut Ui, lfo: &mut Lfo, index: usize) {
    ui.horizontal_wrapped(|ui| {
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
    ui.horizontal_wrapped(|ui| {
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
    fn sample_import_selects_the_waveform_and_embeds_wav_audio() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oscillator.wav");
        evil_synth::RenderedSound {
            samples: vec![0.125],
            sample_rate: 48_000,
        }
        .write_wav(&path)
        .unwrap();

        let mut oscillator = Oscillator::default();
        let message = import_sample(&mut oscillator, 0, &path).unwrap();
        assert!(message.contains("Loaded WAV 'oscillator'"));
        assert_eq!(oscillator.waveform, Waveform::Sample);
        let sample = oscillator.sample.unwrap();
        assert_eq!(sample.name, "oscillator");
        assert_eq!(sample.samples, vec![0.125]);
        assert_eq!(sample.loop_range(), 0..1);

        let mut unchanged = Oscillator::default();
        assert!(import_sample(&mut unchanged, 0, &directory.path().join("missing.wav")).is_err());
        assert_eq!(unchanged, Oscillator::default());
    }

    #[test]
    fn sample_zone_editing_splits_safely_and_replaces_only_the_selected_zone() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.wav");
        evil_synth::RenderedSound {
            samples: vec![0.125, -0.125],
            sample_rate: 8_000,
        }
        .write_wav(&first_path)
        .unwrap();
        let second_path = directory.path().join("second.wav");
        evil_synth::RenderedSound {
            samples: vec![0.5],
            sample_rate: 48_000,
        }
        .write_wav(&second_path)
        .unwrap();

        let mut oscillator = Oscillator::default();
        import_sample(&mut oscillator, 0, &first_path).unwrap();
        let original = oscillator.sample.clone().unwrap();
        assert!(add_sample_zone(&mut oscillator));
        assert!(oscillator.sample.is_none());
        assert_eq!(oscillator.sample_zones.len(), 2);
        assert_eq!(oscillator.sample_zones[0].low_key, 0);
        assert_eq!(oscillator.sample_zones[0].high_key, 59);
        assert_eq!(oscillator.sample_zones[1].low_key, 60);
        assert_eq!(oscillator.sample_zones[1].high_key, 127);
        for key in 0..=127 {
            assert_eq!(oscillator.sample_for_key(key), Some(&original));
        }

        oscillator.sample_zones.remove(0);
        assert!(oscillator.sample_for_key(59).is_none());
        let message =
            import_zone_sample(&mut oscillator.sample_zones[0], 0, 0, &second_path).unwrap();
        assert!(message.contains("oscillator 1 zone 1"));
        assert_eq!(oscillator.sample_zones[0].low_key, 60);
        assert_eq!(oscillator.sample_zones[0].high_key, 127);
        assert_eq!(oscillator.sample_zones[0].sample.name, "second");
        assert_eq!(oscillator.sample_for_key(60).unwrap().samples, vec![0.5]);

        let unchanged = oscillator.clone();
        assert!(
            import_zone_sample(
                &mut oscillator.sample_zones[0],
                0,
                0,
                &directory.path().join("missing.wav")
            )
            .is_err()
        );
        assert_eq!(oscillator, unchanged);
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
