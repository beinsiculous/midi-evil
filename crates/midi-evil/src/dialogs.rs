//! Every dialog the application opens passes through one `Dialogs`: the desktop answers with
//! its own dialogs, a test with scripted answers, and a build that has none yet with nothing.

use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::DesktopDialogs;

/// The questions the application asks with a dialog. A file dialog is asked with its title, its
/// filters (each a name and the extensions it admits), the folder it starts in and the file name
/// it suggests, and answers `None` when it is closed or cancelled without a choice.
///
/// Each is asked and answered before the call returns, as the desktop's dialogs are; a browser's
/// picker answers later and with bytes, so a browser's dialogs will change this shape.
pub trait Dialogs {
    /// One existing file.
    fn pick_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf>;

    /// Any number of existing files.
    fn pick_files(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>>;

    /// One existing folder.
    fn pick_folder(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf>;

    /// Where to write a new file.
    fn save_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf>;

    /// A yes-or-no question: true only for Yes, so a prompt closed or cancelled is no.
    fn confirm(&mut self, title: &str, description: &str) -> bool;
}

/// A build without dialogs of its own: nothing is ever chosen, and every question is answered no.
#[cfg(target_arch = "wasm32")]
impl Dialogs for () {
    fn pick_file(
        &mut self,
        _title: Option<&str>,
        _filters: &[(String, Vec<String>)],
        _directory: Option<&Path>,
        _file_name: Option<&str>,
    ) -> Option<PathBuf> {
        None
    }

    fn pick_files(
        &mut self,
        _title: Option<&str>,
        _filters: &[(String, Vec<String>)],
        _directory: Option<&Path>,
        _file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>> {
        None
    }

    fn pick_folder(
        &mut self,
        _title: Option<&str>,
        _filters: &[(String, Vec<String>)],
        _directory: Option<&Path>,
        _file_name: Option<&str>,
    ) -> Option<PathBuf> {
        None
    }

    fn save_file(
        &mut self,
        _title: Option<&str>,
        _filters: &[(String, Vec<String>)],
        _directory: Option<&Path>,
        _file_name: Option<&str>,
    ) -> Option<PathBuf> {
        None
    }

    fn confirm(&mut self, _title: &str, _description: &str) -> bool {
        false
    }
}

/// One file dialog's request, built the way the desktop's dialog builder is, so each site
/// reads as it did, and asked through the `Dialogs` it was given. No site asks with a title.
pub(crate) struct FileDialog<'a> {
    dialogs: &'a mut dyn Dialogs,
    filters: Vec<(String, Vec<String>)>,
    directory: Option<PathBuf>,
    file_name: Option<String>,
}

impl<'a> FileDialog<'a> {
    pub(crate) fn new(dialogs: &'a mut dyn Dialogs) -> Self {
        Self {
            dialogs,
            filters: Vec::new(),
            directory: None,
            file_name: None,
        }
    }

    pub(crate) fn add_filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((
            name.to_owned(),
            extensions
                .iter()
                .map(|extension| (*extension).to_owned())
                .collect(),
        ));
        self
    }

    pub(crate) fn set_directory(mut self, directory: &Path) -> Self {
        self.directory = Some(directory.to_path_buf());
        self
    }

    pub(crate) fn set_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    pub(crate) fn pick_file(self) -> Option<PathBuf> {
        let directory = self.directory.as_deref();
        let file_name = self.file_name.as_deref();
        self.dialogs
            .pick_file(None, &self.filters, directory, file_name)
    }

    pub(crate) fn pick_files(self) -> Option<Vec<PathBuf>> {
        let directory = self.directory.as_deref();
        let file_name = self.file_name.as_deref();
        self.dialogs
            .pick_files(None, &self.filters, directory, file_name)
    }

    pub(crate) fn pick_folder(self) -> Option<PathBuf> {
        let directory = self.directory.as_deref();
        let file_name = self.file_name.as_deref();
        self.dialogs
            .pick_folder(None, &self.filters, directory, file_name)
    }

    pub(crate) fn save_file(self) -> Option<PathBuf> {
        let directory = self.directory.as_deref();
        let file_name = self.file_name.as_deref();
        self.dialogs
            .save_file(None, &self.filters, directory, file_name)
    }
}
