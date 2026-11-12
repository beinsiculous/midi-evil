use std::{path::PathBuf, sync::mpsc};

use eframe::egui;

/// The recovery worker writes a separate draft; opening it is an explicit document action.
pub struct RecoveryWindow {
    pub open: bool,
    source: Option<PathBuf>,
    detect_tempo: bool,
    tempo_bpm: f64,
    grid: u8,
    worker: Option<mpsc::Receiver<Result<evil_imports::DraftReport, String>>>,
    report: Option<evil_imports::DraftReport>,
    error: Option<String>,
}

impl Default for RecoveryWindow {
    fn default() -> Self {
        Self {
            open: false,
            source: None,
            detect_tempo: true,
            tempo_bpm: 120.0,
            grid: 16,
            worker: None,
            report: None,
            error: None,
        }
    }
}

impl RecoveryWindow {
    pub fn show(&mut self, context: &egui::Context) -> Option<PathBuf> {
        if let Some(worker) = &self.worker {
            let result = match worker.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Recovery worker disconnected".into()))
                }
            };
            if let Some(result) = result {
                self.worker = None;
                match result {
                    Ok(report) => self.report = Some(report),
                    Err(error) => self.error = Some(error),
                }
            }
        }
        let mut open = self.open;
        let mut open_draft = None;
        if open {
            egui::Window::new("Recover rhythm from WAV")
                .open(&mut open).collapsible(false).default_width(440.0)
                .show(context, |ui| {
                    ui.label("Detect onsets and tempo from a recording, then create an editable rhythm draft.");
                    ui.small("This creates click notes for the rhythm; it does not transcribe pitches or separate instruments.");
                    ui.separator();
                    ui.add_enabled_ui(self.worker.is_none(), |ui| {
                        if ui.button("Choose WAV…").clicked()
                            && let Some(path) = rfd::FileDialog::new().add_filter("WAV audio", &["wav"]).pick_file() {
                            self.source = Some(path);
                            self.report = None;
                            self.error = None;
                        }
                        if let Some(source) = &self.source { ui.label(source.display().to_string()); }
                        ui.checkbox(&mut self.detect_tempo, "Detect tempo automatically");
                        ui.add_enabled(!self.detect_tempo,
                            egui::DragValue::new(&mut self.tempo_bpm).range(1.0..=999.0).suffix(" BPM"));
                        egui::ComboBox::from_label("Quantize grid")
                            .selected_text(format!("1/{}", self.grid))
                            .show_ui(ui, |ui| {
                                for grid in [1, 2, 4, 8, 16, 32] { ui.selectable_value(&mut self.grid, grid, format!("1/{grid}")); }
                            });
                        if ui.add_enabled(self.source.is_some(), egui::Button::new("Create .evil draft…")).clicked()
                            && let Some(output) = rfd::FileDialog::new().add_filter("MIDI Evil project", &["evil"])
                                .set_file_name("recovered-rhythm.evil").save_file() {
                            let source = self.source.clone().unwrap();
                            let tempo = (!self.detect_tempo).then_some(self.tempo_bpm);
                            let grid = self.grid;
                            let (sender, receiver) = mpsc::channel();
                            self.worker = Some(receiver);
                            self.report = None;
                            self.error = None;
                            std::thread::spawn(move || {
                                let result = recover(&source, &output, tempo, grid);
                                let _ = sender.send(result);
                            });
                        }
                    });
                    if self.worker.is_some() { ui.spinner(); ui.label("Analyzing audio and creating the draft…"); }
                    if let Some(error) = &self.error { ui.colored_label(crate::theme::RED, error); }
                    if let Some(report) = &self.report {
                        ui.separator();
                        ui.label(format!("{} notes · {:.1} BPM · 1/{} grid", report.note_count, report.bpm, report.grid));
                        ui.label(&report.output);
                        if ui.button("Open recovered draft").clicked() { open_draft = Some(PathBuf::from(&report.output)); }
                    }
                });
        }
        self.open = open;
        open_draft
    }
}

fn recover(
    source: &std::path::Path,
    output: &std::path::Path,
    tempo: Option<f64>,
    grid: u8,
) -> Result<evil_imports::DraftReport, String> {
    let output = output.with_extension("evil");
    if source
        .canonicalize()
        .ok()
        .zip(output.canonicalize().ok())
        .is_some_and(|(source, output)| source == output)
    {
        return Err(
            "Choose a different output file; the source recording must be preserved".into(),
        );
    }
    evil_imports::create_draft(source, output, tempo, grid).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_cannot_overwrite_its_source_recording() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("recording.evil");
        std::fs::write(&source, b"original recording").unwrap();
        assert!(
            recover(&source, &source, Some(120.0), 16)
                .unwrap_err()
                .contains("preserved")
        );
        assert_eq!(std::fs::read(source).unwrap(), b"original recording");
    }
}
