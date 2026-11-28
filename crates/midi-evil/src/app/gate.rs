//! What the browser gate's own bundle asks of the workstation, and a visitor's bundle never can:
//! to start from a project given as text, since a page cannot be handed a file, and to show what
//! the desktop shows, which is how the gate proves its walk would find a control the browser
//! leaves out.

use super::*;
use crate::platform::{self, Platform};

impl StudioApp {
    /// The workstation started on the project `text` holds, as if opened from a file called
    /// `name`, with no file to save it back to.
    pub fn with_project_text(
        creation: &eframe::CreationContext<'_>,
        dialogs: Box<dyn Dialogs>,
        name: &str,
        text: &str,
    ) -> Result<Self, String> {
        let project = Project::from_json(text).map_err(|error| format!("{name}: {error}"))?;
        let mut app = Self::new(creation, LaunchOptions::default(), None, dialogs);
        app.adopt_project(project, None, name, files::Discard::NothingUnsaved);
        Ok(app)
    }

    /// Draws everything the desktop draws, in a build that goes on behaving as a browser's.
    pub fn show_what_the_desktop_shows(context: &egui::Context) {
        platform::install_shown(context, Platform::Desktop);
    }
}
