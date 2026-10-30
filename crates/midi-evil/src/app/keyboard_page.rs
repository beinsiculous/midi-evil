use super::*;
use crate::keyboard::{self, NoteEvent};
use crate::live_player::LivePlayer;

impl StudioApp {
    pub(super) fn stop_keyboard(&mut self) {
        self.keyboard_player = None;
        self.keyboard.reset();
        self.keyboard_binding = None;
        self.keyboard_error = None;
    }

    pub(super) fn keyboard_panel(&mut self, ui: &mut egui::Ui) {
        let index = self.roll.selected_track;
        let Some(track) = self.project.tracks.get(index) else {
            return;
        };
        let binding = (index, track.patch.clone());
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
        let track = &self.project.tracks[index];
        ui.horizontal_wrapped(|ui| {
            theme::label(ui, format!("TRACK {:02}  /  {}", index + 1, track.name));
            if ui.button("Edit / save synth patch").clicked() {
                self.view = View::Synth;
            }
            ui.label(
                RichText::new("XY edits share this synth draft")
                    .small()
                    .color(theme::MUTED),
            );
        });
        let events = keyboard::show(
            ui,
            &mut lab.patch,
            &mut self.keyboard,
            &self.keyboard_badge,
            track.pitch_bend_range_semitones,
        );
        let mut patch = lab.patch.clone();
        patch.pitch_bend_range_semitones = track.pitch_bend_range_semitones;
        let controls = evil_synth::LiveControls {
            bend: self.keyboard.bend,
            bend_range_semitones: track.pitch_bend_range_semitones,
            gain: if track.muted {
                0.0
            } else {
                track.gain * self.project.master_gain
            },
            pan: track.pan,
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
