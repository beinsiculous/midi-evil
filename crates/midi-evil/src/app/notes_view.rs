//! The Notes view: the roll, the note inspector under it and the automation panel, all in the
//! editor region.

use super::*;
use crate::accessibility::Accessible;
use crate::piano_roll::{EditedSequence, edits::RollCommand};
use crate::regions::{self, Region};

impl StudioApp {
    pub(super) fn notes_view(&mut self, ui: &mut egui::Ui) {
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        let Some(track) = self.project.tracks.get(context.track) else {
            return;
        };
        let mut to_arranger = false;
        ui.horizontal(|ui| {
            ui.heading(&track.name);
            if let Some(pattern) = context.pattern {
                ui.label(
                    RichText::new(format!(
                        "/ {} (shared P{:03})",
                        self.project.patterns[pattern].name,
                        pattern + 1
                    ))
                    .color(theme::PURPLE),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::platform::shows(ui.ctx(), crate::platform::Shown::Automation) {
                    ui.toggle_value(&mut self.show_automation, "Automation");
                }
                if ui.button("Arranger").clicked() {
                    to_arranger = true;
                }
            });
        });
        if to_arranger {
            self.select_view(View::Song);
            self.report(format!(
                "The arranger is shown; {} reaches it.",
                crate::key_help::next_region_chord(crate::platform::of(ui.ctx()))
            ));
            return;
        }
        let edited = EditedSequence::new(&self.project, context);
        let Some(sequence) = piano_roll::sequence_mut(&mut self.project, context) else {
            return;
        };
        if piano_roll::show(ui, &edited, sequence, &mut self.roll, &mut self.announcer) {
            self.dirty = true;
            self.status = if context.pattern.is_some() {
                "Shared pattern edited; all linked parts were updated".to_owned()
            } else {
                "Unique timeline notes edited".to_owned()
            };
        }
    }

    pub(super) fn note_inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("note-inspector")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(12, 6)))
            .show(ui, |ui| {
                regions::group(ui, Region::Editor, |ui| self.note_inspector_fields(ui));
            });
    }

    /// The cursor's note, field by field. An edit here is the editor's own, so the cursor stays
    /// on the note it edits even when a twin of it exists.
    fn note_inspector_fields(&mut self, ui: &mut egui::Ui) {
        let context = piano_roll::sequence_context(&self.project, &self.roll, self.document);
        let edited = EditedSequence::new(&self.project, context);
        let Some(sequence) = piano_roll::sequence_mut(&mut self.project, context) else {
            return;
        };
        piano_roll::reconcile(sequence, &mut self.roll, &edited, &mut self.announcer);
        let cursor = self.roll.cursor.index(context);
        let Some((note_index, mut note)) =
            cursor.and_then(|index| sequence.notes().get(index).map(|note| (index, *note)))
        else {
            ui.horizontal(|ui| {
                theme::label(ui, "NOTE INSPECTOR");
                ui.label(
                    RichText::new("Select a note to edit pitch, velocity and timing")
                        .color(theme::MUTED),
                );
            });
            return;
        };
        let original = note;
        let mut remove = false;
        ui.horizontal_wrapped(|ui| {
            theme::label(ui, "NOTE");
            ui.label("Pitch");
            ui.add(egui::DragValue::new(&mut note.pitch).range(0..=127))
                .named("Pitch");
            ui.label("Velocity");
            ui.add(egui::DragValue::new(&mut note.velocity).range(1..=127))
                .named("Velocity");
            ui.label("Start");
            ui.add(
                egui::DragValue::new(&mut note.start_tick).range(
                    0..=edited
                        .grid
                        .pattern_length
                        .unwrap_or(u64::MAX)
                        .saturating_sub(1),
                ),
            )
            .named("Start in ticks");
            ui.label("Length");
            ui.add(egui::DragValue::new(&mut note.duration_ticks).range(1..=1_000_000))
                .named("Length in ticks");
            remove = ui.button("Delete note").clicked();
        });
        if remove {
            piano_roll::run_command(
                sequence,
                &mut self.roll,
                &edited,
                RollCommand::Delete,
                &mut self.announcer,
            );
        } else if note != original {
            sequence.notes_mut()[note_index] = note;
            self.roll
                .cursor
                .set(context, Some(note_index), sequence.notes());
        }
    }

    pub(super) fn automation_panel(&mut self, ui: &mut egui::Ui) {
        let shown = crate::platform::shows(ui.ctx(), crate::platform::Shown::Automation);
        if self.project.tracks.is_empty() || !shown {
            return;
        }
        egui::Panel::bottom("automation")
            .resizable(true)
            .default_size(220.0)
            .min_size(150.0)
            .max_size(360.0)
            .show(ui, |ui| {
                regions::group(ui, Region::Editor, |ui| {
                    if automation::show(ui, &mut self.project, &mut self.automation, &self.roll) {
                        self.dirty = true;
                        let context =
                            piano_roll::sequence_context(&self.project, &self.roll, self.document);
                        if let Some(sequence) = piano_roll::sequence_mut(&mut self.project, context)
                        {
                            self.status = automation::edit_status(sequence, &self.automation);
                        }
                    }
                });
            });
    }
}
