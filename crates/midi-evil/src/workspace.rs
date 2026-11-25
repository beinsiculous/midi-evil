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
