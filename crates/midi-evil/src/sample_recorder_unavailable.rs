//! The microphone recorder in a build that has no microphone to open: the same two functions
//! as the desktop's, and `start` says why nothing can be recorded.

use evil_synth::SampleOscillator;

/// Never constructed: `start` always refuses, so there is no recording to finish.
pub enum SampleRecorder {}

impl SampleRecorder {
    pub fn start() -> Result<Self, String> {
        Err("Recording from a microphone is not available in the browser.".to_owned())
    }

    pub fn finish(self) -> Result<SampleOscillator, String> {
        match self {}
    }
}
