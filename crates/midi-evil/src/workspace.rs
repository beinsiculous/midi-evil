use eframe::egui;
use evil_project::Project;

use crate::transport::Transport;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Song,
    Notes,
    Mixer,
    Synth,
    Keyboard,
    Pads,
}

impl View {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::Song, "SONG"),
        (Self::Notes, "PIANO ROLL"),
        (Self::Mixer, "MIXER"),
        (Self::Synth, "SYNTH"),
        (Self::Keyboard, "KEYBOARD"),
        (Self::Pads, "PADS"),
    ];
}

pub use crate::arranger::SongResult;

/// Arrange shared patterns and unique performances on the song timeline.
pub fn song(
    ui: &mut egui::Ui,
    project: &mut Project,
    roll: &mut crate::piano_roll::RollState,
    transport: &mut Transport,
    playhead_tick: u64,
    allow_structural_edits: bool,
) -> SongResult {
    crate::arranger::show(
        ui,
        project,
        roll,
        transport,
        playhead_tick,
        allow_structural_edits,
    )
}
