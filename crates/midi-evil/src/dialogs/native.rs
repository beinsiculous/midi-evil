//! The desktop's own dialogs.

use std::path::{Path, PathBuf};

use super::Dialogs;

/// The desktop's file and message dialogs. Each blocks the interface thread until it closes.
pub(crate) struct DesktopDialogs;

fn file_dialog(
    title: Option<&str>,
    filters: &[(String, Vec<String>)],
    directory: Option<&Path>,
    file_name: Option<&str>,
) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = title {
        dialog = dialog.set_title(title);
    }
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    for (name, extensions) in filters {
        dialog = dialog.add_filter(name, extensions.as_slice());
    }
    if let Some(file_name) = file_name {
        dialog = dialog.set_file_name(file_name);
    }
    dialog
}

impl Dialogs for DesktopDialogs {
    fn pick_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        file_dialog(title, filters, directory, file_name).pick_file()
    }

    fn pick_files(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<Vec<PathBuf>> {
        file_dialog(title, filters, directory, file_name).pick_files()
    }

    fn pick_folder(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        file_dialog(title, filters, directory, file_name).pick_folder()
    }

    fn save_file(
        &mut self,
        title: Option<&str>,
        filters: &[(String, Vec<String>)],
        directory: Option<&Path>,
        file_name: Option<&str>,
    ) -> Option<PathBuf> {
        file_dialog(title, filters, directory, file_name).save_file()
    }

    fn confirm(&mut self, title: &str, description: &str) -> bool {
        let answer = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(description)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        answered_yes(&answer)
    }
}

/// Only Yes is yes. A prompt that is closed, cancelled or dismissed some other way must never
/// count as agreeing to discard something.
fn answered_yes(answer: &rfd::MessageDialogResult) -> bool {
    *answer == rfd::MessageDialogResult::Yes
}

#[cfg(test)]
mod tests {
    use super::answered_yes;
    use rfd::MessageDialogResult::{Cancel, Custom, No, Ok, Yes};

    #[test]
    fn only_yes_agrees_and_a_closed_or_cancelled_prompt_does_not() {
        assert!(answered_yes(&Yes));
        for refusal in [No, Ok, Cancel, Custom("Yes".to_owned())] {
            assert!(!answered_yes(&refusal), "{refusal:?}");
        }
    }
}
