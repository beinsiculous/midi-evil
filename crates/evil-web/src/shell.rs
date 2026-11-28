//! The workstation, as eframe runs it in a page: the application's own frame, then what the frame
//! says, written into the page.

use std::rc::Rc;

use eframe::egui;

use crate::bridge::Bridge;
use crate::keyboard::{Entry, Shared};

pub struct Shell {
    app: midi_evil::StudioApp,
    bridge: Bridge,
    shared: Rc<Shared>,
    /// Tab's first and last stops as the last frame made them, for focus arriving from the page.
    stops: (Option<egui::Id>, Option<egui::Id>),
}

impl Shell {
    pub fn new(app: midi_evil::StudioApp, bridge: Bridge, shared: Rc<Shared>) -> Self {
        Self {
            app,
            bridge,
            shared,
            stops: (None, None),
        }
    }

    fn publish(&mut self, frame: &midi_evil::SpokenFrame, entered: bool) {
        let editing = frame
            .focused
            .as_ref()
            .filter(|control| control.is_text_entry);
        self.bridge
            .set_text_entry(editing.map(|control| control.name.as_str()));
        self.bridge.publish(
            &frame.current,
            frame.announcement.as_deref(),
            entered,
            frame.happened.outcome.is_some(),
        );
        let edge = match (frame.at_first_stop, frame.at_last_stop) {
            (true, true) => "both",
            (true, false) => "first",
            (false, true) => "last",
            (false, false) => "none",
        };
        self.bridge
            .set_attribute("data-focus-edge", edge.to_owned());
        self.bridge
            .set_attribute("data-unsaved", frame.unsaved.to_string());
        self.bridge
            .set_attribute("data-editing-text", editing.is_some().to_string());
    }
}

impl eframe::App for Shell {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let context = root.ctx().clone();
        let entering = self.shared.entering.take();
        let target = match entering {
            Some(Entry::First) => self.stops.0,
            Some(Entry::Last) => self.stops.1,
            None => None,
        };
        if let Some(target) = target {
            context.memory_mut(|memory| memory.request_focus(target));
        }
        eframe::App::ui(&mut self.app, root, frame);
        // A pass the workstation discards is run again at once with no new input; the page is
        // told the pass that stands, and the entry it was making waits for it.
        if context.will_discard() {
            self.shared.entering.set(entering);
            return;
        }
        let spoken = self.app.spoken_frame(&context);
        self.stops = (spoken.first_stop, spoken.last_stop);
        self.shared.frame_finished(&spoken);
        self.publish(&spoken, entering.is_some());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::App::save(&mut self.app, storage);
    }
}
