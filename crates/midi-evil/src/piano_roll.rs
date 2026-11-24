use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use evil_project::{EventSequence, Note, Project};

use crate::description::{self, Meter, Origin, Outcome};
use crate::theme;

const KEY_HEIGHT: f32 = 18.0;
const KEYBOARD_WIDTH: f32 = 62.0;
const RULER_HEIGHT: f32 = 26.0;
const MAX_PITCH: u8 = 127;
const DEFAULT_NOTE_VELOCITY: u8 = 100;
const MIDDLE_C: u8 = 60;

#[derive(Debug, Clone, Copy)]
pub struct PartDragState {
    pub track: usize,
    pub part: usize,
    pub start_tick: u64,
    pub press_x: f32,
}

#[derive(Debug)]
pub struct RollState {
    pub selected_track: usize,
    pub selected_part: Option<usize>,
    pub selected_note: Option<usize>,
    pub part_drag: Option<PartDragState>,
    clipboard: Vec<Note>,
    dragging_note: Option<usize>,
    resizing_note: Option<usize>,
    zoom: f32,
    scroll_to_middle: bool,
    /// The roll's widget id, so the editor region's F6 stop can focus it.
    pub focus_id: Option<egui::Id>,
    /// The notes and cursor as the roll last left them; a difference at the next frame is a
    /// change the roll did not make, and the cursor finds its note again by value.
    synced_notes: Vec<Note>,
    synced_index: Option<usize>,
    last_outcome: Option<Outcome>,
    exposed_name: String,
    /// Set in a frame where a key was handled, for the status line.
    pub spoken_this_frame: Option<String>,
    /// The newest name, never held back a frame: a surface with no accessibility node of its
    /// own (the web build) reads it here.
    pub current_name: String,
    /// The notice raised this frame, for the same surfaces.
    pub notice_this_frame: Option<String>,
}

impl Default for RollState {
    fn default() -> Self {
        Self {
            selected_track: 0,
            selected_part: None,
            selected_note: None,
            part_drag: None,
            clipboard: Vec::new(),
            dragging_note: None,
            resizing_note: None,
            zoom: 1.0,
            scroll_to_middle: true,
            focus_id: None,
            synced_notes: Vec::new(),
            synced_index: None,
            last_outcome: None,
            exposed_name: String::new(),
            spoken_this_frame: None,
            current_name: String::new(),
            notice_this_frame: None,
        }
    }
}

impl RollState {
    pub fn select_track(&mut self, index: usize) {
        self.selected_track = index;
        self.selected_part = None;
        self.selected_note = None;
        self.dragging_note = None;
        self.resizing_note = None;
        self.scroll_to_middle = true;
    }

    pub fn select_part(&mut self, track: usize, part: Option<usize>) {
        if self.selected_track != track || self.selected_part != part {
            self.selected_note = None;
            self.dragging_note = None;
            self.resizing_note = None;
            self.scroll_to_middle = true;
        }
        self.selected_track = track;
        self.selected_part = part;
    }

    pub fn clamp(&mut self, project: &Project) {
        self.dragging_note = None;
        self.resizing_note = None;
        if project.tracks.is_empty() {
            self.selected_track = 0;
            self.selected_part = None;
            self.selected_note = None;
            return;
        }
        self.selected_track = self.selected_track.min(project.tracks.len() - 1);
        let track = &project.tracks[self.selected_track];
        self.selected_part = self
            .selected_part
            .filter(|index| *index < track.parts.len());
        let note_count = self.selected_part.map_or(track.notes.len(), |part| {
            project.patterns[track.parts[part].pattern].notes.len()
        });
        self.selected_note = self.selected_note.filter(|index| *index < note_count);
    }

    pub fn pixels_per_tick(&self, ticks_per_quarter: u16) -> f32 {
        64.0 * self.zoom / f32::from(ticks_per_quarter)
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
    (u64::from(project.ticks_per_quarter.max(1)) * 4 * u64::from(project.beats_per_bar.max(1))
        / u64::from(project.beat_unit.max(1)))
    .max(1)
}

#[derive(Clone, Copy)]
pub struct RollContext {
    pub ticks_per_quarter: u16,
    pub bar_ticks: u64,
    pub meter: Meter,
    /// How many arrangement parts play the pattern shown; `None` for a unique timeline.
    pub sharing_parts: Option<usize>,
}

impl RollContext {
    pub fn new(project: &Project) -> Self {
        Self {
            ticks_per_quarter: project.ticks_per_quarter,
            bar_ticks: bar_ticks(project),
            meter: Meter {
                ticks_per_quarter: project.ticks_per_quarter,
                beats_per_bar: project.beats_per_bar,
                beat_unit: project.beat_unit,
            },
            sharing_parts: None,
        }
    }

    fn origin(self) -> Origin {
        match self.sharing_parts {
            Some(sharing_parts) => Origin::Pattern { sharing_parts },
            None => Origin::Song,
        }
    }
}

/// One keyboard command on the roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollCommand {
    Previous,
    Next,
    First,
    Last,
    PitchUp,
    PitchDown,
    OctaveUp,
    OctaveDown,
    Earlier,
    Later,
    Shorter,
    Longer,
    Add,
    Delete,
}

impl RollCommand {
    fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        use egui::Key;
        let command = if modifiers.is_none() {
            match key {
                Key::ArrowLeft => Self::Previous,
                Key::ArrowRight => Self::Next,
                Key::Home => Self::First,
                Key::End => Self::Last,
                Key::ArrowUp => Self::PitchUp,
                Key::ArrowDown => Self::PitchDown,
                Key::Enter => Self::Add,
                Key::Delete => Self::Delete,
                _ => return None,
            }
        } else if modifiers.shift_only() {
            match key {
                Key::ArrowUp => Self::OctaveUp,
                Key::ArrowDown => Self::OctaveDown,
                Key::ArrowLeft => Self::Shorter,
                Key::ArrowRight => Self::Longer,
                _ => return None,
            }
        } else if modifiers.command_only() {
            match key {
                Key::ArrowLeft => Self::Earlier,
                Key::ArrowRight => Self::Later,
                _ => return None,
            }
        } else {
            return None;
        };
        Some(command)
    }
}

/// The grid the keyboard edits move on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditGrid {
    pub snap: u64,
    pub beat: u64,
    /// A shared pattern's length; every note must start inside it.
    pub pattern_length: Option<u64>,
}

/// Note indices in the order the cursor walks them: by start, then pitch, then index.
pub fn walk_order(notes: &[Note]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by_key(|&index| (notes[index].start_tick, notes[index].pitch, index));
    order
}

/// Applies one command to the notes and returns the new cursor and what happened. The cursor
/// is an index into `notes`; one out of range is treated as no cursor.
pub fn apply_command(
    notes: &mut Vec<Note>,
    cursor: Option<usize>,
    command: RollCommand,
    grid: EditGrid,
) -> (Option<usize>, Outcome) {
    let cursor = cursor.filter(|&index| index < notes.len());
    if notes.is_empty() && command != RollCommand::Add {
        return (None, Outcome::Empty);
    }
    let order = walk_order(notes);
    let place = cursor.and_then(|index| order.iter().position(|&walked| walked == index));
    match command {
        RollCommand::Next => match place {
            None => (Some(order[0]), Outcome::Moved),
            Some(place) if place + 1 < order.len() => (Some(order[place + 1]), Outcome::Moved),
            Some(_) => (cursor, Outcome::AlreadyLast),
        },
        RollCommand::Previous => match place {
            None => (order.last().copied(), Outcome::Moved),
            Some(0) => (cursor, Outcome::AlreadyFirst),
            Some(place) => (Some(order[place - 1]), Outcome::Moved),
        },
        RollCommand::First => match place {
            Some(0) => (cursor, Outcome::AlreadyFirst),
            _ => (Some(order[0]), Outcome::Moved),
        },
        RollCommand::Last => match place {
            Some(place) if place + 1 == order.len() => (cursor, Outcome::AlreadyLast),
            _ => (order.last().copied(), Outcome::Moved),
        },
        RollCommand::Add => add_note(notes, cursor, grid),
        RollCommand::Delete => match (cursor, place) {
            (Some(index), Some(place)) => {
                let neighbour = order
                    .get(place + 1)
                    .or_else(|| place.checked_sub(1).and_then(|before| order.get(before)))
                    .copied();
                let removed = notes.remove(index);
                let neighbour =
                    neighbour.map(|other| if other > index { other - 1 } else { other });
                (neighbour, Outcome::Deleted(removed))
            }
            _ => (None, Outcome::NoSelection),
        },
        _ => {
            let Some(index) = cursor else {
                return (None, Outcome::NoSelection);
            };
            let outcome = edit_note(&mut notes[index], command, grid);
            (Some(index), outcome)
        }
    }
}

fn add_note(
    notes: &mut Vec<Note>,
    cursor: Option<usize>,
    grid: EditGrid,
) -> (Option<usize>, Outcome) {
    let (pitch, start_tick) = match cursor {
        Some(index) => {
            let after = &notes[index];
            match after.start_tick.checked_add(after.duration_ticks) {
                Some(end) => (after.pitch, end),
                None => return (cursor, Outcome::NoRoom),
            }
        }
        None => (MIDDLE_C, 0),
    };
    let beyond_pattern = grid
        .pattern_length
        .is_some_and(|length| start_tick >= length);
    if beyond_pattern || start_tick.checked_add(grid.beat).is_none() {
        return (cursor, Outcome::NoRoom);
    }
    notes.push(Note {
        pitch,
        velocity: DEFAULT_NOTE_VELOCITY,
        start_tick,
        duration_ticks: grid.beat,
    });
    (Some(notes.len() - 1), Outcome::Added)
}

fn edit_note(note: &mut Note, command: RollCommand, grid: EditGrid) -> Outcome {
    let snap = grid.snap.max(1);
    match command {
        RollCommand::PitchUp | RollCommand::OctaveUp => {
            if note.pitch == MAX_PITCH {
                return Outcome::AlreadyHighest;
            }
            let step = if command == RollCommand::OctaveUp {
                12
            } else {
                1
            };
            note.pitch = note.pitch.saturating_add(step).min(MAX_PITCH);
        }
        RollCommand::PitchDown | RollCommand::OctaveDown => {
            if note.pitch == 0 {
                return Outcome::AlreadyLowest;
            }
            let step = if command == RollCommand::OctaveDown {
                12
            } else {
                1
            };
            note.pitch = note.pitch.saturating_sub(step);
        }
        RollCommand::Earlier => {
            if note.start_tick == 0 {
                return Outcome::AlreadyAtStart;
            }
            note.start_tick = note.start_tick.saturating_sub(snap);
        }
        RollCommand::Later => {
            let latest_start = match grid.pattern_length {
                Some(length) => (length.saturating_sub(1) / snap) * snap,
                None => u64::MAX - note.duration_ticks,
            };
            if note.start_tick >= latest_start {
                return Outcome::AlreadyAtEnd;
            }
            note.start_tick = note.start_tick.saturating_add(snap).min(latest_start);
        }
        RollCommand::Shorter => {
            if note.duration_ticks <= snap {
                return Outcome::AlreadyShortest;
            }
            note.duration_ticks = (note.duration_ticks - snap).max(snap);
        }
        RollCommand::Longer => {
            let Some(longer) = note
                .duration_ticks
                .checked_add(snap)
                .filter(|longer| note.start_tick.checked_add(*longer).is_some())
            else {
                return Outcome::AlreadyAtEnd;
            };
            note.duration_ticks = longer;
        }
        RollCommand::Previous
        | RollCommand::Next
        | RollCommand::First
        | RollCommand::Last
        | RollCommand::Add
        | RollCommand::Delete => unreachable!("navigation, Add and Delete are not note edits"),
    }
    Outcome::Edited
}

/// Where the cursor's note is after a change the roll did not make: Clean overlaps re-sorts and
/// shortens, undo replaces the list, the inspector edits in place. `None` when the note cannot be
/// found again — the cursor then resets rather than point at a different note.
pub fn relocate_cursor(
    previous: &[Note],
    previous_index: usize,
    current: &[Note],
) -> Option<usize> {
    let cursor_note = *previous.get(previous_index)?;
    let occurrence = previous[..previous_index]
        .iter()
        .filter(|note| **note == cursor_note)
        .count();
    let equal: Vec<usize> = (0..current.len())
        .filter(|&index| current[index] == cursor_note)
        .collect();
    if let Some(&index) = equal.get(occurrence).or(equal.first()) {
        return Some(index);
    }
    let edited_in_place = previous.len() == current.len()
        && (0..current.len())
            .all(|index| index == previous_index || current[index] == previous[index]);
    if edited_in_place {
        return Some(previous_index);
    }
    let same_onset: Vec<usize> = (0..current.len())
        .filter(|&index| {
            current[index].start_tick == cursor_note.start_tick
                && current[index].pitch == cursor_note.pitch
        })
        .collect();
    match same_onset.as_slice() {
        [index] => Some(*index),
        _ => None,
    }
}

pub fn show(
    ui: &mut egui::Ui,
    context: RollContext,
    sequence: &mut dyn EventSequence,
    state: &mut RollState,
    pattern_length: Option<u64>,
) -> bool {
    let original_notes = sequence.notes().to_vec();
    ui.horizontal_wrapped(|ui| {
        theme::label(
            ui,
            if pattern_length.is_some() {
                "SHARED PATTERN"
            } else {
                "UNIQUE TIMELINE"
            },
        );
        ui.add(egui::Slider::new(&mut state.zoom, 0.25..=3.0).text("Zoom"));
        let selected = state.selected_note;
        if ui.button("Quantize").clicked() {
            let _ = evil_project::quantize_notes(
                sequence,
                (u64::from(context.ticks_per_quarter.max(1)) / 4).max(1),
                selected,
                pattern_length,
            );
        }
        for (label, semitones) in [("−12", -12), ("−1", -1), ("+1", 1), ("+12", 12)] {
            if ui.button(label).clicked() {
                let _ = evil_project::transpose_notes(sequence, semitones, selected);
            }
        }
        if ui.button("Clean overlaps").clicked() {
            evil_project::clean_overlaps(sequence);
        }
        if ui.button("Copy").clicked() {
            state.clipboard = match selected.and_then(|index| sequence.notes().get(index)) {
                Some(note) => vec![*note],
                None => sequence.notes().to_vec(),
            };
        }
        let paste = !state.clipboard.is_empty() && ui.button("Paste").clicked();
        if paste {
            let origin = state
                .clipboard
                .iter()
                .map(|note| note.start_tick)
                .min()
                .unwrap_or(0);
            let destination = selected
                .and_then(|index| sequence.notes().get(index))
                .map_or(0, |note| note.start_tick + note.duration_ticks);
            let additions: Vec<_> = state
                .clipboard
                .iter()
                .filter_map(|note| {
                    let start_tick = destination.checked_add(note.start_tick - origin)?;
                    start_tick.checked_add(note.duration_ticks)?;
                    Some(Note {
                        start_tick,
                        ..*note
                    })
                })
                .filter(|note| pattern_length.is_none_or(|length| note.start_tick < length))
                .collect();
            state.selected_note = (!additions.is_empty()).then_some(sequence.notes().len());
            sequence.notes_mut().extend(additions);
        }
        ui.label(
            egui::RichText::new(if selected.is_some() {
                "Operations affect the selected note · Copy/Paste inserts after it"
            } else {
                "Operations affect all notes · Click to draw · Drag to move/resize"
            })
            .size(11.0)
            .color(theme::MUTED),
        );
    });
    ui.add_space(4.0);

    let ticks_per_quarter = context.ticks_per_quarter.max(1);
    let beat = u64::from(ticks_per_quarter);
    let snap = (beat / 4).max(1);
    let bar = context.bar_ticks;
    let timeline_ticks = pattern_length
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
            notes.iter().map(|n| n.pitch).min(),
            notes.iter().map(|n| n.pitch).max(),
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

    scroll
        .show_viewport(ui, |ui, viewport| {
            let (response, full_painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
            let board = response.rect;
            // egui gives focus on click only to text fields; without this a click selects nothing
            // a key can act on.
            if response.clicked_by(egui::PointerButton::Primary) || response.drag_started() {
                response.request_focus();
            }
            state.focus_id = Some(response.id);
            state.spoken_this_frame = None;
            state.notice_this_frame = None;
            reconcile_cursor(state, sequence.notes());
            let mut notice = None;
            let mut cursor_moved = false;
            if state.last_outcome == Some(Outcome::SelectionLost) && state.synced_index.is_some() {
                notice = description::notice(Outcome::SelectionLost, context.origin());
            }
            if response.has_focus() {
                // Without the filter an arrow key moves focus off the roll; Tab is left alone.
                let arrows = egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                };
                ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, arrows));
                let grid = EditGrid {
                    snap,
                    beat,
                    pattern_length,
                };
                // Every press counts, repeats included, in the order they came: a held arrow
                // moves once per repeat even when several arrive in one frame.
                let commands: Vec<RollCommand> = ui.input(|input| {
                    input
                        .events
                        .iter()
                        .filter_map(|event| match event {
                            egui::Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } => RollCommand::from_key(*key, *modifiers),
                            _ => None,
                        })
                        .collect()
                });
                for command in commands {
                    let (cursor, outcome) =
                        apply_command(sequence.notes_mut(), state.selected_note, command, grid);
                    state.selected_note = cursor;
                    state.last_outcome = Some(outcome);
                    notice = description::notice(outcome, context.origin());
                    cursor_moved = true;
                }
            }
            let rect = Rect::from_min_max(
                board.min + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
                board.max,
            );
            let visible_origin = board.min + viewport.min.to_vec2();
            let edit_area = Rect::from_min_max(
                visible_origin + Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT),
                full_painter.clip_rect().max,
            );
            let painter =
                full_painter.with_clip_rect(edit_area.intersect(full_painter.clip_rect()));
            painter.rect_filled(rect, 0.0, theme::BACKGROUND);
            draw_grid(&painter, rect, timeline_ticks, beat, bar, pixels_per_tick);

            for (index, note) in sequence.notes().iter().enumerate() {
                let note_rect = note_rect(note, rect, pixels_per_tick);
                let selected = state.selected_note == Some(index);
                painter.rect_filled(
                    note_rect,
                    2.0,
                    if selected {
                        theme::IVORY
                    } else {
                        theme::track_color(state.selected_track)
                    },
                );
                painter.rect_stroke(
                    note_rect,
                    2.0,
                    Stroke::new(1.0, theme::TEXT.gamma_multiply(0.35)),
                    egui::StrokeKind::Inside,
                );
                if note_rect.width() >= 36.0 {
                    painter.text(
                        note_rect.left_center() + Vec2::new(5.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        note_name(note.pitch),
                        egui::FontId::monospace(10.0),
                        theme::BACKGROUND,
                    );
                }
            }
            draw_keyboard(&full_painter, rect, visible_origin);
            draw_ruler(
                &full_painter,
                rect,
                visible_origin,
                timeline_ticks,
                bar,
                pixels_per_tick,
            );

            let mut changed = false;
            if response.drag_started()
                && let Some(position) = ui.input(|input| input.pointer.press_origin())
                && edit_area.contains(position)
            {
                if let Some(index) = note_at(sequence, position, rect, pixels_per_tick) {
                    state.selected_note = Some(index);
                    let note = &sequence.notes()[index];
                    let right_edge = note_rect(note, rect, pixels_per_tick).max.x;
                    if (position.x - right_edge).abs() <= 4.0 {
                        state.resizing_note = Some(index);
                        state.dragging_note = None;
                    } else {
                        state.dragging_note = Some(index);
                        state.resizing_note = None;
                    }
                } else {
                    state.selected_note = None;
                    state.dragging_note = None;
                    state.resizing_note = None;
                }
            }
            if response.dragged()
                && let (Some(index), Some(position)) =
                    (state.dragging_note, response.interact_pointer_pos())
                && let Some(note) = sequence.notes_mut().get_mut(index)
            {
                let (pitch, tick) = position_to_note(position, rect, pixels_per_tick, snap);
                let tick =
                    pattern_length.map_or(tick, |length| tick.min(length.saturating_sub(snap)));
                if note.pitch != pitch || note.start_tick != tick {
                    note.pitch = pitch;
                    note.start_tick = tick;
                    changed = true;
                }
            }
            if response.dragged()
                && let (Some(index), Some(position)) =
                    (state.resizing_note, response.interact_pointer_pos())
                && let Some(note) = sequence.notes_mut().get_mut(index)
            {
                let (_, end_tick) = position_to_note(position, rect, pixels_per_tick, snap);
                let duration = end_tick.saturating_sub(note.start_tick).max(snap);
                if note.duration_ticks != duration {
                    note.duration_ticks = duration;
                    changed = true;
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
                if let Some(index) = note_at(sequence, position, rect, pixels_per_tick) {
                    state.selected_note = Some(index);
                } else {
                    let (pitch, start_tick) =
                        position_to_note(position, rect, pixels_per_tick, snap);
                    let start_tick = pattern_length.map_or(start_tick, |length| {
                        start_tick.min(length.saturating_sub(snap))
                    });
                    let index = sequence.notes().len();
                    sequence.notes_mut().push(Note {
                        pitch,
                        start_tick,
                        duration_ticks: beat,
                        velocity: DEFAULT_NOTE_VELOCITY,
                    });
                    state.selected_note = Some(index);
                    changed = true;
                }
            }
            if state.selected_note != state.synced_index && !cursor_moved {
                // The pointer chose a note; what the last key did no longer describes it.
                state.last_outcome = None;
            }
            if cursor_moved
                && let Some(note) = state
                    .selected_note
                    .and_then(|index| sequence.notes().get(index))
            {
                ui.scroll_to_rect(
                    note_rect(note, rect, pixels_per_tick),
                    Some(egui::Align::Center),
                );
            }
            if response.has_focus() {
                full_painter.rect_stroke(
                    Rect::from_min_size(visible_origin, viewport.size()).shrink(1.0),
                    0.0,
                    Stroke::new(2.0, theme::IVORY),
                    egui::StrokeKind::Inside,
                );
            }
            state.synced_notes = sequence.notes().to_vec();
            state.synced_index = state.selected_note;
            let name = description::roll_name(
                state
                    .selected_note
                    .and_then(|index| sequence.notes().get(index)),
                sequence.notes().len(),
                state.last_outcome,
                context.meter,
                context.origin(),
            );
            if cursor_moved {
                state.spoken_this_frame = Some(name.clone());
            }
            state.current_name = name.clone();
            state.notice_this_frame = notice.clone();
            match notice {
                // The notice gets a frame of its own: when the focused node's name changes in
                // the same frame, the name is what a screen reader speaks.
                Some(notice) => {
                    ui.ctx()
                        .accesskit_node_builder(response.id.with("roll-notice"), |node| {
                            node.set_role(egui::accesskit::Role::Label);
                            node.set_value(notice);
                            node.set_live(egui::accesskit::Live::Polite);
                        });
                    ui.ctx().request_repaint();
                }
                None => state.exposed_name = name,
            }
            let exposed_name = state.exposed_name.clone();
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_role(egui::accesskit::Role::Canvas);
                node.set_label(exposed_name);
                node.set_description(description::ROLL_KEYS);
            });
            changed
        })
        .inner
        || original_notes != sequence.notes()
}

/// Finds the cursor's note again after a change the roll did not make, and adopts a selection
/// someone else made (the pointer, the inspector, a track or part change).
fn reconcile_cursor(state: &mut RollState, notes: &[Note]) {
    if state.selected_note != state.synced_index {
        state.last_outcome = None;
        return;
    }
    if notes == state.synced_notes.as_slice() {
        return;
    }
    if let Some(index) = state.selected_note {
        state.selected_note = relocate_cursor(&state.synced_notes, index, notes);
        if state.selected_note.is_none() {
            state.last_outcome = Some(Outcome::SelectionLost);
        }
    }
}

fn draw_grid(
    painter: &egui::Painter,
    rect: Rect,
    timeline_ticks: u64,
    beat: u64,
    bar: u64,
    pixels_per_tick: f32,
) {
    for pitch in 0..=MAX_PITCH {
        let y = rect.top() + f32::from(MAX_PITCH - pitch) * KEY_HEIGHT;
        if is_black_key(pitch) {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left(), y),
                    Pos2::new(rect.right(), y + KEY_HEIGHT),
                ),
                0.0,
                Color32::from_rgb(26, 23, 33),
            );
        }
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, Color32::from_rgb(42, 36, 49)),
        );
    }
    let grid = (beat / 4).max(1);
    let mut tick = (((painter.clip_rect().left() - rect.left()).max(0.0) / pixels_per_tick) as u64
        / grid)
        * grid;
    let last_tick = (((painter.clip_rect().right() - rect.left()).max(0.0) / pixels_per_tick)
        as u64)
        .min(timeline_ticks);
    while tick <= last_tick {
        let x = rect.left() + tick as f32 * pixels_per_tick;
        let (width, color) = if tick.is_multiple_of(bar) {
            (1.5, theme::BORDER)
        } else if tick.is_multiple_of(beat) {
            (1.0, Color32::from_rgb(55, 46, 63))
        } else {
            (0.5, Color32::from_rgb(35, 30, 42))
        };
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(width, color),
        );
        tick += grid;
    }
}

fn draw_keyboard(painter: &egui::Painter, grid: Rect, visible: Pos2) {
    let keyboard = Rect::from_min_max(
        Pos2::new(visible.x, visible.y + RULER_HEIGHT),
        Pos2::new(visible.x + KEYBOARD_WIDTH, painter.clip_rect().bottom()),
    );
    let painter = painter.with_clip_rect(keyboard.intersect(painter.clip_rect()));
    painter.rect_filled(keyboard, 0.0, theme::PANEL);
    for pitch in 0..=MAX_PITCH {
        let y = grid.top() + f32::from(MAX_PITCH - pitch) * KEY_HEIGHT;
        let black = is_black_key(pitch);
        let key = Rect::from_min_size(
            Pos2::new(visible.x, y + 0.5),
            Vec2::new(
                if black { 42.0 } else { KEYBOARD_WIDTH - 2.0 },
                KEY_HEIGHT - 1.0,
            ),
        );
        painter.rect_filled(
            key,
            1.0,
            if black {
                theme::BACKGROUND
            } else {
                Color32::from_rgb(190, 180, 169)
            },
        );
        if pitch % 12 == 0 {
            painter.text(
                key.right_center() - Vec2::new(5.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                octave_label(pitch),
                egui::FontId::monospace(10.0),
                theme::BACKGROUND,
            );
        }
    }
}

fn draw_ruler(
    painter: &egui::Painter,
    grid: Rect,
    visible: Pos2,
    ticks: u64,
    bar: u64,
    pixels: f32,
) {
    let ruler = Rect::from_min_max(
        visible + Vec2::new(KEYBOARD_WIDTH, 0.0),
        Pos2::new(painter.clip_rect().right(), visible.y + RULER_HEIGHT),
    );
    let ruler_painter = painter.with_clip_rect(ruler.intersect(painter.clip_rect()));
    ruler_painter.rect_filled(ruler, 0.0, theme::RAISED);
    let first = (((ruler.left() - grid.left()).max(0.0) / pixels) as u64 / bar) * bar;
    let last = (((ruler.right() - grid.left()).max(0.0) / pixels) as u64).min(ticks);
    for tick in (first..=last).step_by(bar as usize) {
        let x = grid.left() + tick as f32 * pixels;
        ruler_painter.text(
            Pos2::new(x + 6.0, ruler.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{:02}", tick / bar + 1),
            egui::FontId::monospace(11.0),
            theme::IVORY,
        );
    }
    let corner = Rect::from_min_size(visible, Vec2::new(KEYBOARD_WIDTH, RULER_HEIGHT));
    painter.rect_filled(corner, 0.0, theme::PANEL);
    painter.text(
        corner.center(),
        egui::Align2::CENTER_CENTER,
        "KEY",
        egui::FontId::monospace(10.0),
        theme::MUTED,
    );
}

fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(pitch % 12)],
        i32::from(pitch) / 12 - 1
    )
}

fn note_rect(note: &Note, rect: Rect, pixels_per_tick: f32) -> Rect {
    let x = rect.left() + note.start_tick as f32 * pixels_per_tick;
    let y = rect.top() + f32::from(MAX_PITCH - note.pitch) * KEY_HEIGHT;
    Rect::from_min_size(
        Pos2::new(x, y + 1.0),
        Vec2::new(
            (note.duration_ticks as f32 * pixels_per_tick).max(3.0),
            KEY_HEIGHT - 2.0,
        ),
    )
}

fn note_at(
    sequence: &(impl EventSequence + ?Sized),
    position: Pos2,
    rect: Rect,
    pixels_per_tick: f32,
) -> Option<usize> {
    if !rect.contains(position) {
        return None;
    }
    let tick = ((position.x - rect.left()) / pixels_per_tick) as u64;
    let pitch = pitch_at(position, rect);
    sequence.notes().iter().position(|note| {
        note.pitch == pitch
            && tick >= note.start_tick
            && tick < note.start_tick + note.duration_ticks
    })
}

fn position_to_note(position: Pos2, rect: Rect, pixels_per_tick: f32, snap: u64) -> (u8, u64) {
    let raw_tick = ((position.x - rect.left()) / pixels_per_tick).max(0.0) as u64;
    (pitch_at(position, rect), snap_tick(raw_tick, snap))
}

fn pitch_at(position: Pos2, rect: Rect) -> u8 {
    let row = ((position.y - rect.top()) / KEY_HEIGHT).clamp(0.0, 127.0) as u8;
    MAX_PITCH - row
}

pub(crate) fn snap_tick(tick: u64, snap: u64) -> u64 {
    ((tick + snap / 2) / snap) * snap
}

fn octave_label(pitch: u8) -> String {
    format!("C{}", i32::from(pitch) / 12 - 1)
}

fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use evil_project::Track;

    #[test]
    fn bar_lengths_follow_the_imported_time_signature() {
        let mut project = Project::new("Meters");
        assert_eq!(bar_ticks(&project), 1_920);
        project.beats_per_bar = 6;
        project.beat_unit = 8;
        assert_eq!(bar_ticks(&project), 1_440);
        project.ticks_per_quarter = 384;
        project.beats_per_bar = 3;
        project.beat_unit = 4;
        assert_eq!(bar_ticks(&project), 1_152);
    }

    #[test]
    fn sticky_keyboard_and_ruler_do_not_create_notes_but_the_grid_does() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut project = Project::new("Hit testing");
        project.add_track(Track::default()).unwrap();
        let mut state = RollState::default();
        let mut time = 0.0;
        let mut frame = |events| {
            time += 0.1;
            let _ = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        RollContext::new(&project),
                        &mut project.tracks[0],
                        &mut state,
                        None,
                    );
                },
            );
            project.tracks[0].notes.len()
        };
        frame(vec![]);
        frame(vec![]);
        for (position, expected) in [
            (Pos2::new(24.0, 140.0), 0), // fixed keyboard, after initial vertical scroll
            (Pos2::new(180.0, 48.0), 0), // fixed bar ruler
            (Pos2::new(180.0, 140.0), 1),
        ] {
            frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            let count = frame(vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert_eq!(count, expected, "unexpected note creation at {position:?}");
        }
        assert_eq!(project.tracks[0].notes[0].start_tick, 840);
    }

    #[test]
    fn snaps_to_the_nearest_grid_point_without_underflow() {
        assert_eq!(snap_tick(0, 32), 0);
        assert_eq!(snap_tick(15, 32), 0);
        assert_eq!(snap_tick(16, 32), 32);
        assert_eq!(snap_tick(47, 32), 32);
        assert_eq!(snap_tick(48, 32), 64);
    }

    #[test]
    fn octave_labels_cover_the_full_midi_range() {
        for pitch in 0..=MAX_PITCH {
            let _ = octave_label(pitch);
        }
        assert_eq!(octave_label(0), "C-1");
        assert_eq!(octave_label(60), "C4");
        assert_eq!(octave_label(120), "C9");
    }

    #[test]
    fn timeline_extends_to_cover_automation() {
        let track = Track {
            pitch_bends: vec![evil_project::PitchBend {
                tick: 28 * 480,
                value: 0,
            }],
            ..Track::default()
        };
        assert_eq!(timeline_ticks(&track, 480), 32 * 480);
    }

    const PATTERN_GRID: EditGrid = EditGrid {
        snap: 120,
        beat: 480,
        pattern_length: Some(1_920),
    };
    const TIMELINE_GRID: EditGrid = EditGrid {
        snap: 120,
        beat: 480,
        pattern_length: None,
    };

    fn note_at(pitch: u8, start_tick: u64, duration_ticks: u64) -> Note {
        Note {
            pitch,
            velocity: 90,
            start_tick,
            duration_ticks,
        }
    }

    #[test]
    fn the_cursor_walks_by_start_then_pitch_then_index() {
        let notes = vec![
            note_at(64, 480, 120),
            note_at(60, 0, 120),
            note_at(60, 480, 120),
            note_at(60, 480, 120),
        ];
        assert_eq!(walk_order(&notes), vec![1, 2, 3, 0]);
        let mut edited = notes.clone();
        assert_eq!(
            apply_command(&mut edited, None, RollCommand::Next, TIMELINE_GRID),
            (Some(1), Outcome::Moved)
        );
        assert_eq!(
            apply_command(&mut edited, Some(3), RollCommand::Next, TIMELINE_GRID),
            (Some(0), Outcome::Moved)
        );
        assert_eq!(
            apply_command(&mut edited, Some(0), RollCommand::Next, TIMELINE_GRID),
            (Some(0), Outcome::AlreadyLast)
        );
        assert_eq!(
            apply_command(&mut edited, Some(1), RollCommand::Previous, TIMELINE_GRID),
            (Some(1), Outcome::AlreadyFirst)
        );
        assert_eq!(
            apply_command(&mut edited, Some(0), RollCommand::First, TIMELINE_GRID),
            (Some(1), Outcome::Moved)
        );
        assert_eq!(
            apply_command(&mut edited, Some(1), RollCommand::Last, TIMELINE_GRID),
            (Some(0), Outcome::Moved)
        );
        assert_eq!(edited, notes, "moving the cursor edits nothing");
    }

    #[test]
    fn pitch_is_clamped_at_127_and_at_0() {
        let mut notes = vec![note_at(127, 0, 120)];
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::PitchUp, TIMELINE_GRID),
            (Some(0), Outcome::AlreadyHighest)
        );
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::OctaveUp, TIMELINE_GRID),
            (Some(0), Outcome::AlreadyHighest)
        );
        assert_eq!(notes[0].pitch, 127);
        notes[0].pitch = 120;
        apply_command(&mut notes, Some(0), RollCommand::OctaveUp, TIMELINE_GRID);
        assert_eq!(notes[0].pitch, 127);
        notes[0].pitch = 0;
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::PitchDown, TIMELINE_GRID),
            (Some(0), Outcome::AlreadyLowest)
        );
        assert_eq!(notes[0].pitch, 0);
        notes[0].pitch = 5;
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::OctaveDown, TIMELINE_GRID),
            (Some(0), Outcome::Edited)
        );
        assert_eq!(notes[0].pitch, 0);
    }

    #[test]
    fn a_move_saturates_at_tick_0_and_at_the_patterns_last_snap() {
        let mut notes = vec![note_at(60, 0, 480)];
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::Earlier, PATTERN_GRID),
            (Some(0), Outcome::AlreadyAtStart)
        );
        notes[0].start_tick = 50;
        apply_command(&mut notes, Some(0), RollCommand::Earlier, PATTERN_GRID);
        assert_eq!(notes[0].start_tick, 0);
        notes[0].start_tick = 1_750;
        apply_command(&mut notes, Some(0), RollCommand::Later, PATTERN_GRID);
        assert_eq!(notes[0].start_tick, 1_800);
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::Later, PATTERN_GRID),
            (Some(0), Outcome::AlreadyAtEnd)
        );
        assert_eq!(notes[0].start_tick, 1_800);
        notes[0].start_tick = 1_800;
        apply_command(&mut notes, Some(0), RollCommand::Later, TIMELINE_GRID);
        assert_eq!(notes[0].start_tick, 1_920, "a unique timeline has no end");
    }

    #[test]
    fn a_note_is_never_shorter_than_one_snap() {
        let mut notes = vec![note_at(60, 0, 120)];
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::Shorter, TIMELINE_GRID),
            (Some(0), Outcome::AlreadyShortest)
        );
        notes[0].duration_ticks = 150;
        apply_command(&mut notes, Some(0), RollCommand::Shorter, TIMELINE_GRID);
        assert_eq!(notes[0].duration_ticks, 120);
        apply_command(&mut notes, Some(0), RollCommand::Longer, TIMELINE_GRID);
        assert_eq!(notes[0].duration_ticks, 240);
    }

    #[test]
    fn enter_with_no_cursor_or_in_an_empty_roll_adds_middle_c_at_tick_0() {
        let mut empty = Vec::new();
        assert_eq!(
            apply_command(&mut empty, None, RollCommand::Add, PATTERN_GRID),
            (Some(0), Outcome::Added)
        );
        assert_eq!(
            empty,
            vec![Note {
                pitch: 60,
                velocity: DEFAULT_NOTE_VELOCITY,
                start_tick: 0,
                duration_ticks: 480
            }]
        );
        let mut notes = vec![note_at(72, 960, 240)];
        assert_eq!(
            apply_command(&mut notes, None, RollCommand::Add, TIMELINE_GRID),
            (Some(1), Outcome::Added)
        );
        assert_eq!((notes[1].pitch, notes[1].start_tick), (60, 0));
    }

    #[test]
    fn enter_adds_one_beat_after_the_cursors_note_at_its_pitch() {
        let mut notes = vec![note_at(67, 240, 360)];
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::Add, TIMELINE_GRID),
            (Some(1), Outcome::Added)
        );
        assert_eq!(
            (notes[1].pitch, notes[1].start_tick, notes[1].duration_ticks),
            (67, 600, 480)
        );
    }

    #[test]
    fn enter_with_no_room_in_a_shared_pattern_adds_nothing() {
        let mut notes = vec![note_at(60, 1_440, 480)];
        assert_eq!(
            apply_command(&mut notes, Some(0), RollCommand::Add, PATTERN_GRID),
            (Some(0), Outcome::NoRoom)
        );
        assert_eq!(notes.len(), 1);
        let mut pattern = evil_project::Pattern {
            length_ticks: 1_920,
            notes,
            ..evil_project::Pattern::default()
        };
        apply_command(&mut pattern.notes, Some(0), RollCommand::Add, PATTERN_GRID);
        let mut project = Project::new("Room");
        project
            .add_pattern(pattern)
            .expect("the pattern still validates");
    }

    #[test]
    fn delete_moves_to_the_next_note_else_the_previous_else_none() {
        let mut notes = vec![
            note_at(60, 0, 120),
            note_at(62, 480, 120),
            note_at(64, 960, 120),
        ];
        let (cursor, outcome) =
            apply_command(&mut notes, Some(1), RollCommand::Delete, TIMELINE_GRID);
        assert_eq!(outcome, Outcome::Deleted(note_at(62, 480, 120)));
        assert_eq!(
            cursor.map(|index| notes[index]),
            Some(note_at(64, 960, 120))
        );
        let (cursor, _) = apply_command(&mut notes, cursor, RollCommand::Delete, TIMELINE_GRID);
        assert_eq!(cursor.map(|index| notes[index]), Some(note_at(60, 0, 120)));
        let (cursor, outcome) =
            apply_command(&mut notes, cursor, RollCommand::Delete, TIMELINE_GRID);
        assert_eq!(
            (cursor, outcome),
            (None, Outcome::Deleted(note_at(60, 0, 120)))
        );
        assert!(notes.is_empty());
        assert_eq!(
            apply_command(&mut notes, None, RollCommand::Delete, TIMELINE_GRID),
            (None, Outcome::Empty)
        );
        assert_eq!(
            apply_command(&mut notes, None, RollCommand::Next, TIMELINE_GRID),
            (None, Outcome::Empty)
        );
    }

    #[test]
    fn an_edit_with_no_cursor_says_so_and_changes_nothing() {
        let mut notes = vec![note_at(60, 0, 120)];
        for command in [
            RollCommand::PitchUp,
            RollCommand::Earlier,
            RollCommand::Shorter,
            RollCommand::Delete,
        ] {
            assert_eq!(
                apply_command(&mut notes, None, command, TIMELINE_GRID),
                (None, Outcome::NoSelection)
            );
        }
        assert_eq!(notes, vec![note_at(60, 0, 120)]);
    }

    #[test]
    fn the_cursor_keeps_its_note_through_clean_overlaps() {
        let mut track = Track {
            notes: vec![
                note_at(64, 960, 240),
                note_at(60, 0, 960),
                note_at(60, 480, 240),
                note_at(62, 480, 240),
            ],
            ..Track::default()
        };
        let before = track.notes.clone();
        evil_project::clean_overlaps(&mut track);
        assert_ne!(
            track.notes[1], before[1],
            "the selected note was re-sorted and shortened"
        );
        let found = relocate_cursor(&before, 1, &track.notes).expect("found again");
        assert_eq!(
            (track.notes[found].pitch, track.notes[found].start_tick),
            (60, 0)
        );
        let found = relocate_cursor(&before, 3, &track.notes).expect("found again");
        assert_eq!(track.notes[found], note_at(62, 480, 240));
    }

    #[test]
    fn the_cursor_keeps_its_note_when_the_list_is_replaced_and_resets_when_it_is_gone() {
        let before = vec![
            note_at(60, 0, 120),
            note_at(62, 480, 120),
            note_at(64, 960, 120),
        ];
        let replaced = vec![
            note_at(64, 960, 120),
            note_at(70, 10, 5),
            note_at(62, 480, 120),
        ];
        assert_eq!(relocate_cursor(&before, 1, &replaced), Some(2));
        let without = vec![note_at(60, 0, 120), note_at(64, 960, 120)];
        assert_eq!(relocate_cursor(&before, 1, &without), None);
        let mut edited_in_place = before.clone();
        edited_in_place[1].pitch = 63;
        assert_eq!(relocate_cursor(&before, 1, &edited_in_place), Some(1));
    }

    #[test]
    fn a_held_arrow_moves_once_per_press_even_within_one_frame() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut project = Project::new("Held");
        project.add_track(Track::default()).unwrap();
        project.tracks[0].notes = (0..6).map(|step| note_at(60, step * 480, 240)).collect();
        let mut state = RollState::default();
        let mut time = 0.0;
        let mut frame = |state: &mut RollState, events: Vec<egui::Event>| {
            time += 0.1;
            let _ = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        RollContext::new(&project),
                        &mut project.tracks[0],
                        state,
                        None,
                    );
                },
            );
        };
        frame(&mut state, vec![]);
        let roll = state.focus_id.expect("the roll was shown");
        context.memory_mut(|memory| memory.request_focus(roll));
        frame(&mut state, vec![]);
        frame(&mut state, vec![]);
        let right = |repeat| egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut state,
            vec![right(false), right(true), right(true), right(true)],
        );
        assert_eq!(state.selected_note, Some(3));
        assert!(
            state
                .exposed_name
                .starts_with("Piano roll, C 4, bar 1 beat 4"),
            "{}",
            state.exposed_name
        );
        assert!(
            context.memory(|memory| memory.has_focus(roll)),
            "arrows keep focus"
        );
    }

    #[test]
    fn black_keys_match_a_piano_keyboard() {
        assert!(!is_black_key(60));
        assert!(is_black_key(61));
        assert!(!is_black_key(62));
        assert!(is_black_key(63));
    }
}
