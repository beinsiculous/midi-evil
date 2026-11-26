//! Which build is running: the desktop's or the browser's. It is decided once, where the
//! application is made, and kept in egui's memory beside the widgets, so every module reads the
//! same value from the context it already has and a test can run the browser's behaviour
//! natively by installing it.

use eframe::egui;

use crate::speech::Unavailable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    #[default]
    Desktop,
    Browser,
}

impl Platform {
    /// The build this binary is. The only place the library asks the compiler which it is.
    pub const fn of_this_build() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Browser
        } else {
            Self::Desktop
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

/// Records which build this is, for every later frame of this context.
pub fn install(context: &egui::Context, platform: Platform) {
    context.data_mut(|data| data.insert_temp(memory_id(), platform));
}

/// The build recorded for this context; the desktop's when none was.
pub fn of(context: &egui::Context) -> Platform {
    context.data(|data| data.get_temp(memory_id()).unwrap_or_default())
}
