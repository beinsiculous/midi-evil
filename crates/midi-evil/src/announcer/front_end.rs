//! The announcer's account for a front end that speaks by writing text. Natively an outcome goes
//! out in one frame and the name it changed in the next, because a screen reader given both at
//! once speaks only the name; a page that writes one string a frame must have both at once, and
//! the outcome has been consumed by the time the name is published. So the outcomes of an action
//! are kept here as the commands say them, paired at the frame's end with the focused control as
//! it now is — before any hold-back — and compared with what a listener was last told, so one
//! action is one string and the frame after it says nothing.
//!
//! A control an action changes is drawn as it was until the next pass: the button that undid the
//! last edit is still available, the part's button that merged the part is still there. In the
//! browser a pass with an outcome is therefore discarded and run again with no new input, and the
//! outcome is paired with the controls as the action left them — where focus now is, what the
//! control now says — so nothing is left to say a frame later for the same action.

use std::collections::HashMap;

use eframe::egui;

use crate::platform;
use crate::speech;
use crate::spoken::{FocusedControl, Happened};

/// What a listener was last told about the focused control.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Told {
    focus: Option<egui::Id>,
    name: String,
    value: String,
    states: Vec<String>,
}

/// The front end's part of one frame, before the application adds what only it knows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameWords {
    pub focused: Option<FocusedControl>,
    pub current: String,
    pub happened: Happened,
    pub announcement: Option<String>,
}

#[derive(Debug, Default)]
pub struct FrontEnd {
    /// What the commands of the action not yet told said, in the order they said it.
    outcomes: Vec<String>,
    /// The names the application's editors gave as current this frame, before any hold-back.
    current_names: HashMap<egui::Id, String>,
    told: Told,
    /// The field egui was editing text in when the last frame ended.
    editing: Option<egui::Id>,
    /// A field whose edit ended, holding its place while nothing has egui's focus, and the
    /// controls before and after it in Tab's walk.
    place_kept: Option<egui::Id>,
    kept_neighbours: (Option<egui::Id>, Option<egui::Id>),
    made: FrameWords,
    /// The keys of a pass discarded so its outcome waits for the controls it changed: the pass
    /// run again has no input of its own.
    deferred_keys: Option<FrameKeys>,
}

/// What a pass's input says about an edit: whether Enter or Escape was pressed, and whether text
/// was typed, pasted, cut or deleted.
#[derive(Debug, Default, Clone, Copy)]
struct FrameKeys {
    enter: bool,
    escape: bool,
    typed: bool,
}

impl FrontEnd {
    pub fn outcome(&mut self, text: String) {
        self.outcomes.push(text);
    }

    pub fn current_name(&mut self, widget: egui::Id, name: &str) {
        self.current_names.insert(widget, name.to_owned());
    }

    pub fn made(&self) -> &FrameWords {
        &self.made
    }

    /// Where Tab (`forwards`) or Shift+Tab goes from a field whose place is kept since its edit
    /// ended. egui cannot be asked: given the focus back for one frame, the field opens its edit
    /// again and takes the Tab's next stop for itself.
    pub fn next_stop_after_the_place_kept(&self, forwards: bool) -> Option<egui::Id> {
        self.place_kept?;
        if forwards {
            self.kept_neighbours.1
        } else {
            self.kept_neighbours.0
        }
    }

    /// Makes this frame's words. `holding` while a key repeats and `waiting` while a command's
    /// outcome waits for its focus move: neither says anything yet, and the outcomes wait with
    /// them.
    pub fn end_frame(&mut self, context: &egui::Context, holding: bool, waiting: bool) {
        let platform = platform::of(context);
        let keys = Self::keys_of(context);
        let keys = match self.deferred_keys.take() {
            Some(earlier) => FrameKeys {
                enter: earlier.enter || keys.enter,
                escape: earlier.escape || keys.escape,
                typed: earlier.typed || keys.typed,
            },
            None => keys,
        };
        if platform.pairs_an_outcome_with_the_controls_it_changed()
            && !self.outcomes.is_empty()
            && !holding
            && !waiting
            && context.current_pass_index() == 0
        {
            context.request_discard("an outcome waits for the controls its action changed");
            if context.will_discard() {
                self.deferred_keys = Some(keys);
                self.current_names.clear();
                return;
            }
        }
        // A control in a menu that closed this frame, or one not drawn at all, loses the focus
        // when the frame ends; told now, an outcome and the focus's move are one announcement,
        // not two in a row.
        let mut focus = context
            .memory(|memory| memory.focused())
            .filter(|widget| drawn(context, *widget) && !in_a_closed_menu(context, *widget));
        let editing_now = context.output(|output| output.ime.is_some());
        let FrameKeys {
            enter,
            escape,
            typed,
        } = keys;
        let ended = self
            .editing
            .filter(|field| focus != Some(*field) && drawn(context, *field) && (enter || escape));
        let committed = ended.filter(|_| enter);
        // egui leaves nothing focused when an edit ends. In the browser the field keeps its
        // place — it is what is current, and Tab moves on from it — without egui's focus, which
        // on a field is an open edit.
        if focus.is_some() {
            self.place_kept = None;
        } else if let Some(field) = ended.filter(|_| platform.keeps_the_place_of_an_ended_edit()) {
            self.place_kept = Some(field);
        }
        self.place_kept = self.place_kept.filter(|field| drawn(context, *field));
        self.kept_neighbours = match self.place_kept {
            Some(field) => {
                let order = crate::regions::tab_order(context);
                let place = order.iter().position(|widget| *widget == field);
                let after = |step: usize| place.map(|place| order[(place + step) % order.len()]);
                (after(order.len() - 1), after(1))
            }
            None => (None, None),
        };
        let editing_now = editing_now && focus.is_some();
        // A kept field shows its value as a control does, the suffix with it; nothing edits it
        // until it has the focus again, so the change of form is not a change to say.
        let on_a_kept_place = focus.is_none() && self.place_kept.is_some();
        focus = focus.or(self.place_kept);
        let focused = focus.and_then(|id| self.read(context, id, editing_now));
        self.editing = focus.filter(|_| editing_now);
        self.current_names.clear();

        let current = match &focused {
            Some(control) => speech::describe_control_in_full(
                &control.name,
                &control.role,
                &control.states,
                &control.value,
                &control.description,
            ),
            None => speech::nothing_focused(platform),
        };
        let now = Told {
            focus,
            name: focused
                .as_ref()
                .map(|control| control.name.clone())
                .unwrap_or_default(),
            value: focused
                .as_ref()
                .map(|control| control.value.clone())
                .unwrap_or_default(),
            states: focused
                .as_ref()
                .map(|control| control.states.clone())
                .unwrap_or_default(),
        };
        if holding || waiting {
            self.made = FrameWords {
                focused,
                current,
                ..FrameWords::default()
            };
            return;
        }
        let focus_moved = now.focus != self.told.focus;
        let name_changed = !focus_moved && now.name != self.told.name;
        let states_changed = !focus_moved && now.states != self.told.states;
        let value_changed =
            !focus_moved && now.value != self.told.value && !typed && !on_a_kept_place;
        let control_text = match &focused {
            Some(control) if committed.is_some() => {
                Some(speech::committed(&control.name, &control.value))
            }
            Some(control) if focus_moved => Some(speech::describe_control(
                &control.name,
                &control.role,
                &control.states,
                &control.value,
            )),
            None if focus_moved => Some(current.clone()),
            // A name, a state and a value that change together are all said, in that order.
            Some(control) => {
                let changed: Vec<String> = [
                    name_changed.then(|| control.name.clone()),
                    states_changed.then(|| control.states.join(", ")),
                    value_changed.then(|| control.value.clone()),
                ]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty())
                .collect();
                (!changed.is_empty()).then(|| changed.join(", "))
            }
            None => None,
        };
        let outcomes = std::mem::take(&mut self.outcomes);
        let announcement = speech::announcement(&outcomes, control_text);
        self.made = FrameWords {
            focused,
            current,
            happened: Happened {
                outcome: (!outcomes.is_empty()).then(|| outcomes.join(" ")),
                focus_moved,
                control_changed: name_changed || states_changed || value_changed,
                text_committed: committed.is_some(),
            },
            announcement,
        };
        self.told = now;
    }

    fn keys_of(context: &egui::Context) -> FrameKeys {
        context.input(|input| {
            let pressed = |wanted: egui::Key| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key, pressed: true, .. } if *key == wanted)
                })
            };
            FrameKeys {
                enter: pressed(egui::Key::Enter),
                escape: pressed(egui::Key::Escape),
                typed: input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Cut
                            | egui::Event::Ime(_)
                            | egui::Event::Key {
                                key: egui::Key::Backspace | egui::Key::Delete,
                                pressed: true,
                                ..
                            }
                    )
                }),
            }
        })
    }

    /// The focused control as its node holds it, with an editor's name as it now is rather than
    /// as it is held back for a desktop screen reader.
    fn read(
        &self,
        context: &egui::Context,
        widget: egui::Id,
        editing: bool,
    ) -> Option<FocusedControl> {
        let (node, labelled_by, value) = context.viewport(|viewport| {
            let state = viewport.this_pass.accesskit_state.as_ref()?;
            let node = state.nodes.get(&widget)?.clone();
            let labelled_by = labelled_by(&state.nodes, &node);
            let value = value(&state.nodes, &node);
            Some((node, labelled_by, value))
        })?;
        // The id egui's `Popup::default_response_id` gives a menu button's popup.
        let menu_open = egui::Popup::is_id_open(context, widget.with("popup"));
        let states = speech::state_words(&node, menu_open)
            .into_iter()
            .map(str::to_owned)
            .collect();
        Some(FocusedControl {
            id: widget,
            name: self
                .current_names
                .get(&widget)
                .cloned()
                .or_else(|| node.label().map(str::to_owned))
                .unwrap_or(labelled_by),
            description: node.description().unwrap_or_default().to_owned(),
            value,
            role: speech::role_word(&node).to_owned(),
            states,
            is_text_entry: editing,
            holds_selectable_text: matches!(
                node.role(),
                egui::accesskit::Role::TextInput | egui::accesskit::Role::MultilineTextInput
            ),
        })
    }
}

/// What a control holds, as it shows it: its text, or, for a slider, which keeps only a number,
/// the text of the value box drawn beside it — labelled by the same caption — or the number
/// itself when the slider is drawn without one.
fn value(nodes: &egui::IdMap<egui::accesskit::Node>, node: &egui::accesskit::Node) -> String {
    if let Some(text) = node.value() {
        return text.to_owned();
    }
    let Some(number) = node.numeric_value() else {
        return String::new();
    };
    let shown_beside = (!node.labelled_by().is_empty())
        .then(|| {
            nodes.values().find_map(|other| {
                (other.role() == egui::accesskit::Role::SpinButton
                    && other.labelled_by() == node.labelled_by())
                .then(|| other.value().map(str::to_owned))
                .flatten()
            })
        })
        .flatten();
    shown_beside.unwrap_or_else(|| speech::number(number))
}

/// The name a node takes from the nodes that label it, as a desktop screen reader is given it: a
/// text's own words, any other node's name, one after another — how a slider's value box is named
/// by the slider's caption.
fn labelled_by(nodes: &egui::IdMap<egui::accesskit::Node>, node: &egui::accesskit::Node) -> String {
    node.labelled_by()
        .iter()
        .filter_map(|wanted| {
            let (_, labelling) = nodes.iter().find(|(id, _)| id.accesskit_id() == *wanted)?;
            if labelling.role() == egui::accesskit::Role::Label {
                labelling.value()
            } else {
                labelling.label()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn in_a_closed_menu(context: &egui::Context, widget: egui::Id) -> bool {
    let layer = context.viewport(|viewport| {
        viewport
            .this_pass
            .widgets
            .get(widget)
            .map(|rect| rect.layer_id)
    });
    layer.is_some_and(|layer| {
        layer.order == egui::Order::Foreground && !egui::Popup::is_id_open(context, layer.id)
    })
}

fn drawn(context: &egui::Context, widget: egui::Id) -> bool {
    context.viewport(|viewport| viewport.this_pass.widgets.contains(widget))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_and_a_state_that_change_together_are_both_said() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut front_end = FrontEnd::default();
        let mut on = false;
        for (label, flip) in [("Loop", false), ("Loop", false), ("Loop all", true)] {
            if flip {
                on = !on;
            }
            let _ = context.run_ui(egui::RawInput::default(), |ui| {
                let toggle = ui.checkbox(&mut on, label).id;
                ui.memory_mut(|memory| memory.request_focus(toggle));
                front_end.end_frame(ui.ctx(), false, false);
            });
        }
        assert_eq!(
            front_end.made().announcement.as_deref(),
            Some("Loop all, checked")
        );
    }

    #[test]
    fn a_slider_drawn_without_its_number_says_the_number() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut front_end = FrontEnd::default();
        let mut level = 5.0;
        let mut slider = egui::Id::NULL;
        for _ in 0..2 {
            let _ = context.run_ui(egui::RawInput::default(), |ui| {
                slider = ui
                    .add(egui::Slider::new(&mut level, 0.0..=10.0).show_value(false))
                    .id;
                ui.memory_mut(|memory| memory.request_focus(slider));
                front_end.end_frame(ui.ctx(), false, false);
            });
        }
        let focused = front_end
            .made()
            .focused
            .clone()
            .expect("the slider has focus");
        assert_eq!(focused.value, "5");
    }
}
