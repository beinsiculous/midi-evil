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

    /// Bind both keyboard surfaces to the selected track without discarding its draft.
    pub(super) fn prepare_keyboard(&mut self, ui: &mut egui::Ui) -> bool {
        let index = self.roll.selected_track;
        let source = self
            .project
            .instrument_for_track(index)
            .ok()
            .and_then(|instrument| instrument.patch())
            .map(str::to_owned);
        let Some(source) = source else {
            self.stop_keyboard();
            ui.label("Select a synth instrument to edit and play it.");
            return false;
        };
        let binding = (index, source);
        if self.keyboard_binding.as_ref() != Some(&binding) {
            if self.keyboard_binding.is_some() {
                self.record_note_events(&[NoteEvent::Panic]);
            }
            let source_changed = self
                .keyboard_binding
                .as_ref()
                .is_some_and(|old| old != &binding);
            self.keyboard_player = None;
            self.keyboard.reset();
            let retrying = self.keyboard_error.take().is_some();
            if source_changed
                || retrying
                || self
                    .patch_lab
                    .as_ref()
                    .is_none_or(|lab| lab.track_index != index)
            {
                let view = self.view;
                if !self.open_patch_lab() {
                    self.keyboard_error = Some(self.status.clone());
                }
                self.view = view;
            }
            self.keyboard_binding = Some(binding);
        }
        if let Some(error) = &self.keyboard_error {
            ui.colored_label(theme::RED, error);
            if ui.button("Retry instrument").clicked() {
                self.keyboard_binding = None;
            }
            return false;
        }
        self.patch_lab.is_some()
    }

    pub(super) fn keyboard_panel(&mut self, ui: &mut egui::Ui) {
        if !self.prepare_keyboard(ui) {
            return;
        }
        let index = self.roll.selected_track;
        let track_name = self.project.tracks[index].name.clone();
        let Some(mut lab) = self.patch_lab.take() else {
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
        let bend_range = self
            .project
            .instrument_for_track(index)
            .unwrap()
            .pitch_bend_range_semitones;
        let events = keyboard::show(
            ui,
            &mut lab.patch,
            &mut self.keyboard,
            &self.keyboard_badge,
            bend_range,
        );
        self.play_keyboard_events(ui, &lab.patch, events);
        self.patch_lab = Some(lab);
    }

    pub(super) fn play_keyboard_events(
        &mut self,
        ui: &mut egui::Ui,
        patch: &evil_synth::Patch,
        mut events: Vec<NoteEvent>,
    ) {
        self.record_note_events(&events);
        events.append(&mut self.pending_midi_notes);
        let index = self.roll.selected_track;
        let Ok(instrument) = self.project.instrument_for_track(index) else {
            return;
        };
        let controls = evil_synth::LiveControls {
            bend: self.keyboard.bend,
            bend_range_semitones: instrument.pitch_bend_range_semitones,
            gain: if self.project.tracks[index].muted {
                0.0
            } else {
                instrument.gain * self.project.master_gain
            },
            pan: instrument.pan,
            tempo_bpm: self.project.tempo_bpm,
            filter: patch.filter.clone(),
            waveshaper: patch.waveshaper.clone(),
            delay: patch.delay.clone(),
        };
        if self
            .keyboard_player
            .as_ref()
            .is_some_and(|player| player.sample_rate() != patch.sample_rate)
        {
            self.record_note_events(&[NoteEvent::Panic]);
            self.keyboard_player = None;
            self.keyboard.reset();
        }
        if self.keyboard_player.is_none()
            && events
                .iter()
                .any(|event| matches!(event, NoteEvent::On(..)))
        {
            match LivePlayer::new(patch.clone(), controls.clone()) {
                Ok(player) => self.keyboard_player = Some(player),
                Err(error) => {
                    self.status = format!("Could not start keyboard audio: {error}");
                    self.record_note_events(&[NoteEvent::Panic]);
                    self.keyboard.reset();
                }
            }
        }
        if let Some(player) = &mut self.keyboard_player
            && let Err(error) = player.update(patch, controls, self.keyboard.sustain, events)
        {
            self.status = format!("Could not update keyboard audio: {error}");
            ui.colored_label(theme::RED, &self.status);
            // Stop on invalid edits so releases cannot be lost behind a validation error.
            self.record_note_events(&[NoteEvent::Panic]);
            self.keyboard_player = None;
            self.keyboard.reset();
        }
    }
}
