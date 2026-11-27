//! The view the editor shows, and the one place it changes. The view is this module's alone, so
//! every route to a view comes through `select_view`, which answers a request for a view the
//! platform does not show by staying where it is: no route anybody adds can arrive at one.

use super::*;
use crate::platform::{self, Shown};

pub(super) struct SelectedView {
    view: View,
    /// The context the application draws in, whose platform says which views are shown.
    context: egui::Context,
}

impl SelectedView {
    pub(super) fn new(context: &egui::Context) -> Self {
        Self {
            view: View::default(),
            context: context.clone(),
        }
    }
}

impl StudioApp {
    pub(super) fn view(&self) -> View {
        self.view.view
    }

    /// Shows `view` when this platform shows it, and otherwise stays where it is; says whether
    /// `view` is what is shown now.
    pub(super) fn select_view(&mut self, view: View) -> bool {
        if platform::shows(&self.view.context, Shown::View(view)) {
            self.view.view = view;
        }
        self.view.view == view
    }
}
