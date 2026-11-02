use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;
use midir::{Ignore, MidiInput as OsMidiInput, MidiInputConnection};

use crate::recording::InputEvent;

/// Desktop MIDI input is parsed on the callback thread and consumed by the
/// same performance/recording event path as the on-screen keyboard and pads.
pub struct MidiInput {
    ports: Vec<String>,
    selected: Option<usize>,
    connection: Option<MidiInputConnection<()>>,
    sender: Sender<InputEvent>,
    receiver: Receiver<InputEvent>,
    message: String,
}

impl Default for MidiInput {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut input = Self {
            ports: Vec::new(),
            selected: None,
            connection: None,
            sender,
            receiver,
            message: "No MIDI input connected".to_owned(),
        };
        input.refresh();
        input
    }
}

impl MidiInput {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let previous = self.selected;
        egui::ComboBox::from_id_salt("midi-input-port")
            .selected_text(
                self.selected
                    .and_then(|index| self.ports.get(index))
                    .map_or("MIDI input: off", String::as_str),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.selected, None, "Off");
                for (index, name) in self.ports.iter().enumerate() {
                    ui.selectable_value(&mut self.selected, Some(index), name);
                }
            });
        if self.selected != previous {
            if let Some(index) = self.selected {
                self.connect(index);
            } else {
                self.disconnect();
            }
        }
        if ui
            .button("↻")
            .on_hover_text("Refresh MIDI inputs")
            .clicked()
        {
            self.refresh();
        }
        ui.label(egui::RichText::new(&self.message).small())
            .on_hover_text(&self.message);
    }

    pub fn poll(&self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn disconnect(&mut self) {
        self.connection = None;
        self.selected = None;
        self.message = "No MIDI input connected".to_owned();
    }

    fn refresh(&mut self) {
        match port_names() {
            Ok(ports) => {
                self.ports = ports;
                if self
                    .selected
                    .is_some_and(|selected| selected >= self.ports.len())
                {
                    self.disconnect();
                }
            }
            Err(error) => self.message = error,
        }
    }

    fn connect(&mut self, index: usize) {
        self.disconnect();
        match connect(index, self.sender.clone()) {
            Ok((connection, name)) => {
                self.connection = Some(connection);
                self.selected = Some(index);
                self.message = format!("Recording MIDI from {name}");
            }
            Err(error) => self.message = error,
        }
    }
}

fn port_names() -> Result<Vec<String>, String> {
    let input = OsMidiInput::new("MIDI Evil input discovery").map_err(|error| error.to_string())?;
    input
        .ports()
        .iter()
        .map(|port| input.port_name(port).map_err(|error| error.to_string()))
        .collect()
}

fn connect(
    index: usize,
    sender: Sender<InputEvent>,
) -> Result<(MidiInputConnection<()>, String), String> {
    let mut input = OsMidiInput::new("MIDI Evil recording").map_err(|error| error.to_string())?;
    input.ignore(Ignore::None);
    let ports = input.ports();
    let port = ports
        .get(index)
        .ok_or_else(|| "The selected MIDI input is no longer available".to_owned())?;
    let name = input.port_name(port).map_err(|error| error.to_string())?;
    let connection = input
        .connect(
            port,
            "midi-evil-input",
            move |_timestamp, bytes, _| {
                if let Some(event) = parse_message(bytes) {
                    let _ = sender.send(event);
                }
            },
            (),
        )
        .map_err(|error| error.to_string())?;
    Ok((connection, name))
}

fn parse_message(bytes: &[u8]) -> Option<InputEvent> {
    if bytes.len() < 2 {
        return None;
    }
    let data = |index: usize| bytes.get(index).copied().unwrap_or(0) & 0x7f;
    match bytes[0] & 0xf0 {
        0x80 => Some(InputEvent::NoteOff { pitch: data(1) }),
        0x90 => Some(InputEvent::NoteOn {
            pitch: data(1),
            velocity: data(2),
        }),
        0xa0 => Some(InputEvent::Aftertouch {
            note: Some(data(1)),
            value: data(2),
        }),
        0xb0 => Some(InputEvent::Controller {
            controller: data(1),
            value: data(2),
        }),
        0xd0 => Some(InputEvent::Aftertouch {
            note: None,
            value: data(1),
        }),
        0xe0 => {
            let value = i16::from(data(1)) | (i16::from(data(2)) << 7);
            Some(InputEvent::PitchBend {
                value: value - 8192,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notes_controllers_pressure_and_signed_bend() {
        assert_eq!(
            parse_message(&[0x90, 60, 100]),
            Some(InputEvent::NoteOn {
                pitch: 60,
                velocity: 100
            })
        );
        assert_eq!(
            parse_message(&[0x90, 60, 0]),
            Some(InputEvent::NoteOn {
                pitch: 60,
                velocity: 0
            })
        );
        assert_eq!(
            parse_message(&[0xb0, 1, 90]),
            Some(InputEvent::Controller {
                controller: 1,
                value: 90
            })
        );
        assert_eq!(
            parse_message(&[0xd0, 70]),
            Some(InputEvent::Aftertouch {
                note: None,
                value: 70
            })
        );
        assert_eq!(
            parse_message(&[0xe0, 0, 0]),
            Some(InputEvent::PitchBend { value: -8192 })
        );
        assert_eq!(parse_message(&[0xf8]), None);
    }
}
