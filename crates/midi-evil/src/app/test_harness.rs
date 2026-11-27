//! A whole application driven frame by frame for the keyboard tests, read back from the
//! accessibility tree egui builds, and the scripted dialogs it answers its dialogs with.

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use eframe::{App as _, egui::accesskit};

use super::*;

/// Which dialog was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DialogKind {
    PickFile,
    PickFiles,
    PickFolder,
    SaveFile,
    Confirm,
}

/// What one dialog was asked, as the scripted dialogs record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Asked {
    pub(super) kind: DialogKind,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) filters: Vec<(String, Vec<String>)>,
    pub(super) directory: Option<PathBuf>,
    pub(super) file_name: Option<String>,
}

impl Asked {
    /// A file dialog with no title, asked with one filter.
    pub(super) fn file(
        kind: DialogKind,
        filter: (&str, &[&str]),
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Self {
        let (name, extensions) = filter;
        Self {
            kind,
            title: None,
            description: None,
            filters: vec![(
                name.to_owned(),
                extensions
                    .iter()
                    .map(|extension| (*extension).to_owned())
                    .collect(),
            )],
            directory: directory.map(Path::to_path_buf),
            file_name: file_name.map(str::to_owned),
        }
    }

    pub(super) fn confirm(title: &str, description: &str) -> Self {
        Self {
            kind: DialogKind::Confirm,
            title: Some(title.to_owned()),
            description: Some(description.to_owned()),
            filters: Vec::new(),
            directory: None,
            file_name: None,
        }
    }
}

/// The answer a test queues for the next dialog asked.
#[derive(Debug)]
pub(super) enum Answer {
    Paths(Vec<PathBuf>),
    Nothing,
    Yes,
    No,
}

#[derive(Default)]
struct Script {
    answers: VecDeque<Answer>,
    asked: Vec<Asked>,
}

/// Dialogs a test answers: every request is recorded, and each takes the next queued answer. A
/// dialog asked with nothing queued fails the test, where a real one would have hung it.
#[derive(Clone, Default)]
pub(super) struct ScriptedDialogs(Rc<RefCell<Script>>);

impl ScriptedDialogs {
    pub(super) fn answer(&self, answer: Answer) {
        self.0.borrow_mut().answers.push_back(answer);
    }

    /// Every request since the last call, oldest first.
    pub(super) fn take_asked(&self) -> Vec<Asked> {
        std::mem::take(&mut self.0.borrow_mut().asked)
    }

    fn ask(&self, asked: Asked) -> Answer {
        let mut script = self.0.borrow_mut();
        let answer = script
            .answers
            .pop_front()
            .unwrap_or_else(|| panic!("a dialog was asked with no answer queued: {asked:?}"));
        script.asked.push(asked);
        answer
    }

    fn ask_for_paths(
        &self,
        kind: DialogKind,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>> {
        let asked = Asked {
            kind,
            title: None,
            description: None,
            filters: filters.to_vec(),
            directory: directory.map(Path::to_path_buf),
            file_name: file_name.map(str::to_owned),
        };
        match self.ask(asked) {
            Answer::Paths(paths) => Some(paths),
            Answer::Nothing => None,
            answer => panic!("a file dialog was answered {answer:?}"),
        }
    }
}

impl crate::dialogs::Dialogs for ScriptedDialogs {
    fn pick_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        assert_eq!(title, None, "no dialog is asked with a title");
        self.ask_for_paths(DialogKind::PickFile, filters, directory, file_name)
            .and_then(|paths| paths.into_iter().next())
    }

    fn pick_files(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>> {
        assert_eq!(title, None, "no dialog is asked with a title");
        self.ask_for_paths(DialogKind::PickFiles, filters, directory, file_name)
    }

    fn pick_folder(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        assert_eq!(title, None, "no dialog is asked with a title");
        self.ask_for_paths(DialogKind::PickFolder, filters, directory, file_name)
            .and_then(|paths| paths.into_iter().next())
    }

    fn save_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        assert_eq!(title, None, "no dialog is asked with a title");
        self.ask_for_paths(DialogKind::SaveFile, filters, directory, file_name)
            .and_then(|paths| paths.into_iter().next())
    }

    fn confirm(&mut self, title: &str, description: &str) -> bool {
        match self.ask(Asked::confirm(title, description)) {
            Answer::Yes => true,
            Answer::No => false,
            answer => panic!("a confirm was answered {answer:?}"),
        }
    }
}

impl StudioApp {
    /// An application whose dialogs fail the test if one is asked; `Harness` answers them.
    pub(super) fn create(
        context: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        options: LaunchOptions,
        instance: Option<SingleInstance>,
    ) -> Self {
        let dialogs = Box::new(ScriptedDialogs::default());
        Self::with_dialogs(context, storage, options, instance, dialogs)
    }
}

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
    /// The application's dialogs: what each was asked, and the answers queued for the next.
    pub(super) dialogs: ScriptedDialogs,
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
        let dialogs = ScriptedDialogs::default();
        let mut app = StudioApp::create(&context, None, LaunchOptions::default(), None);
        app.dialogs = Box::new(dialogs.clone());
        app.project = project;
        app.reset_history();
        app.roll = RollState::default();
        app.select_view(View::Notes);
        app.show_about = false;
        let mut harness = Self {
            context,
            app,
            frame: eframe::Frame::_new_kittest(),
            time: 0.0,
            output: egui::FullOutput::default(),
            width: 1440.0,
            dialogs,
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

    /// A key, then the frame its name waits for.
    pub(super) fn key(&mut self, event: egui::Event) {
        self.run(vec![event]);
        self.run(vec![]);
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

    /// Clicks the one node with this name. A name two nodes carry is refused: which of them
    /// the tree lists first is not something a test may lean on.
    pub(super) fn click(&mut self, label: &str) {
        let carrying = self
            .output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .filter(|(_, node)| node.label() == Some(label))
            .count();
        assert!(carrying <= 1, "{carrying} nodes are named {label:?}");
        let node = self.node(label).0;
        self.action(node, accesskit::Action::Click, None);
    }

    /// Clicks one of several nodes that share a name, counted from 0 in reading order: top to
    /// bottom, then left to right.
    pub(super) fn click_in_reading_order(&mut self, label: &str, position: usize) {
        let mut carrying: Vec<(accesskit::NodeId, accesskit::Rect)> = self
            .output
            .platform_output
            .accesskit_update
            .iter()
            .flat_map(|update| update.nodes.iter())
            .filter(|(_, node)| node.label() == Some(label))
            .map(|(id, node)| (*id, node.bounds().unwrap_or_default()))
            .collect();
        carrying.sort_by(|(_, first), (_, second)| {
            first
                .y0
                .total_cmp(&second.y0)
                .then(first.x0.total_cmp(&second.x0))
        });
        let node = carrying
            .get(position)
            .unwrap_or_else(|| panic!("{} nodes are named {label:?}", carrying.len()))
            .0;
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
