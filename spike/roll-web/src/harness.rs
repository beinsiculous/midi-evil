//! The harness-side state struct and its one frame: the transport readout in a top panel, the
//! roll in the central one, F6 between the two regions, and the bridge written at the end of the
//! pass.

use eframe::egui;
use evil_project::{EventSequence, Project};

use crate::bridge::{Bridge, NOTHING_FOCUSED};
use crate::control_text;
use crate::fixture::Loaded;
use crate::focus_boundary::{self, Edges, FocusBoundary};
use crate::piano_roll::{self, RollContext, RollState};
use crate::theme;
use crate::transport_readout::{self, PlayingState, TransportReadout, TransportState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focused {
    Nothing,
    Roll,
    Readout,
    Other(egui::Id),
}

/// The regions F6 moves between, in the native application's order (it has a side panel between
/// the two, which the harness does not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Toolbar,
    Editor,
}

pub struct Harness {
    project: Project,
    roll: RollState,
    readout: TransportReadout,
    readout_id: Option<egui::Id>,
    /// `?edge=value`: a value box as egui's first stop, so leaving the canvas backwards from the
    /// text entry can be tested.
    edge_value: Option<f32>,
    edge_value_id: Option<egui::Id>,
    bridge: Bridge,
    boundary: FocusBoundary,
    focused: Focused,
    /// What egui last said about the standard control that has focus: `FocusGained` is raised
    /// once, in the frame focus arrives, and later frames may raise nothing.
    other_info: Option<egui::WidgetInfo>,
    /// The widget that had focus at the end of the last pass, given focus back if egui drops it.
    last_focus: Option<egui::Id>,
    /// What egui asked of the text entry in the last pass.
    last_text_input: Option<egui::output::IMEOutput>,
    /// A widget egui dropped focus from, to be given it back at the start of the next pass.
    give_back: Option<egui::Id>,
    gave_back: bool,
    /// What has been typed into the control being edited, until the edit ends.
    typed_text: Option<String>,
    initial_focus_given: bool,
    revision: u64,
    /// Every key press egui has received, for the browser-key table.
    keys_received: u64,
    last_key: String,
}

impl Harness {
    pub fn new(loaded: Loaded, edge_value: bool, bridge: Bridge, boundary: FocusBoundary) -> Self {
        let mut roll = RollState::default();
        roll.select_part(0, loaded.part);
        Self {
            project: loaded.project,
            roll,
            readout: TransportReadout::default(),
            readout_id: None,
            edge_value: edge_value.then_some(1.0),
            edge_value_id: None,
            bridge,
            boundary,
            focused: Focused::Nothing,
            other_info: None,
            last_focus: None,
            last_text_input: None,
            give_back: None,
            gave_back: false,
            typed_text: None,
            initial_focus_given: false,
            revision: 0,
            keys_received: 0,
            last_key: String::new(),
        }
    }

    fn transport(&self) -> TransportState {
        TransportState {
            playing: PlayingState::Stopped,
            tempo_beats_per_minute: self.project.tempo_bpm,
            position_milliseconds: 0,
        }
    }

    fn region_of(&self, focused: Focused) -> Option<Region> {
        match focused {
            Focused::Nothing => None,
            Focused::Readout => Some(Region::Toolbar),
            Focused::Other(id) if Some(id) == self.edge_value_id => Some(Region::Toolbar),
            Focused::Roll | Focused::Other(_) => Some(Region::Editor),
        }
    }

    /// F6 and Shift+F6 move between the regions as the native ones do: to the next or previous
    /// region from wherever focus is in one, and with nothing focused, forwards to the toolbar
    /// and backwards to the editor. A region's stop is the readout or the roll.
    fn region_keys(&self, context: &egui::Context) {
        let backwards =
            context.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F6));
        let forwards = !backwards
            && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F6));
        if !backwards && !forwards {
            return;
        }
        let regions = [Region::Toolbar, Region::Editor];
        let current = self
            .region_of(self.classify(context.memory(|memory| memory.focused())))
            .and_then(|region| regions.iter().position(|candidate| *candidate == region));
        let next = match (current, forwards) {
            (Some(place), true) => (place + 1) % regions.len(),
            (Some(place), false) => (place + regions.len() - 1) % regions.len(),
            (None, true) => 0,
            (None, false) => regions.len() - 1,
        };
        let target = match regions[next] {
            Region::Toolbar => self.readout_id,
            Region::Editor => self.roll.focus_id,
        };
        if let Some(target) = target {
            context.memory_mut(|memory| memory.request_focus(target));
        }
    }

    /// Read from the frame's raw input, before anything consumes a key.
    fn count_keys(&mut self, context: &egui::Context) {
        let pressed: Vec<String> = context.input(|input| {
            input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some(chord_name(*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        if let Some(last) = pressed.last() {
            self.keys_received += pressed.len() as u64;
            self.last_key = last.clone();
        }
    }

    fn classify(&self, focused: Option<egui::Id>) -> Focused {
        match focused {
            None => Focused::Nothing,
            Some(id) if Some(id) == self.roll.focus_id => Focused::Roll,
            Some(id) if Some(id) == self.readout_id => Focused::Readout,
            Some(id) => Focused::Other(id),
        }
    }

    /// egui drops focus on Escape and when a text edit is committed with Enter, which would leave
    /// a keyboard user on nothing. The control that had it gets it back at the start of the next
    /// pass, before it is drawn, so egui raises `FocusGained` for it with its value as it now is —
    /// that is what announces a committed value. Returns true for the pass in which focus was
    /// dropped: nothing is published for it, and eframe's text entry is asked to stay focused
    /// through it, so the page's focus does not bounce to the canvas and back.
    fn focus_dropped(&mut self, context: &egui::Context) -> bool {
        let focused = context.memory(|memory| memory.focused());
        let Some(previous) = self.last_focus.filter(|previous| {
            focused.is_none()
                && context.viewport(|viewport| viewport.this_pass.widgets.contains(*previous))
        }) else {
            self.last_focus = focused;
            self.last_text_input = context.output(|output| output.ime);
            return false;
        };
        // A value box keeps its edited text under its id; without it, it shows its value.
        context.data_mut(|data| data.remove::<String>(previous));
        self.give_back = Some(previous);
        let text_input = self.last_text_input;
        context.output_mut(|output| output.ime = output.ime.or(text_input));
        context.request_repaint();
        true
    }

    fn give_focus_back(&mut self, context: &egui::Context) {
        if let Some(previous) = self.give_back.take()
            && context.memory(|memory| memory.focused()).is_none()
        {
            context.memory_mut(|memory| memory.request_focus(previous));
            self.gave_back = true;
        }
    }

    /// Mechanism 1: what egui focused, described in D5's strings, into the page's elements.
    fn publish(&mut self, context: &egui::Context, transport: &TransportState) {
        if self.focus_dropped(context) {
            return;
        }
        let focused = self.classify(self.last_focus);
        // Focus given back is announced as arriving, so Enter and Escape say where they left it.
        let focus_moved = focused != self.focused || std::mem::take(&mut self.gave_back);
        self.focused = focused;
        // The pass's events are still in the context here: the runner takes them only when the
        // pass ends, after this function returns, and then discards them.
        let events: Vec<(bool, egui::WidgetInfo)> = context.output(|output| {
            output
                .events
                .iter()
                .map(|event| {
                    (
                        matches!(event, egui::output::OutputEvent::FocusGained(_)),
                        event.widget_info().clone(),
                    )
                })
                .collect()
        });
        let editing_text = context.output(|output| output.ime.is_some());
        // While a value box or text field is being typed into, its own description still holds
        // the value from before the edit; what was typed is in the text edit's event. The screen
        // reader echoes the keys, so typing updates the current element without announcing it.
        let typed = events.iter().rev().find_map(|(_, info)| {
            (info.typ == egui::WidgetType::TextEdit
                && info.prev_text_value != info.current_text_value)
                .then(|| info.current_text_value.clone())
                .flatten()
        });
        let editing_other = matches!(focused, Focused::Other(_)) && editing_text;
        if focus_moved || !editing_other {
            self.typed_text = None;
        }
        let typing = editing_other && !focus_moved && typed.is_some();
        if typing {
            self.typed_text = typed;
        }
        if let Focused::Other(_) = focused {
            if focus_moved {
                // Only the arriving control's own `FocusGained` describes it: an event from any
                // other widget in the pass is not borrowed, and a control with none is unnamed.
                // A value box raises one for its text edit and one for itself; the last is its own.
                self.other_info = events
                    .iter()
                    .rev()
                    .find(|(gained, _)| *gained)
                    .map(|(_, info)| info.clone());
            } else if !typing && let Some((_, info)) = events.last() {
                self.other_info = Some(info.clone());
            }
        }
        let (current, notice) = match focused {
            Focused::Roll => (
                self.roll.current_name.clone(),
                self.roll.notice_this_frame.clone(),
            ),
            Focused::Readout => (transport_readout::describe(transport), None),
            Focused::Other(_) => match &self.typed_text {
                Some(typed_text) => (
                    format!(
                        "{}, editing {typed_text}",
                        control_text::name(self.other_info.as_ref())
                    ),
                    None,
                ),
                None => (control_text::description(self.other_info.as_ref()), None),
            },
            Focused::Nothing => (NOTHING_FOCUSED.to_owned(), None),
        };
        // egui asks for text input while a text edit has focus, and eframe then moves the page's
        // focus to its text entry, after this pass: the entry is named and exposed first.
        let entry_name = match focused {
            Focused::Other(_) if editing_text => Some(control_text::name(self.other_info.as_ref())),
            _ => None,
        };
        self.bridge.set_text_entry(entry_name);
        let page_focused = context.input(|input| input.focused);
        self.bridge
            .publish(current, notice, focus_moved, page_focused && !typing);

        let (first, last) = focus_boundary::tab_ends(context);
        let egui_focus = context.memory(|memory| memory.focused());
        let edges = Edges {
            at_first: egui_focus.is_some() && egui_focus == first,
            at_last: egui_focus.is_some() && egui_focus == last,
        };
        self.boundary.set(edges);
        self.publish_probe_state(focused, edges);
    }

    fn publish_probe_state(&mut self, focused: Focused, edges: Edges) {
        let notes = self.shown_notes();
        let cursor_place = self.roll.selected_note.and_then(|index| {
            piano_roll::walk_order(notes)
                .iter()
                .position(|walked| *walked == index)
        });
        let note_count = notes.len();
        let valid = self.project.validate().is_ok();
        let focus_name = match focused {
            Focused::Nothing => "none",
            Focused::Roll => "roll",
            Focused::Readout => "readout",
            Focused::Other(_) => "other",
        };
        let edge_name = match (edges.at_first, edges.at_last) {
            (true, true) => "both",
            (true, false) => "first",
            (false, true) => "last",
            (false, false) => "none",
        };
        self.bridge
            .set_attribute("data-egui-focus", focus_name.to_owned());
        self.bridge
            .set_attribute("data-focus-edge", edge_name.to_owned());
        self.bridge.set_attribute(
            "data-cursor-place",
            cursor_place.map_or_else(|| "none".to_owned(), |place| place.to_string()),
        );
        self.bridge
            .set_attribute("data-note-count", note_count.to_string());
        self.bridge
            .set_attribute("data-revision", self.revision.to_string());
        self.bridge
            .set_attribute("data-project-valid", valid.to_string());
        self.bridge
            .set_attribute("data-keys-received", self.keys_received.to_string());
        self.bridge
            .set_attribute("data-last-key", self.last_key.clone());
    }

    fn shown_pattern(&self) -> Option<usize> {
        let track = self.project.tracks.get(self.roll.selected_track)?;
        let part = self.roll.selected_part?;
        track.parts.get(part).map(|part| part.pattern)
    }

    fn shown_notes(&self) -> &[evil_project::Note] {
        match self.shown_pattern() {
            Some(pattern) => &self.project.patterns[pattern].notes,
            None => &self.project.tracks[self.roll.selected_track].notes,
        }
    }
}

fn chord_name(key: egui::Key, modifiers: egui::Modifiers) -> String {
    let mut name = String::new();
    if modifiers.ctrl {
        name.push_str("Control+");
    }
    if modifiers.alt {
        name.push_str("Alt+");
    }
    if modifiers.shift {
        name.push_str("Shift+");
    }
    if modifiers.mac_cmd {
        name.push_str("Meta+");
    }
    name.push_str(key.name());
    name
}

impl eframe::App for Harness {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = root.ctx().clone();
        self.count_keys(&context);
        self.give_focus_back(&context);
        self.region_keys(&context);
        let transport = self.transport();

        egui::Panel::top("transport")
            .frame(theme::panel().inner_margin(egui::Margin::symmetric(14, 10)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    if let Some(value) = &mut self.edge_value {
                        let response = ui.add(egui::DragValue::new(value).range(0.0..=10.0));
                        self.edge_value_id = Some(response.id);
                    }
                    theme::label(ui, "MIDI EVIL · WEB SPIKE");
                    let readout = self.readout.show(ui, &transport);
                    self.readout_id = Some(readout.id);
                });
            });

        let pattern = self.shown_pattern();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(16))
            .show(root, |ui| {
                let mut roll_context = RollContext::new(&self.project);
                roll_context.sharing_parts = pattern.map(|pattern| {
                    self.project
                        .tracks
                        .iter()
                        .flat_map(|track| &track.parts)
                        .filter(|part| part.pattern == pattern)
                        .count()
                });
                let pattern_length = pattern.map(|index| self.project.patterns[index].length_ticks);
                let sequence: &mut dyn EventSequence = match pattern {
                    Some(index) => &mut self.project.patterns[index],
                    None => &mut self.project.tracks[self.roll.selected_track],
                };
                if piano_roll::show(ui, roll_context, sequence, &mut self.roll, pattern_length) {
                    self.revision += 1;
                }
            });

        // The roll is where a visitor lands the first time the canvas takes focus, as the
        // native gate's F6 lands there; after that, egui keeps whatever had focus while the page
        // has it elsewhere, so returning finds the cursor where it was.
        if !self.initial_focus_given
            && let Some(roll) = self.roll.focus_id
        {
            context.memory_mut(|memory| memory.request_focus(roll));
            self.initial_focus_given = true;
        }

        self.publish(&context, &transport);
    }
}
