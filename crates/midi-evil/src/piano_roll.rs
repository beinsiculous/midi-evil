//! The piano roll: one focusable widget with a note cursor, not a node per note, because the
//! roll paints only what the viewport shows and per-note nodes would make the rest unreachable.
//! Its accessible name carries the cursor's note; its description carries the key help.

pub mod cursor;
mod drawing;
pub mod edits;
mod toolbar;

pub(crate) use drawing::snap_tick;

use eframe::egui::{self, Rect, Stroke, Vec2, accesskit};
use evil_project::{EventSequence, Note, Project};

use self::{
    cursor::{CursorKeeper, Reconciled, SequenceContext},
    drawing::{KEY_HEIGHT, KEYBOARD_WIDTH, RULER_HEIGHT},
    edits::{EditGrid, Edited, MAX_PITCH, Outcome, RollCommand},
};
use crate::{
    announcer::Announcer,
    regions::{self, Region},
    speech::{self, Meter, Origin},
    theme,
};

#[derive(Debug)]
pub struct RollState {
    pub selected_track: usize,
    pub selected_part: Option<usize>,
    pub cursor: CursorKeeper,
    clipboard: Vec<Note>,
    dragging_note: Option<usize>,
    resizing_note: Option<usize>,
    zoom: f32,
    scroll_to_middle: bool,
    scroll_to_cursor: bool,
    /// The roll's widget, once it has been shown: the key dispatch and F6 address it.
    pub widget_id: Option<egui::Id>,
    /// Whether the roll had focus when the last frame ended.
    pub had_focus: bool,
    /// The arranger opened something here and focus follows it, the next time the roll is drawn.
    pub focus_when_shown: bool,
}

impl Default for RollState {
    fn default() -> Self {
        Self {
            selected_track: 0,
            selected_part: None,
            cursor: CursorKeeper::default(),
            clipboard: Vec::new(),
            dragging_note: None,
            resizing_note: None,
            zoom: 1.0,
            scroll_to_middle: true,
            scroll_to_cursor: false,
            widget_id: None,
            had_focus: false,
            focus_when_shown: false,
        }
    }
}

impl RollState {
    pub fn select_track(&mut self, index: usize) {
        self.selected_track = index;
        self.selected_part = None;
        self.cursor.clear();
        self.dragging_note = None;
        self.resizing_note = None;
        self.scroll_to_middle = true;
    }

    pub fn select_part(&mut self, track: usize, part: Option<usize>) {
        if self.selected_track != track || self.selected_part != part {
            self.cursor.clear();
            self.dragging_note = None;
            self.resizing_note = None;
            self.scroll_to_middle = true;
        }
        self.selected_track = track;
        self.selected_part = part;
    }

    /// Puts back a selection that undo stored with a project, keeping it inside that project.
    pub fn restore(
        &mut self,
        project: &Project,
        track: usize,
        part: Option<usize>,
        document: u64,
        cursor: Option<usize>,
    ) {
        self.dragging_note = None;
        self.resizing_note = None;
        if project.tracks.is_empty() {
            self.selected_track = 0;
            self.selected_part = None;
            self.cursor.clear();
            return;
        }
        // One scroll: to the restored cursor's note, or to the middle of a newly shown sequence;
        // a cursor cancels a middle scroll something earlier in the frame asked for.
        if cursor.is_some() {
            self.scroll_to_middle = false;
        } else if track != self.selected_track || part != self.selected_part {
            self.scroll_to_middle = true;
        }
        self.selected_track = track.min(project.tracks.len() - 1);
        self.selected_part =
            part.filter(|part| *part < project.tracks[self.selected_track].parts.len());
        let context = sequence_context(project, self, document);
        let notes = sequence_notes(project, context);
        self.cursor.set(context, cursor, notes);
        self.scroll_to_cursor = cursor.is_some();
    }

    pub fn pixels_per_tick(&self, ticks_per_quarter: u16) -> f32 {
        64.0 * self.zoom / f32::from(ticks_per_quarter)
    }
}

/// The track or shared pattern the roll's selection points at.
pub fn sequence_context(project: &Project, roll: &RollState, document: u64) -> SequenceContext {
    let part = roll.selected_part.filter(|part| {
        project
            .tracks
            .get(roll.selected_track)
            .is_some_and(|track| *part < track.parts.len())
    });
    let pattern = part.map(|part| project.tracks[roll.selected_track].parts[part].pattern);
    SequenceContext {
        document,
        track: roll.selected_track,
        part,
        pattern,
    }
}

/// The track or pattern a context names, to edit.
pub fn sequence_mut(
    project: &mut Project,
    context: SequenceContext,
) -> Option<&mut dyn EventSequence> {
    match context.pattern {
        Some(pattern) => project
            .patterns
            .get_mut(pattern)
            .map(|pattern| pattern as &mut dyn EventSequence),
        None => project
            .tracks
            .get_mut(context.track)
            .map(|track| track as &mut dyn EventSequence),
    }
}

/// The notes a context names, or none when the selection no longer exists.
pub fn sequence_notes(project: &Project, context: SequenceContext) -> &[Note] {
    match context.pattern {
        Some(pattern) => project
            .patterns
            .get(pattern)
            .map_or(&[], |pattern| &pattern.notes),
        None => project
            .tracks
            .get(context.track)
            .map_or(&[], |track| &track.notes),
    }
}

pub fn timeline_ticks(sequence: &(impl EventSequence + ?Sized), ticks_per_quarter: u16) -> u64 {
    let beat = u64::from(ticks_per_quarter.max(1));
    let longest_event = sequence
        .notes()
        .iter()
        .map(|note| note.start_tick + note.duration_ticks)
        .chain(sequence.pitch_bends().iter().map(|event| event.tick))
        .chain(sequence.aftertouch().iter().map(|event| event.tick))
        .chain(sequence.controllers().iter().map(|event| event.tick))
        .chain(sequence.mixer_automation().iter().map(|event| event.tick))
        .max()
        .unwrap_or(0);
    (longest_event.saturating_add(4 * beat)).max(16 * beat)
}

pub fn bar_ticks(project: &Project) -> u64 {
    Meter::of(project).bar_ticks()
}

#[derive(Clone, Copy)]
pub struct RollContext {
    pub ticks_per_quarter: u16,
    pub bar_ticks: u64,
}

impl RollContext {
    pub fn new(project: &Project) -> Self {
        Self {
            ticks_per_quarter: project.ticks_per_quarter,
            bar_ticks: bar_ticks(project),
        }
    }
}

/// Everything about the sequence the roll edits that is not its notes.
#[derive(Debug, Clone, Copy)]
pub struct EditedSequence {
    pub context: SequenceContext,
    pub meter: Meter,
    pub origin: Origin,
    pub grid: EditGrid,
}

impl EditedSequence {
    pub fn new(project: &Project, context: SequenceContext) -> Self {
        let beat = u64::from(project.ticks_per_quarter.max(1));
        let origin = match context.pattern {
            Some(pattern) => Origin::Pattern {
                sharing_parts: crate::arranger::cursor::parts_sharing(project, pattern),
            },
            None => Origin::Song,
        };
        Self {
            context,
            meter: Meter::of(project),
            origin,
            grid: EditGrid {
                snap: (beat / 4).max(1),
                beat,
                pattern_length: context
                    .pattern
                    .and_then(|pattern| project.patterns.get(pattern))
                    .map(|pattern| pattern.length_ticks),
            },
        }
    }

    /// The roll's name for the notes as they are: what its cursor is on.
    pub fn roll_name(&self, notes: &[Note], state: &RollState) -> String {
        let cursor = state
            .cursor
            .index(self.context)
            .and_then(|index| notes.get(index));
        speech::roll_name(cursor, notes.len(), self.meter, self.origin)
    }
}

/// Finds the cursor's note again after a change the roll did not make, and says so when the
/// cursor had to be cleared.
pub fn reconcile(
    sequence: &dyn EventSequence,
    state: &mut RollState,
    edited: &EditedSequence,
    announcer: &mut Announcer,
) {
    if state.cursor.reconcile(edited.context, sequence.notes()) == Reconciled::Reset {
        announce(announcer, Outcome::SelectionReset, edited.origin);
    }
}

/// Runs one command of the keyboard model on the cursor's note and announces its outcome.
pub fn run_command(
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    edited: &EditedSequence,
    command: RollCommand,
    announcer: &mut Announcer,
) -> Edited {
    let cursor = state.cursor.index(edited.context);
    let result = edits::apply(sequence.notes_mut(), cursor, command, edited.grid);
    state
        .cursor
        .set(edited.context, result.cursor, sequence.notes());
    // A refusal leaves the view alone; a move to another note, or an edit that may have taken
    // the note out of view, brings it back in view — and only as far as needed. After a deletion
    // the cursor's index can be the deleted note's own and still name a different, distant note.
    state.scroll_to_cursor = result.cursor.is_some()
        && (result.cursor != cursor
            || matches!(
                result.outcome,
                Outcome::Edited | Outcome::Added | Outcome::Deleted
            ));
    announce(announcer, result.outcome, edited.origin);
    result
}

fn announce(announcer: &mut Announcer, outcome: Outcome, origin: Origin) {
    if let Some(text) = speech::describe_outcome(outcome, origin) {
        announcer.announce(text);
    }
}

/// Shows the roll's toolbar and canvas. Keys reach the roll through the application's ordered
/// key dispatch, before any panel is drawn; the pointer and the buttons act here, through the
/// same edits. Returns whether the notes changed.
pub fn show(
    ui: &mut egui::Ui,
    edited: &EditedSequence,
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    announcer: &mut Announcer,
) -> bool {
    let original_notes = sequence.notes().to_vec();
    reconcile(sequence, state, edited, announcer);
    toolbar::show(ui, edited, sequence, state, announcer);
    ui.add_space(4.0);

    let ticks_per_quarter = edited.meter.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let timeline_ticks = edited
        .grid
        .pattern_length
        .unwrap_or_else(|| timeline_ticks(sequence, ticks_per_quarter))
        .max(beat);
    let pixels_per_tick = state.pixels_per_tick(ticks_per_quarter);
    let size = Vec2::new(
        KEYBOARD_WIDTH + timeline_ticks as f32 * pixels_per_tick,
        RULER_HEIGHT + (u16::from(MAX_PITCH) + 1) as f32 * KEY_HEIGHT,
    );
    let mut scroll = egui::ScrollArea::both()
        .id_salt("piano-roll")
        .auto_shrink([false, false]);
    if state.scroll_to_middle {
        let notes = sequence.notes();
        let middle = match (
            notes.iter().map(|note| note.pitch).min(),
            notes.iter().map(|note| note.pitch).max(),
        ) {
            (Some(low), Some(high)) => ((u16::from(low) + u16::from(high)) / 2) as u8,
            _ => 60,
        };
        scroll = scroll.scroll_offset(Vec2::new(
            0.0,
            f32::from(MAX_PITCH - middle.saturating_add(10).min(MAX_PITCH)) * KEY_HEIGHT,
        ));
        state.scroll_to_middle = false;
    }

    let scroll_area = scroll.show_viewport(ui, |ui, viewport| {
        let (response, full_painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        // egui gives focus on click only to text fields; without this a pointer selection is
        // something no key can act on.
        if response.clicked_by(egui::PointerButton::Primary)
            || response.drag_started()
            || std::mem::take(&mut state.focus_when_shown)
        {
            response.request_focus();
        }
        state.widget_id = Some(response.id);
        crate::custom_editors::register(ui.ctx(), response.id);
        regions::prefer_stop(ui.ctx(), Region::Editor, response.id);
        if response.has_focus() {
            // Without the filter an arrow key moves focus off the roll; Tab stays egui's.
            let arrows = egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            };
            ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, arrows));
        } else if state.had_focus {
            announcer.focus_lost();
        }
        let board = response.rect;
        let rect = Rect::from_min_max(
            board.min + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
            board.max,
        );
        let visible_origin = board.min + viewport.min.to_vec2();
        let edit_area = Rect::from_min_max(
            visible_origin + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
            full_painter.clip_rect().max,
        );
        let canvas = Canvas {
            edit_area,
            rect,
            pixels_per_tick,
        };
        pointer_edits(ui, &response, canvas, edited, sequence, state, announcer);

        let painter = full_painter.with_clip_rect(edit_area.intersect(full_painter.clip_rect()));
        painter.rect_filled(rect, 0.0, theme::BACKGROUND);
        drawing::draw_grid(
            &painter,
            rect,
            timeline_ticks,
            beat,
            edited.meter.bar_ticks(),
            pixels_per_tick,
        );
        let cursor = state.cursor.index(edited.context);
        drawing::draw_notes(
            &painter,
            sequence.notes(),
            rect,
            pixels_per_tick,
            cursor,
            state.selected_track,
        );
        drawing::draw_keyboard(&full_painter, rect, visible_origin);
        drawing::draw_ruler(
            &full_painter,
            rect,
            visible_origin,
            timeline_ticks,
            edited.meter.bar_ticks(),
            pixels_per_tick,
        );
        if std::mem::take(&mut state.scroll_to_cursor)
            && let Some(note) = cursor.and_then(|index| sequence.notes().get(index))
        {
            let note_rect = drawing::note_rect(note, rect, pixels_per_tick);
            ui.scroll_to_rect(drawing::scroll_target(note_rect), None);
        }
        if response.has_focus() {
            full_painter.rect_stroke(
                Rect::from_min_size(visible_origin, viewport.size()).shrink(1.0),
                0.0,
                Stroke::new(2.0, theme::IVORY),
                egui::StrokeKind::Inside,
            );
        }
        let name = announcer.focused_name(response.id, edited.roll_name(sequence.notes(), state));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(accesskit::Role::Canvas);
            node.set_label(name);
            node.set_description(speech::roll_key_help(speech::platform_command_key()));
        });
    });
    crate::accessibility::name_scroll_bars(ui.ctx(), scroll_area.id, "Piano roll");
    original_notes != sequence.notes()
}

/// Where the canvas is: the visible edit area, the whole grid, and its scale.
#[derive(Clone, Copy)]
struct Canvas {
    edit_area: Rect,
    rect: Rect,
    pixels_per_tick: f32,
}

/// The pointer's gestures, through the same edits as the keys.
fn pointer_edits(
    ui: &egui::Ui,
    response: &egui::Response,
    canvas: Canvas,
    edited: &EditedSequence,
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    announcer: &mut Announcer,
) {
    let Canvas {
        edit_area,
        rect,
        pixels_per_tick,
    } = canvas;
    let snap = edited.grid.snap;
    let mut result = None;
    if response.drag_started()
        && let Some(position) = ui.input(|input| input.pointer.press_origin())
        && edit_area.contains(position)
    {
        let index = drawing::note_at(sequence, position, rect, pixels_per_tick);
        let near_end = index.is_some_and(|index| {
            let note = &sequence.notes()[index];
            (position.x - drawing::note_rect(note, rect, pixels_per_tick).max.x).abs() <= 4.0
        });
        state.resizing_note = index.filter(|_| near_end);
        state.dragging_note = index.filter(|_| !near_end);
        result = Some(edits::select(sequence.notes(), index));
    }
    if response.dragged()
        && let Some(position) = response.interact_pointer_pos()
    {
        let (pitch, tick) = drawing::position_to_note(position, rect, pixels_per_tick, snap);
        if let Some(index) = state.dragging_note {
            result = Some(edits::move_to(
                sequence.notes_mut(),
                index,
                pitch,
                tick,
                edited.grid,
            ));
        } else if let Some(index) = state.resizing_note {
            result = Some(edits::resize_to(
                sequence.notes_mut(),
                index,
                tick,
                edited.grid,
            ));
        }
    }
    if response.drag_stopped() {
        state.dragging_note = None;
        state.resizing_note = None;
    }
    if response.clicked_by(egui::PointerButton::Primary)
        && let Some(position) = response.interact_pointer_pos()
        && edit_area.contains(position)
    {
        result = Some(
            match drawing::note_at(sequence, position, rect, pixels_per_tick) {
                Some(index) => edits::select(sequence.notes(), Some(index)),
                None => {
                    let (pitch, start_tick) =
                        drawing::position_to_note(position, rect, pixels_per_tick, snap);
                    edits::add_at(sequence.notes_mut(), pitch, start_tick, edited.grid)
                }
            },
        );
    }
    if let Some(result) = result {
        state
            .cursor
            .set(edited.context, result.cursor, sequence.notes());
        if result.outcome == Outcome::NoRoom {
            announce(announcer, result.outcome, edited.origin);
        }
    }
}

#[cfg(test)]
mod tests;
