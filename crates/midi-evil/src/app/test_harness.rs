//! A whole application driven frame by frame for the keyboard tests, read back from the
//! accessibility tree egui builds.

use eframe::{App as _, egui::accesskit};

use super::*;

pub(super) const CONTROL: egui::Modifiers = egui::Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};

pub(super) struct Harness {
    pub(super) context: egui::Context,
    pub(super) app: StudioApp,
    pub(super) frame: eframe::Frame,
    pub(super) time: f64,
    pub(super) output: egui::FullOutput,
    /// The window's width in points; the narrowest the window allows is 900.
    pub(super) width: f32,
}

pub(super) fn note_at(pitch: u8, start_tick: u64, duration_ticks: u64) -> evil_project::Note {
    evil_project::Note {
        pitch,
        velocity: 100,
        start_tick,
        duration_ticks,
    }
}

pub(super) fn project_with(tracks: &[&[evil_project::Note]]) -> Project {
    let mut project = Project::new("Keys");
    for (index, notes) in tracks.iter().enumerate() {
        let name = format!("Track {}", index + 1);
        let instrument = project
            .add_instrument(Instrument::synth(&name, "lead"))
            .unwrap();
        project
            .add_track(Track {
                name,
                instrument,
                notes: notes.to_vec(),
                ..Track::default()
            })
            .unwrap();
    }
    project
}

pub(super) fn press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

pub(super) fn repeat(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: egui::Modifiers::NONE,
    }
}

pub(super) fn release(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

pub(super) fn plain(key: egui::Key) -> egui::Event {
    press(key, egui::Modifiers::NONE)
}

impl Harness {
    pub(super) fn new(project: Project) -> Self {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.project = project;
        app.reset_history();
        app.roll = RollState::default();
        app.view = View::Notes;
        app.show_about = false;
        let mut harness = Self {
            context,
            app,
            frame: eframe::Frame::_new_kittest(),
            time: 0.0,
            output: egui::FullOutput::default(),
            width: 1440.0,
        };
        harness.run(vec![]);
        harness.run(vec![]);
        harness
    }

    /// One frame. Every first press in it is released at the end of the frame, as a tap is:
    /// egui counts a press of a key still down as a repeat.
    pub(super) fn run(&mut self, mut events: Vec<egui::Event>) {
        let releases: Vec<_> = events
            .iter()
            .filter_map(|event| match *event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers,
                }),
                _ => None,
            })
            .collect();
        events.extend(releases);
        self.run_holding(events);
    }

    /// One frame whose keys stay down: the first press of a hold.
    pub(super) fn run_holding(&mut self, events: Vec<egui::Event>) {
        self.time += 0.1;
        let Self {
            context,
            app,
            frame,
            time,
            width,
            ..
        } = self;
        self.output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(*width, 900.0),
                )),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ui| app.ui(ui, frame),
        );
    }

    /// F6 until the roll has focus: three presses from nothing focused, fewer from a region.
    pub(super) fn focus_roll(&mut self) {
        for _ in 0..3 {
            if self.roll_focused() {
                break;
            }
            self.run(vec![plain(egui::Key::F6)]);
        }
        assert!(self.roll_focused(), "F6 reaches the roll");
    }

    pub(super) fn roll_focused(&self) -> bool {
        self.app.roll.widget_id.is_some() && self.focused() == self.app.roll.widget_id
    }

    pub(super) fn focused(&self) -> Option<egui::Id> {
        self.context.memory(|memory| memory.focused())
    }

    pub(super) fn cursor(&self) -> Option<usize> {
        let context =
            piano_roll::sequence_context(&self.app.project, &self.app.roll, self.app.document);
        self.app.roll.cursor.index(context)
    }

    pub(super) fn notes(&self, track: usize) -> &[evil_project::Note] {
        &self.app.project.tracks[track].notes
    }

    pub(super) fn nodes(&self) -> Vec<accesskit::Node> {
        self.output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter().map(|(_, node)| node.clone()))
            .collect()
    }

    pub(super) fn roll_name(&self) -> String {
        self.nodes()
            .into_iter()
            .find(|node| node.role() == accesskit::Role::Canvas)
            .and_then(|node| node.label().map(str::to_owned))
            .unwrap_or_default()
    }

    pub(super) fn announcements(&self) -> Vec<String> {
        self.nodes()
            .into_iter()
            .filter(|node| node.live() == Some(accesskit::Live::Polite))
            .filter_map(|node| node.value().map(str::to_owned))
            .collect()
    }

    /// A node of last frame's accessibility tree, by its name.
    pub(super) fn node(&self, label: &str) -> (accesskit::NodeId, accesskit::Node) {
        self.output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(_, node)| node.label() == Some(label))
            .cloned()
            .unwrap_or_else(|| panic!("no node named {label:?}"))
    }

    /// A node of last frame's accessibility tree, by its widget.
    pub(super) fn node_of(&self, widget: egui::Id) -> Option<accesskit::Node> {
        self.output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(id, _)| *id == widget.accesskit_id())
            .map(|(_, node)| node.clone())
    }

    pub(super) fn enabled(&self, label: &str) -> bool {
        !self.node(label).1.is_disabled()
    }

    pub(super) fn node_with_value(&self, value: f64) -> accesskit::NodeId {
        self.output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(_, node)| node.numeric_value() == Some(value))
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("no value box holding {value}"))
    }

    pub(super) fn action(
        &mut self,
        node: accesskit::NodeId,
        action: accesskit::Action,
        data: Option<accesskit::ActionData>,
    ) {
        self.run(vec![egui::Event::AccessKitActionRequest(
            accesskit::ActionRequest {
                action,
                target_tree: accesskit::TreeId::ROOT,
                target_node: node,
                data,
            },
        )]);
    }

    pub(super) fn click(&mut self, label: &str) {
        let node = self.node(label).0;
        self.action(node, accesskit::Action::Click, None);
    }

    pub(super) fn node_with_role(&self, role: accesskit::Role) -> accesskit::NodeId {
        self.output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .find(|(_, node)| node.role() == role)
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("no {role:?} node"))
    }

    /// Whether the transport started; stopped again at once, so a test never reaches audio.
    pub(super) fn transport_started_then_stopped(&mut self) -> bool {
        let started = self.app.render_job.is_some() || self.app.preview.is_some();
        self.app.stop_preview();
        started
    }
}
