//! The desktop's MIDI input ports, through `midir`: the names of the ports there are, and a
//! connection to one that hands every message it receives to a callback on midir's thread.

use midir::{Ignore, MidiInput as OsMidiInput, MidiInputConnection};

/// An open input port; dropping it closes the port.
pub type Connection = MidiInputConnection<()>;

pub fn port_names() -> Result<Vec<String>, String> {
    let input = OsMidiInput::new("MIDI Evil input discovery").map_err(|error| error.to_string())?;
    input
        .ports()
        .iter()
        .map(|port| input.port_name(port).map_err(|error| error.to_string()))
        .collect()
}

pub fn connect(
    index: usize,
    mut on_message: impl FnMut(&[u8]) + Send + 'static,
) -> Result<(Connection, String), String> {
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
            move |_timestamp, bytes, _| on_message(bytes),
            (),
        )
        .map_err(|error| error.to_string())?;
    Ok((connection, name))
}
