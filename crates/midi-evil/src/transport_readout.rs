//! The transport as one focusable readout: state, tempo and position, read when it takes focus
//! or is activated, never announced on repaint. It is a function of a plain state struct so the
//! web build can include this file and fill the struct from its own harness.

use eframe::egui;

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayingState {
    Stopped,
    Rendering,
    Playing,
    Recording,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportState {
    pub playing: PlayingState,
    pub tempo_beats_per_minute: f32,
    pub position_milliseconds: u128,
}

/// "Transport, playing, tempo 120 beats per minute, position 1 minute 5 seconds".
pub fn describe(state: &TransportState) -> String {
    let playing = match state.playing {
        PlayingState::Stopped => "stopped",
        PlayingState::Rendering => "rendering",
        PlayingState::Playing => "playing",
        PlayingState::Recording => "recording",
    };
    let tempo = format!("{:.1}", state.tempo_beats_per_minute);
    let tempo = tempo.trim_end_matches('0').trim_end_matches('.');
    let total_seconds = state.position_milliseconds / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!(
        "Transport, {playing}, tempo {tempo} beats per minute, position {minutes} {}, {seconds} {}",
        if minutes == 1 { "minute" } else { "minutes" },
        if seconds == 1 { "second" } else { "seconds" },
    )
}

/// The readout's spoken text, kept between frames: it changes only when the readout takes focus
/// or is activated, because a focused node whose name changes is spoken on every change.
#[derive(Debug, Default)]
pub struct TransportReadout {
    spoken: String,
}

impl TransportReadout {
    pub fn show(&mut self, ui: &mut egui::Ui, state: &TransportState) -> egui::Response {
        let (visible, color) = match state.playing {
            PlayingState::Stopped => ("READY", theme::MUTED),
            PlayingState::Rendering => ("RENDERING", theme::GREEN),
            PlayingState::Playing => ("PLAYING", theme::GREEN),
            PlayingState::Recording => ("RECORDING", theme::GREEN),
        };
        let response = ui.add(
            egui::Label::new(egui::RichText::new(visible).size(11.0).color(color))
                .selectable(false)
                .sense(egui::Sense::click()),
        );
        if self.spoken.is_empty() || response.gained_focus() || response.clicked() {
            self.spoken = describe(state);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.expand(2.0),
                2.0,
                egui::Stroke::new(1.5, theme::IVORY),
                egui::StrokeKind::Outside,
            );
        }
        let spoken = self.spoken.clone();
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Status);
            node.set_label(spoken);
        });
        response.on_hover_text("Transport · focus or press Enter to read state, tempo and position")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readout_gives_state_tempo_and_position() {
        let state = TransportState {
            playing: PlayingState::Playing,
            tempo_beats_per_minute: 92.5,
            position_milliseconds: 65_400,
        };
        assert_eq!(
            describe(&state),
            "Transport, playing, tempo 92.5 beats per minute, position 1 minute, 5 seconds"
        );
        let stopped = TransportState {
            playing: PlayingState::Stopped,
            tempo_beats_per_minute: 120.0,
            position_milliseconds: 0,
        };
        assert_eq!(
            describe(&stopped),
            "Transport, stopped, tempo 120 beats per minute, position 0 minutes, 0 seconds"
        );
    }
}
