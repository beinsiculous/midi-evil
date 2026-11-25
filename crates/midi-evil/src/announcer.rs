//! The two channels a screen reader hears, and the rule between them, so no widget hand-rolls
//! it. A focused widget's accessible *name* is state: what it is on now. An *outcome* — what a
//! key did when the state alone does not say it — goes to a polite live node added for one
//! frame. Orca speaks a focused name's change on every key, and speaks the live node only when
//! no name changes in the same frame, so an outcome goes out first and the name follows in the
//! next frame. A held key would make the name change on every repeat — a queue still reading an
//! old note when the person acts on a new one — so while a key repeats the name rests, and the
//! note it came to rest on is published once when the hold ends.

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, accesskit};

#[derive(Debug)]
struct Hold {
    key: egui::Key,
    /// The last outcome announced since the key went down; an identical one is not repeated.
    last_outcome: Option<String>,
}

#[derive(Debug, Default)]
pub struct Announcer {
    published_names: HashMap<egui::Id, String>,
    /// The outcome to announce at the end of this frame.
    pending_outcome: Option<String>,
    /// The outcome of the latest key press that was not a repeat, so a hold that follows it
    /// does not say it again.
    last_press_outcome: Option<String>,
    hold: Option<Hold>,
    /// A name held back this frame, for an outcome or a hold, waits for a frame of its own.
    name_waiting: bool,
    announcements_made: u64,
    /// The readouts read on demand that were focused last frame.
    focused_readouts: HashSet<egui::Id>,
}

impl Announcer {
    /// Every key event of the frame, in order, so a hold ends on the key's release or on any
    /// press that is not a repeat. A release in another window never arrives, hence the latter.
    pub fn observe_key(&mut self, key: egui::Key, pressed: bool, repeat: bool) {
        match (pressed, repeat) {
            (true, true) => {
                if self.hold.as_ref().is_none_or(|hold| hold.key != key) {
                    self.hold = Some(Hold {
                        key,
                        last_outcome: self.last_press_outcome.clone(),
                    });
                }
            }
            (true, false) => {
                self.end_hold();
                self.last_press_outcome = None;
            }
            (false, _) => {
                if self.hold.as_ref().is_some_and(|hold| hold.key == key) {
                    self.end_hold();
                }
            }
        }
    }

    /// The focused widget lost focus: whatever it was holding has ended.
    pub fn focus_lost(&mut self) {
        self.end_hold();
    }

    pub fn is_holding(&self) -> bool {
        self.hold.is_some()
    }

    fn end_hold(&mut self) {
        if self.hold.take().is_some() {
            self.name_waiting = true;
        }
    }

    /// Queues an outcome for the polite live node. Within one hold, the outcome just announced
    /// is not announced again.
    pub fn announce(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(hold) = &mut self.hold {
            if hold.last_outcome.as_deref() == Some(text.as_str()) {
                return;
            }
            hold.last_outcome = Some(text.clone());
        } else {
            self.last_press_outcome = Some(text.clone());
        }
        self.pending_outcome = Some(match self.pending_outcome.take() {
            Some(earlier) if earlier != text => format!("{earlier} {text}"),
            _ => text,
        });
    }

    /// The name to expose for a focused widget this frame, given what it is on now. It stays as
    /// last published while an outcome goes out this frame or a key is held.
    pub fn focused_name(&mut self, widget: egui::Id, current: String) -> String {
        let published = self.published_names.get(&widget);
        let hold_back =
            published.is_some() && (self.pending_outcome.is_some() || self.is_holding());
        if hold_back {
            // A hold's end asks for its own frame; asking on every held frame would repaint
            // without pause until the key came up.
            if !self.is_holding() && published.is_some_and(|published| *published != current) {
                self.name_waiting = true;
            }
        } else {
            self.published_names.insert(widget, current);
        }
        self.published_names
            .get(&widget)
            .cloned()
            .unwrap_or_default()
    }

    /// The name of a readout that is read on demand, the transport's, whose state changes by
    /// itself. A focused node whose name changes on a timer is spoken at every change, so while
    /// the readout is focused its name rests at what was last read; while it is not, the name
    /// follows `current` silently, and focus arriving finds it true. `read` is the person asking
    /// again: the name is brought up to date, and that change is what is heard — or, when
    /// nothing changed and an unchanged name would say nothing, or the readout is not the focus
    /// and its name change would be passed over, the reading is announced.
    pub fn on_demand_name(
        &mut self,
        widget: egui::Id,
        focused: bool,
        current: String,
        read: bool,
    ) -> String {
        let focus_arrived = if focused {
            self.focused_readouts.insert(widget)
        } else {
            self.focused_readouts.remove(&widget);
            false
        };
        let published = self.published_names.get(&widget);
        let unpublished = published.is_none();
        if read && (published == Some(&current) || !focused) {
            self.announce(current.clone());
        }
        if !focused || focus_arrived || read || unpublished {
            self.published_names.insert(widget, current);
        }
        self.published_names
            .get(&widget)
            .cloned()
            .unwrap_or_default()
    }

    /// Adds this frame's live node, if an outcome is pending, and asks for the next frame when a
    /// name is waiting for it: in egui's reactive mode that frame otherwise may not come.
    pub fn end_frame(&mut self, context: &egui::Context) {
        if let Some(text) = self.pending_outcome.take() {
            self.announcements_made += 1;
            // A fresh node per announcement: adding a live node is what raises the event, and
            // an identical text on a node that stayed would raise nothing.
            let node_id = egui::Id::new("announcement").with(self.announcements_made);
            context.accesskit_node_builder(node_id, |node| {
                node.set_role(accesskit::Role::Label);
                node.set_value(text);
                node.set_live(accesskit::Live::Polite);
            });
            self.name_waiting = true;
        }
        if std::mem::take(&mut self.name_waiting) {
            context.request_repaint();
        }
    }

    #[cfg(test)]
    pub fn take_pending_outcome(&mut self) -> Option<String> {
        self.pending_outcome.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget() -> egui::Id {
        egui::Id::new("roll")
    }

    #[test]
    fn an_outcome_goes_out_first_and_the_name_follows_in_the_next_frame() {
        let context = egui::Context::default();
        let mut announcer = Announcer::default();
        assert_eq!(announcer.focused_name(widget(), "first".into()), "first");
        announcer.observe_key(egui::Key::Delete, true, false);
        announcer.announce("Removed.");
        assert_eq!(announcer.focused_name(widget(), "second".into()), "first");
        announcer.end_frame(&context);
        assert_eq!(announcer.focused_name(widget(), "second".into()), "second");
    }

    #[test]
    fn a_held_key_publishes_its_name_once_when_the_hold_ends() {
        let mut announcer = Announcer::default();
        let right = egui::Key::ArrowRight;
        announcer.observe_key(right, true, false);
        assert_eq!(announcer.focused_name(widget(), "note 1".into()), "note 1");
        for note in 2..=20 {
            announcer.observe_key(right, true, true);
            assert_eq!(
                announcer.focused_name(widget(), format!("note {note}")),
                "note 1"
            );
        }
        announcer.observe_key(right, false, false);
        assert_eq!(
            announcer.focused_name(widget(), "note 20".into()),
            "note 20"
        );
    }

    #[test]
    fn a_hold_ends_on_focus_loss_or_any_press_that_is_not_a_repeat() {
        let mut announcer = Announcer::default();
        announcer.observe_key(egui::Key::ArrowRight, true, true);
        assert!(announcer.is_holding());
        announcer.focus_lost();
        assert!(!announcer.is_holding());
        announcer.observe_key(egui::Key::ArrowRight, true, true);
        announcer.observe_key(egui::Key::ArrowLeft, false, false);
        assert!(
            announcer.is_holding(),
            "another key's release is not this hold's end"
        );
        announcer.observe_key(egui::Key::Delete, true, false);
        assert!(!announcer.is_holding());
    }

    #[test]
    fn an_outcome_repeated_within_one_hold_is_announced_once_and_again_after_a_release() {
        let mut announcer = Announcer::default();
        let up = egui::Key::ArrowUp;
        announcer.observe_key(up, true, false);
        announcer.announce("Cannot go higher.");
        assert_eq!(
            announcer.take_pending_outcome().as_deref(),
            Some("Cannot go higher.")
        );
        for _ in 0..5 {
            announcer.observe_key(up, true, true);
            announcer.announce("Cannot go higher.");
            assert_eq!(announcer.take_pending_outcome(), None);
        }
        announcer.observe_key(up, false, false);
        announcer.observe_key(up, true, false);
        announcer.announce("Cannot go higher.");
        assert_eq!(
            announcer.take_pending_outcome().as_deref(),
            Some("Cannot go higher.")
        );
    }

    #[test]
    fn a_read_of_a_readout_that_is_not_the_focus_is_announced() {
        let mut announcer = Announcer::default();
        let readout = egui::Id::new("readout");
        announcer.on_demand_name(readout, false, "at 1".to_owned(), false);
        let name = announcer.on_demand_name(readout, false, "at 2".to_owned(), true);
        assert_eq!(name, "at 2");
        assert_eq!(
            announcer.take_pending_outcome().as_deref(),
            Some("at 2"),
            "a name change on a node that is not the focus is passed over, so the read is spoken"
        );
    }

    #[test]
    fn a_readout_follows_its_state_unfocused_rests_focused_and_is_brought_up_to_date_on_a_read() {
        let mut announcer = Announcer::default();
        let readout = egui::Id::new("readout");
        let name = |announcer: &mut Announcer, focused, current: &str, read| {
            announcer.on_demand_name(readout, focused, current.to_owned(), read)
        };
        assert_eq!(name(&mut announcer, false, "at 1", false), "at 1");
        assert_eq!(name(&mut announcer, false, "at 2", false), "at 2");
        assert_eq!(
            name(&mut announcer, true, "at 3", false),
            "at 3",
            "focus arriving finds it true"
        );
        for later in ["at 4", "at 5", "at 6"] {
            assert_eq!(name(&mut announcer, true, later, false), "at 3");
        }
        assert_eq!(announcer.take_pending_outcome(), None);
        assert_eq!(name(&mut announcer, true, "at 7", true), "at 7");
        assert_eq!(
            announcer.take_pending_outcome(),
            None,
            "the name's change is the reading"
        );
        assert_eq!(name(&mut announcer, true, "at 7", true), "at 7");
        assert_eq!(
            announcer.take_pending_outcome().as_deref(),
            Some("at 7"),
            "a reading that changes nothing is announced"
        );
        assert_eq!(name(&mut announcer, false, "at 8", false), "at 8");
    }

    #[test]
    fn a_name_held_back_asks_for_another_frame() {
        let context = egui::Context::default();
        let mut announcer = Announcer::default();
        announcer.focused_name(widget(), "before".into());
        announcer.announce("Added.");
        announcer.focused_name(widget(), "after".into());
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            announcer.end_frame(ui.ctx());
        });
        assert!(
            output
                .viewport_output
                .values()
                .any(|viewport| viewport.repaint_delay.is_zero()),
            "the waiting name must get its frame"
        );
    }
}
