use std::{
    num::NonZero,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

use evil_beats::{DrumKit, DrumMachine, PreparedPad};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

enum Command {
    Pad(usize, Option<PreparedPad>),
    Input(u16, u8, Option<u8>, f32),
    Gain(f32),
    Stop,
}

#[derive(Clone, Copy)]
struct AudioRepeat {
    slot: usize,
    frame: u64,
    velocity: u8,
}

#[derive(Clone, Copy)]
pub struct RepeatHit {
    pub slot: u8,
    pub velocity: u8,
    pub at: Instant,
}

pub struct BeatsPlayer {
    _stream: MixerDeviceSink,
    _player: Player,
    sender: Sender<Command>,
    repeat_receiver: Receiver<AudioRepeat>,
    clock_origin: Instant,
    input: Option<(u16, u8, Option<u8>, f32)>,
    gain: f32,
}

impl BeatsPlayer {
    pub fn new(kit: &DrumKit) -> Result<Self, String> {
        kit.validate().map_err(|e| e.to_string())?;
        let mut machine = DrumMachine::default();
        for (slot, pad) in kit.pads.iter().enumerate() {
            let prepared = pad
                .as_ref()
                .map(PreparedPad::new)
                .transpose()
                .map_err(|e| e.to_string())?;
            machine.set_pad(slot, prepared);
        }
        machine.set_gain(kit.gain);
        let mut stream = DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
        stream.log_on_drop(false);
        let player = Player::connect_new(stream.mixer());
        let (sender, receiver) = mpsc::channel();
        let (repeat_sender, repeat_receiver) = mpsc::channel();
        let clock_origin = Instant::now();
        player.append(BeatsSource {
            machine,
            receiver,
            repeat_sender,
            right: None,
            frame: 0,
        });
        Ok(Self {
            _stream: stream,
            _player: player,
            sender,
            repeat_receiver,
            clock_origin,
            input: None,
            gain: kit.gain,
        })
    }

    pub fn update_pad(&self, kit: &DrumKit, slot: usize) -> Result<(), String> {
        let prepared = kit.pads[slot]
            .as_ref()
            .map(PreparedPad::new)
            .transpose()
            .map_err(|e| e.to_string())?;
        self.sender
            .send(Command::Pad(slot, prepared))
            .map_err(|e| e.to_string())
    }

    pub fn input(&mut self, held: u16, velocity: u8, repeat: Option<u8>, tempo: f32, gain: f32) {
        let input = (held, velocity, repeat, tempo);
        if self.input != Some(input) {
            let _ = self
                .sender
                .send(Command::Input(held, velocity, repeat, tempo));
            self.input = Some(input);
        }
        if self.gain != gain {
            let _ = self.sender.send(Command::Gain(gain));
            self.gain = gain;
        }
    }

    pub fn repeated_hits(&self) -> Vec<RepeatHit> {
        self.repeat_receiver
            .try_iter()
            .map(|event| RepeatHit {
                slot: event.slot as u8,
                velocity: event.velocity,
                at: self.clock_origin
                    + Duration::from_secs_f64(
                        event.frame as f64 / f64::from(DrumMachine::SAMPLE_RATE),
                    ),
            })
            .collect()
    }

    pub fn stop(&mut self) {
        let _ = self.sender.send(Command::Stop);
        self.input = None;
    }
}

struct BeatsSource {
    machine: DrumMachine,
    receiver: Receiver<Command>,
    repeat_sender: Sender<AudioRepeat>,
    right: Option<f32>,
    frame: u64,
}

impl Iterator for BeatsSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if self.frame.is_multiple_of(64) {
            for _ in 0..256 {
                match self.receiver.try_recv() {
                    Ok(Command::Pad(slot, pad)) => self.machine.set_pad(slot, pad),
                    Ok(Command::Input(held, velocity, repeat, tempo)) => {
                        let _ = self.machine.set_input(held, velocity, repeat, tempo);
                    }
                    Ok(Command::Gain(gain)) => self.machine.set_gain(gain),
                    Ok(Command::Stop) => self.machine.stop(),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return None,
                }
            }
        }
        let ([left, right], repeated) = self.machine.next_frame_with_repeats();
        for slot in 0..evil_beats::PAD_COUNT {
            if repeated & (1 << slot) != 0 {
                let _ = self.repeat_sender.send(AudioRepeat {
                    slot,
                    frame: self.frame,
                    velocity: self.machine.input_velocity(),
                });
            }
        }
        self.frame = self.frame.wrapping_add(1);
        self.right = Some(right);
        Some(left)
    }
}

impl Source for BeatsSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(DrumMachine::SAMPLE_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
