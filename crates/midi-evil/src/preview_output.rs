//! What plays a preview. The application is handed one where it is made, as it is handed its
//! dialogs: the desktop's plays through the default audio device, a browser's through the page's
//! own audio, and the library names no device of either.

use std::time::Duration;

use evil_synth::{ChannelMeter, StereoSound};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterSnapshot {
    pub level: [f32; 2],
    pub peak: [f32; 2],
}

/// Plays what a render returned.
pub trait PreviewOutput {
    /// Starts playing `sound` — the range that was asked for, from its first frame — again and
    /// again when `repeats`, with the render's meters for each channel beside it; or says why it
    /// could not start, in words that follow "Could not start audio preview: ".
    fn play(
        &mut self,
        sound: StereoSound,
        channel_meters: Vec<(usize, ChannelMeter)>,
        repeats: bool,
    ) -> Result<Box<dyn PlayingPreview>, String>;
}

/// A preview that has started. Dropping it stops it.
pub trait PlayingPreview {
    fn stop(&self);

    /// Whether a preview that does not repeat has played to its end.
    fn is_finished(&self) -> bool;

    /// How far into the sound it is; a repeat starts again from zero.
    fn position(&self) -> Duration;

    /// Whether the device is taking the sound now. A preview whose device is not running stays
    /// where it is, so a transport that read only its position would say it plays.
    fn device_running(&self) -> bool;

    /// The meters of the whole mix, as they read now; an output that reads none reads silence.
    fn meter(&self) -> MeterSnapshot {
        MeterSnapshot::default()
    }

    /// The meters of each channel, by track; an output that reads none has none.
    fn channel_meters(&self) -> Vec<(usize, MeterSnapshot)> {
        Vec::new()
    }
}
