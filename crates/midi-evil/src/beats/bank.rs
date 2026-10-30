use super::{Action, BeatsPage, kit_peaks};
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
                    self.message = "Bank ready / save banks separately from the song".into();
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
                    self.path = Some(path);
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
                    self.kit.pads[self.selected] = Some(KitPad::new(voice));
                    self.dirty = true;
                    self.message = format!("Loaded WAV into pad {:02}", self.selected + 1);
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
