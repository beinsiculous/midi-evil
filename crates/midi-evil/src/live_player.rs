//! Streaming bridge: UI commands are consumed at audio-block boundaries.
use std::num::NonZero;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use evil_synth::{LiveControls, LiveSynth, Patch};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::keyboard::NoteEvent;

enum Command {
    Note(NoteEvent),
    Sustain(bool),
    Controls(LiveControls),
}

pub struct LivePlayer {
    _stream: MixerDeviceSink,
    _player: Player,
    sender: Sender<Command>,
    controls: LiveControls,
    sustain: bool,
}

impl LivePlayer {
    pub fn new(patch: Patch, controls: LiveControls) -> Result<Self, String> {
        let mut synth = LiveSynth::new(patch).map_err(|error| error.to_string())?;
        synth
            .set_controls(controls.clone())
            .map_err(|error| error.to_string())?;
        let mut stream =
            DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string())?;
        stream.log_on_drop(false);
        let player = Player::connect_new(stream.mixer());
        let (sender, receiver) = mpsc::channel();
        player.append(LiveSource {
            synth,
            receiver,
            right: None,
            frame: 0,
        });
        Ok(Self {
            _stream: stream,
            _player: player,
            sender,
            controls,
            sustain: false,
        })
    }

    pub fn update(&mut self, controls: LiveControls, sustain: bool, events: Vec<NoteEvent>) {
        if controls != self.controls {
            let _ = self.sender.send(Command::Controls(controls.clone()));
            self.controls = controls;
        }
        if sustain != self.sustain {
            let _ = self.sender.send(Command::Sustain(sustain));
            self.sustain = sustain;
        }
        for event in events {
            let _ = self.sender.send(Command::Note(event));
        }
    }
}

struct LiveSource {
    synth: LiveSynth,
    receiver: Receiver<Command>,
    right: Option<f32>,
    frame: u64,
}

impl Iterator for LiveSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if self.frame.is_multiple_of(64) {
            // Bound work per audio block even during rapid pointer updates.
            for _ in 0..256 {
                match self.receiver.try_recv() {
                    Ok(Command::Note(NoteEvent::On(pitch, velocity))) => {
                        let _ = self.synth.note_on(pitch, velocity);
                    }
                    Ok(Command::Note(NoteEvent::Off(pitch))) => self.synth.note_off(pitch),
                    Ok(Command::Note(NoteEvent::Panic)) => self.synth.all_notes_off(),
                    Ok(Command::Sustain(sustain)) => self.synth.set_sustain(sustain),
                    Ok(Command::Controls(controls)) => {
                        let _ = self.synth.set_controls(controls);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return None,
                }
            }
        }
        self.frame = self.frame.wrapping_add(1);
        let [left, right] = self.synth.next_frame();
        self.right = Some(right);
        Some(left)
    }
}

impl Source for LiveSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(self.synth.sample_rate()).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_accepts_note_release_and_stops_when_ui_disconnects() {
        let (sender, receiver) = mpsc::channel();
        let mut source = LiveSource {
            synth: LiveSynth::new(Patch::default()).unwrap(),
            receiver,
            right: None,
            frame: 0,
        };
        sender.send(Command::Note(NoteEvent::On(60, 100))).unwrap();
        assert!(source.by_ref().take(2000).any(|sample| sample.abs() > 0.01));
        sender.send(Command::Note(NoteEvent::Panic)).unwrap();
        for _ in 0..128 {
            source.next();
        }
        assert!(source.by_ref().take(1000).all(|sample| sample == 0.0));
        drop(sender);
        assert!(source.take(130).count() < 130);
    }
}
