//! MIDI input ports in a build that has none to list: the same two functions as the desktop's,
//! and no port is ever found. Nothing is asked of the browser, because asking for MIDI access on
//! load puts a permission prompt in front of someone who has pressed nothing.

/// Never constructed: there is no port to connect to.
pub enum Connection {}

pub fn port_names() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

pub fn connect(
    _index: usize,
    _on_message: impl FnMut(&[u8]) + Send + 'static,
) -> Result<(Connection, String), String> {
    Err("The selected MIDI input is no longer available".to_owned())
}
