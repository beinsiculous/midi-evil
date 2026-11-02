use super::*;
use crate::keyboard::{self, NoteEvent};
use crate::live_player::LivePlayer;

impl StudioApp {
    pub(super) fn stop_keyboard(&mut self) {
        if self.keyboard_player.is_some() || self.keyboard_binding.is_some() {
            self.record_note_events(&[NoteEvent::Panic]);
        }
        self.keyboard_player = None;
        self.keyboard.reset();
        self.keyboard_binding = None;
        self.keyboard_error = None;
        self.pending_midi_notes.clear();
    }

    pub(super) fn keyboard_panel(&mut self, ui: &mut egui::Ui) {
        let index = self.roll.selected_track;
        let Some(track) = self.project.tracks.get(index) else {
            return;
        };
        let track_name = track.name.clone();
        let track_muted = track.muted;
        let Ok(instrument) = self.project.instrument_for_track(index).cloned() else {
            return;
        };
        let Some(source) = instrument.patch().map(str::to_owned) else {
            self.keyboard_error = Some("Select a synth instrument for the keyboard".to_owned());
            return;
        };
        let binding = (index, source);
        if self.keyboard_binding.as_ref() != Some(&binding) {
            self.keyboard_player = None;
            self.keyboard.reset();
            let needs_load = self
                .patch_lab
                .as_ref()
                .is_none_or(|lab| lab.track_index != index)
                || self.keyboard_binding.is_some()
                || self.keyboard_error.is_some();
            self.keyboard_error = None;
            if needs_load && !self.open_patch_lab() {
                self.keyboard_error = Some(self.status.clone());
            }
            self.view = View::Keyboard;
            self.keyboard_binding = Some(binding);
        }
        if let Some(error) = &self.keyboard_error {
            ui.colored_label(theme::RED, error);
            if ui.button("Retry instrument").clicked() {
                self.keyboard_binding = None;
            }
            return;
        }
        let Some(lab) = &mut self.patch_lab else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            theme::label(ui, format!("TRACK {:02}  /  {}", index + 1, track_name));
            if ui.button("Edit / save synth patch").clicked() {
                self.view = View::Synth;
            }
            ui.label(
                RichText::new("XY edits share this synth draft")
                    .small()
                    .color(theme::MUTED),
            );
        });
        let mut events = keyboard::show(
            ui,
            &mut lab.patch,
            &mut self.keyboard,
            &self.keyboard_badge,
            instrument.pitch_bend_range_semitones,
        );
        let mut patch = lab.patch.clone();
        self.record_note_events(&events);
        events.append(&mut self.pending_midi_notes);
        patch.pitch_bend_range_semitones = instrument.pitch_bend_range_semitones;
        let controls = evil_synth::LiveControls {
            bend: self.keyboard.bend,
            bend_range_semitones: instrument.pitch_bend_range_semitones,
            gain: if track_muted {
                0.0
            } else {
                instrument.gain * self.project.master_gain
            },
            pan: instrument.pan,
            tempo_bpm: self.project.tempo_bpm,
            filter: patch.filter.clone(),
            delay: patch.delay.clone(),
        };
        if self.keyboard_player.is_none()
            && events
                .iter()
                .any(|event| matches!(event, NoteEvent::On(..)))
        {
            match LivePlayer::new(patch, controls.clone()) {
                Ok(player) => self.keyboard_player = Some(player),
                Err(error) => {
                    self.status = format!("Could not start keyboard audio: {error}");
                    ui.colored_label(theme::RED, &self.status);
                    self.keyboard.reset();
                }
            }
        }
        if let Some(player) = &mut self.keyboard_player {
            player.update(controls, self.keyboard.sustain, events);
        }
    }
}
