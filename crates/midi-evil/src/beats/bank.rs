use super::{Action, BeatsPage, SampleUndo, kit_peaks};
use evil_beats::{DrumHit, DrumKit, DrumPad, KitPad};

impl BeatsPage {
    pub(super) fn action(&mut self, action: Action) {
        self.stop();
        let result: Result<(), String> = (|| {
            match action {
                Action::New | Action::Starter | Action::Open => {
                    if self.dirty
                        && rfd::MessageDialog::new()
                            .set_title("Unsaved drum bank")
                            .set_description("Replace this bank and discard its unsaved changes?")
                            .set_buttons(rfd::MessageButtons::YesNo)
                            .show()
                            != rfd::MessageDialogResult::Yes
                    {
                        return Ok(());
                    }
                    let (kit, path) = match action {
                        Action::New => (DrumKit::empty("Untitled Kit"), None),
                        Action::Starter => {
                            (evil_beats::maxwell_kit().map_err(|e| e.to_string())?, None)
                        }
                        _ => {
                            let Some(path) = rfd::FileDialog::new()
                                .add_filter("Evil drum bank", &["evilkit"])
                                .pick_file()
                            else {
                                return Ok(());
                            };
                            (DrumKit::read(&path).map_err(|e| e.to_string())?, Some(path))
                        }
                    };
                    self.kit = kit;
                    self.path = path;
                    self.dirty = false;
                    self.selected = 0;
                    self.sample_undo = None;
                    self.message = "Bank ready / Save bank to update its linked instrument".into();
                }
                Action::Save => {
                    let filename = self
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "maxwell-kit.evilkit".into());
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("Evil drum bank", &["evilkit"])
                        .set_file_name(filename)
                        .save_file()
                    else {
                        return Ok(());
                    };
                    let json = self.kit.to_json().map_err(|e| e.to_string())?;
                    evil_project::atomic_write(&path, json.as_bytes())
                        .map_err(|e| e.to_string())?;
                    self.message = format!("Saved {}", path.display());
                    self.path = Some(path.clone());
                    self.saved_path = Some(path);
                    self.dirty = false;
                }
                Action::Import => {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("WAV audio", &["wav"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    let voice = DrumPad::read_wav(&path).map_err(|e| e.to_string())?;
                    self.sample_undo = None;
                    self.kit.pads[self.selected] = Some(KitPad::new(voice));
                    self.dirty = true;
                    self.message = format!("Loaded WAV into pad {:02}", self.selected + 1);
                }
                Action::Crop | Action::Normalize => {
                    let Some(pad) = &mut self.kit.pads[self.selected] else {
                        return Ok(());
                    };
                    let label = if action == Action::Crop {
                        "crop"
                    } else {
                        "normalization"
                    };
                    let undo = SampleUndo::capture(self.selected, &pad.voice, label);
                    let changed = if action == Action::Crop {
                        pad.voice.crop_to_playback_range()
                    } else {
                        pad.voice.normalize()
                    }
                    .map_err(|error| error.to_string())?;
                    if !changed {
                        self.message = if action == Action::Crop {
                            "Pad already uses the complete source".into()
                        } else {
                            "Pad is silent or already normalized".into()
                        };
                        return Ok(());
                    }
                    self.sample_undo = Some(undo);
                    self.dirty = true;
                    self.message = format!(
                        "{} complete on pad {:02} / one-step undo available",
                        if action == Action::Crop {
                            "Crop"
                        } else {
                            "Normalize"
                        },
                        self.selected + 1
                    );
                }
                Action::ApplyEffect => {
                    let Some(pad) = &mut self.kit.pads[self.selected] else {
                        return Ok(());
                    };
                    let undo = SampleUndo::capture(self.selected, &pad.voice, "effect");
                    if !pad
                        .voice
                        .apply_effect(&self.sample_effect, self.tempo_bpm)
                        .map_err(|error| error.to_string())?
                    {
                        self.message = "The selected effect leaves the sample unchanged".into();
                        return Ok(());
                    }
                    self.sample_undo = Some(undo);
                    self.dirty = true;
                    self.message = format!(
                        "Effect rendered into pad {:02} / one-step undo available",
                        self.selected + 1
                    );
                }
                Action::UndoSample => {
                    let Some(undo) = self.sample_undo.take() else {
                        self.message = "No sample edit to undo".into();
                        return Ok(());
                    };
                    let slot = undo.slot;
                    let label = undo.label;
                    let Some(pad) = &mut self.kit.pads[slot] else {
                        self.message = "The edited pad is no longer available".into();
                        return Ok(());
                    };
                    undo.restore(&mut pad.voice);
                    self.selected = slot;
                    self.dirty = true;
                    self.message = format!("Undid {label} on pad {:02}", slot + 1);
                }
                Action::Export => {
                    let Some(pad) = &self.kit.pads[self.selected] else {
                        return Ok(());
                    };
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("WAV audio", &["wav"])
                        .set_file_name("pad.wav")
                        .save_file()
                    else {
                        return Ok(());
                    };
                    pad.voice
                        .render(&[DrumHit {
                            start_seconds: 0.0,
                            duration_seconds: pad.gate_seconds,
                            velocity: 127,
                        }])
                        .map_err(|e| e.to_string())?
                        .write_wav(&path)
                        .map_err(|e| e.to_string())?;
                    self.message = format!("Saved {}", path.display());
                }
                Action::Clear => {
                    self.sample_undo = None;
                    self.kit.pads[self.selected] = None;
                    self.dirty = true;
                    self.message = format!("Pad {:02} cleared", self.selected + 1);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.message = error;
        }
        self.peaks = kit_peaks(&self.kit);
    }
}
