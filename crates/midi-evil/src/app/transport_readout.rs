//! The toolbar's transport block — play, record, the project's state and name, tempo, meter and
//! the clock — and the transport readout in it: one focusable status, present at every window
//! width, that says what the transport is doing. Its name is true whenever it is read and still
//! between reads; the rule for that is the announcer's.

use super::*;
use crate::accessibility::Accessible;
use crate::regions::{self, Region};
use crate::speech::{self, Meter, PlayingState, TransportState};

impl StudioApp {
    pub(super) fn transport_block(&mut self, ui: &mut egui::Ui) {
        let active = self.render_job.is_some() || self.preview.is_some();
        let label = if self.render_job.is_some() {
            "CANCEL"
        } else if active {
            "STOP"
        } else {
            "PLAY"
        };
        if ui
            .add_sized(
                [88.0, 40.0],
                egui::Button::image_and_text(
                    art::control(&self.controls, if active { 5 } else { 4 }),
                    label,
                ),
            )
            .on_hover_text("Render and play the song · Space to play/stop")
            .clicked()
        {
            if active {
                self.stop_preview();
            } else {
                self.preview_project();
            }
        }
        if ui
            .add_sized(
                [88.0, 40.0],
                egui::Button::new(if self.recorder.is_some() {
                    "Stop take"
                } else {
                    "Record"
                })
                .selected(self.recorder.is_some()),
            )
            .on_hover_text("Record keyboard, pads or MIDI into the selected track/part")
            .clicked()
        {
            self.toggle_recording();
        }
        egui::Frame::new()
            .fill(theme::BACKGROUND)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(3)
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        self.transport_readout(ui, active);
                        ui.label(
                            RichText::new(if self.dirty {
                                "• MODIFIED"
                            } else {
                                "• SAVED"
                            })
                            .size(11.0)
                            .color(theme::MUTED),
                        );
                    });
                    let width = (ui.ctx().content_rect().width() - 700.0).clamp(100.0, 280.0);
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.project.name)
                                .desired_width(width)
                                .font(egui::FontId::monospace(16.0))
                                .text_color(theme::IVORY)
                                .frame(egui::Frame::NONE),
                        )
                        .named("Project name")
                        .changed()
                    {
                        self.dirty = true;
                    }
                });
            });
        ui.vertical(|ui| {
            theme::label(ui, "TEMPO");
            if ui
                .add(
                    egui::DragValue::new(&mut self.project.tempo_bpm)
                        .range(1.0..=999.0)
                        .speed(0.1)
                        .suffix(" BPM"),
                )
                .named("Tempo in beats per minute")
                .changed()
            {
                self.dirty = true;
            }
        });
        ui.vertical(|ui| {
            theme::label(ui, "METER");
            if ui
                .add(
                    egui::DragValue::new(&mut self.project.beats_per_bar)
                        .range(1..=32)
                        .suffix(format!(" / {}", self.project.beat_unit)),
                )
                .named(format!(
                    "Meter, beats per bar over {}",
                    self.project.beat_unit
                ))
                .changed()
            {
                self.dirty = true;
            }
        });
        if ui.ctx().content_rect().width() >= 1_160.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    let position = self.transport_state().position_milliseconds;
                    ui.label(
                        RichText::new(format!(
                            "{:02}:{:02}.{:02}",
                            position / 60_000,
                            position / 1_000 % 60,
                            position / 10 % 100
                        ))
                        .monospace()
                        .size(22.0)
                        .color(theme::IVORY),
                    );
                    theme::label(ui, format!("STEREO  /  {} Hz", self.project.sample_rate));
                });
            });
        }
    }

    /// The readout: what is shown keeps counting; what a screen reader is given is the
    /// announcer's on-demand name, with fixed help as its description. Enter, a click or the
    /// accessibility tree's click reads it again; Space is left to the transport.
    fn transport_readout(&mut self, ui: &mut egui::Ui, active: bool) {
        let state = self.transport_state();
        let (bar, beat) = speech::bar_and_beat(state.position_tick, state.meter);
        let shown = format!(
            "{}  ·  BAR {bar} BEAT {beat}",
            if state.playing == PlayingState::Stopped {
                "READY".to_owned()
            } else {
                state.playing.word().to_uppercase()
            }
        );
        let galley = ui.painter().layout_no_wrap(
            shown,
            egui::FontId::proportional(11.0),
            if active { theme::GREEN } else { theme::MUTED },
        );
        let (rect, _) =
            ui.allocate_exact_size(galley.size() + egui::vec2(6.0, 4.0), egui::Sense::hover());
        let response = ui.interact(rect, readout_id(), egui::Sense::click());
        if response.clicked_by(egui::PointerButton::Primary) {
            response.request_focus();
        }
        ui.painter()
            .galley(rect.min + egui::vec2(3.0, 2.0), galley, theme::MUTED);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.5, theme::IVORY),
                egui::StrokeKind::Inside,
            );
        }
        crate::custom_editors::register(ui.ctx(), response.id);
        regions::prefer_stop(ui.ctx(), Region::Toolbar, response.id);
        let name = self.announcer.on_demand_name(
            response.id,
            response.has_focus(),
            speech::describe_transport(&state),
            response.clicked(),
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Status);
            node.set_label(name);
            node.set_description(speech::TRANSPORT_READOUT_HELP);
        });
        response.on_hover_text(speech::TRANSPORT_READOUT_HELP);
    }

    /// What the transport is doing now, made from live state each time it is asked for.
    pub(super) fn transport_state(&self) -> TransportState {
        let playing = if self.recorder.is_some() {
            PlayingState::Recording
        } else if self.render_job.is_some() {
            PlayingState::Rendering
        } else if self.preview.is_some() {
            PlayingState::Playing
        } else {
            PlayingState::Stopped
        };
        TransportState {
            playing,
            tempo_beats_per_minute: self.project.tempo_bpm,
            meter: Meter::of(&self.project),
            position_tick: self.song_playhead_tick(),
            position_milliseconds: self.playing_time().as_millis(),
        }
    }

    /// How long the song has been playing, by the clock the playhead follows: a take's own
    /// clock while one runs — its preview may have no device, or start late after its render —
    /// else the preview's.
    fn playing_time(&self) -> std::time::Duration {
        match (self.recording_started_at, &self.preview) {
            (Some(started_at), _) => started_at.elapsed(),
            (None, Some(preview)) => preview.position(),
            (None, None) => std::time::Duration::ZERO,
        }
    }

    pub(super) fn song_playhead_tick(&self) -> u64 {
        if self.recording_started_at.is_some() {
            return self.recording_tick();
        }
        if let Some(preview) = &self.preview {
            return self.tick_at_elapsed(preview.position().as_secs_f64());
        }
        self.transport.start_tick
    }

    pub(super) fn recording_tick(&self) -> u64 {
        self.recording_tick_offset(0.0)
    }

    pub(super) fn recording_tick_offset(&self, offset_seconds: f64) -> u64 {
        let Some(started_at) = self.recording_started_at else {
            return self.transport.start_tick;
        };
        self.tick_at_elapsed(started_at.elapsed().as_secs_f64() + offset_seconds)
    }

    fn tick_at_elapsed(&self, elapsed: f64) -> u64 {
        let (_, end_tick) = self.transport.range();
        crate::recording::recording_tick_at_elapsed(
            &self.project,
            self.transport.start_tick,
            end_tick,
            elapsed,
        )
        .unwrap_or(self.transport.start_tick)
    }
}

/// The readout's widget, the same whatever the toolbar around it holds, so focus stays on it
/// through a new or opened project.
pub(super) fn readout_id() -> egui::Id {
    egui::Id::new("transport-readout")
}

#[cfg(test)]
mod tests;
