//! Which build is running: the desktop's or the browser's. It is decided once, where the
//! application is made, and kept in egui's memory beside the widgets, so every module reads the
//! same value from the context it already has and a test can run the browser's behaviour
//! natively by installing it. What a platform shows is decided here too, in one list: each
//! control asks it whether to draw itself, and the one place a view is selected asks it whether
//! that view may be shown.

use eframe::egui;

use crate::speech::Unavailable;
use crate::workspace::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    #[default]
    Desktop,
    Browser,
}

/// Something the desktop shows that a platform may leave out. A thing left out is not drawn, so
/// it is neither a Tab stop nor in the frame's tree, and nothing else leads to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// A view: its tab, and every button that opens it.
    View(View),
    /// The Notes view's Automation panel, its toggle, and the side panel's Automation button.
    Automation,
    /// The MIDI input port and its refresh button.
    MidiInput,
    /// Import MIDI, in the Project menu and in the About window.
    ImportMidi,
    RecoverRhythmFromWav,
    RelinkMidi,
    /// Export MIDI, in the Project menu and in the export row.
    ExportMidi,
    /// The WAV renders, the track stems and the arrangement parts, wherever they are offered.
    WavRenders,
    /// The toolbar's Export toggle and the row it shows.
    ExportRow,
    /// Load files…, in the sound bar and in the sound library.
    LoadSoundFiles,
    /// Moving a window by its title bar, which is then a stop of its own.
    MovingWindows,
    /// Dragging the side panel's edge to resize it, which is then a stop of its own.
    ResizingTheSidePanel,
}

/// What the browser build leaves out, each until the batch that gives it its own gate removes
/// its line: the views that have no keyboard model yet, the controls that need a file, a device
/// or a renderer a page does not give this build, and the stops only a pointer can drag.
const LEFT_OUT_OF_THE_BROWSER: [Shown; 15] = [
    Shown::View(View::Mixer),
    Shown::View(View::Synth),
    Shown::View(View::Keyboard),
    Shown::View(View::Pads),
    Shown::Automation,
    Shown::MidiInput,
    Shown::ImportMidi,
    Shown::RecoverRhythmFromWav,
    Shown::RelinkMidi,
    Shown::ExportMidi,
    Shown::WavRenders,
    Shown::ExportRow,
    Shown::LoadSoundFiles,
    Shown::MovingWindows,
    Shown::ResizingTheSidePanel,
];

impl Platform {
    /// The build this binary is. The only place the library asks the compiler which it is.
    pub const fn of_this_build() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Browser
        } else {
            Self::Desktop
        }
    }

    /// What this platform leaves out of what the desktop shows.
    pub fn left_out(self) -> &'static [Shown] {
        match self {
            Self::Desktop => &[],
            Self::Browser => &LEFT_OUT_OF_THE_BROWSER,
        }
    }

    /// Why nothing can play, record or start from a bar: the renderer spawns a thread, which a
    /// browser does not give this build.
    pub fn playback_refused(self) -> Option<Unavailable> {
        match self {
            Self::Desktop => None,
            Self::Browser => Some(Unavailable::NoPlaybackInBrowser),
        }
    }

    /// Whether an unavailable control keeps its place in the Tab order. A desktop screen reader
    /// finds a disabled control by reviewing the window; a canvas in a page offers no review, so
    /// in the browser an unavailable control can still be reached, and says why.
    pub fn keeps_unavailable_controls_reachable(self) -> bool {
        self == Self::Browser
    }

    /// Whether an outcome waits for a second pass, so a front end that writes one string a frame
    /// says it with the controls as its action left them. A desktop screen reader is told the
    /// outcome and then each change as it comes, and nothing there is run twice.
    pub fn pairs_an_outcome_with_the_controls_it_changed(self) -> bool {
        self == Self::Browser
    }

    /// Whether a text field that an Enter or an Escape ended keeps its place. On the desktop
    /// egui leaves nothing focused, as it always has; in the browser that would send the next
    /// Tab back to the page's first control, so the field stays what is current and Tab moves on
    /// from it.
    pub fn keeps_the_place_of_an_ended_edit(self) -> bool {
        self == Self::Browser
    }
}

fn memory_id() -> egui::Id {
    egui::Id::new("platform")
}

fn shown_memory_id() -> egui::Id {
    egui::Id::new("platform-shown")
}

/// Records which build this is, for every later frame of this context.
pub fn install(context: &egui::Context, platform: Platform) {
    context.data_mut(|data| data.insert_temp(memory_id(), platform));
}

/// The build recorded for this context; the desktop's when none was.
pub fn of(context: &egui::Context) -> Platform {
    context.data(|data| data.get_temp(memory_id()).unwrap_or_default())
}

/// Has this context show what `platform` shows while it goes on behaving as the build it is: how
/// a gate shows that its walk would find a control the browser leaves out, were it drawn.
#[cfg(feature = "gate")]
pub fn install_shown(context: &egui::Context, platform: Platform) {
    context.data_mut(|data| data.insert_temp(shown_memory_id(), platform));
}

/// Whether this context draws `thing`.
pub fn shows(context: &egui::Context, thing: Shown) -> bool {
    let shown_as = context.data(|data| data.get_temp(shown_memory_id()));
    !shown_as
        .unwrap_or_else(|| of(context))
        .left_out()
        .contains(&thing)
}
