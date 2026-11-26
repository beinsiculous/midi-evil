//! The keyboard notes: one table of chords per surface — the application's own, the arranger's
//! and the piano roll's. The key dispatch reads them, and every place a chord is written for a
//! person — an editor's accessible description, the help window, a menu item, a button's hover
//! text, a notice — is written from them, so the browser's build, which leaves out every chord a
//! browser gives a meaning to, cannot show or say one it does not bind.

use eframe::egui;

use crate::arranger::edits::SongCommand;
use crate::piano_roll::edits::RollCommand;
use crate::platform::Platform;
use crate::speech;

mod page_keys;
pub use page_keys::KeyTable;

/// The modifier half of a chord. `Command` is Control, or Command on macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordModifier {
    None,
    Shift,
    Command,
    CommandShift,
    Alt,
}

impl ChordModifier {
    /// The modifier a key press carries when it is exactly one of the editors' modifiers, so
    /// Right and Control+Right are never confused.
    pub fn of_editor_press(modifiers: egui::Modifiers) -> Option<Self> {
        if modifiers.is_none() {
            Some(Self::None)
        } else if modifiers.shift_only() {
            Some(Self::Shift)
        } else if modifiers.command_only() {
            Some(Self::Command)
        } else {
            None
        }
    }

    /// The modifier a key press carries, when it is exactly one of the five: any other
    /// combination binds nothing, so a chord with an extra modifier held reaches no command.
    pub fn of_press(modifiers: egui::Modifiers) -> Option<Self> {
        let only_alt = modifiers.alt && !(modifiers.shift || modifiers.ctrl || modifiers.command);
        if let Some(modifier) = Self::of_editor_press(modifiers) {
            Some(modifier)
        } else if modifiers.command && modifiers.shift && !modifiers.alt {
            Some(Self::CommandShift)
        } else if only_alt && !modifiers.mac_cmd {
            Some(Self::Alt)
        } else {
            None
        }
    }

    /// "Control+", "Shift+", or nothing: the chord's text before its key.
    pub fn prefix(self, command_key: &str) -> String {
        match self {
            Self::None => String::new(),
            Self::Shift => "Shift+".to_owned(),
            Self::Command => format!("{command_key}+"),
            Self::CommandShift => format!("{command_key}+Shift+"),
            Self::Alt => "Alt+".to_owned(),
        }
    }
}

/// One line of a surface's keyboard notes: a chord and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyNote {
    pub modifier: ChordModifier,
    pub key: egui::Key,
    pub action: &'static str,
}

impl KeyNote {
    /// "Control+Left", with the platform's name for the command key.
    pub fn chord(&self, command_key: &str) -> String {
        format!(
            "{}{}",
            self.modifier.prefix(command_key),
            key_name(self.key)
        )
    }
}

/// A key as a person reads it: "Page Down", where egui's own name runs the words together.
pub fn key_name(key: egui::Key) -> &'static str {
    match key {
        egui::Key::PageDown => "Page Down",
        egui::Key::PageUp => "Page Up",
        other => other.name(),
    }
}

/// The application's own chords, which work wherever no focused editor or text field claims the
/// key. The key dispatch reads them through `from_key`; the help reads them through `notes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    NewProject,
    OpenProject,
    SaveProject,
    Undo,
    Redo,
    PreviousSound,
    NextSound,
    PlayOrStop,
    NextRegion,
    PreviousRegion,
}

impl AppCommand {
    pub const ALL: [Self; 10] = [
        Self::NewProject,
        Self::OpenProject,
        Self::SaveProject,
        Self::Undo,
        Self::Redo,
        Self::PreviousSound,
        Self::NextSound,
        Self::PlayOrStop,
        Self::NextRegion,
        Self::PreviousRegion,
    ];

    /// Every chord bound to the command on this platform; Redo has two. In the browser a chord a
    /// browser gives a meaning of its own is not bound at all — New, Open and the sounds are
    /// reached by the Project menu and the buttons beside the sound — and the regions move on
    /// Alt with Page Down and Page Up, because F6 is the browser's walk between its own panes.
    pub fn chords_on(self, platform: Platform) -> &'static [(ChordModifier, egui::Key)] {
        use ChordModifier::{Alt, Command, CommandShift, None, Shift};
        use egui::Key;
        match (self, platform) {
            (Self::NewProject, Platform::Desktop) => &[(Command, Key::N)],
            (Self::OpenProject, Platform::Desktop) => &[(Command, Key::O)],
            (Self::SaveProject, _) => &[(Command, Key::S)],
            (Self::Undo, _) => &[(Command, Key::Z)],
            (Self::Redo, _) => &[(CommandShift, Key::Z), (Command, Key::Y)],
            (Self::PreviousSound, Platform::Desktop) => &[(Alt, Key::ArrowLeft)],
            (Self::NextSound, Platform::Desktop) => &[(Alt, Key::ArrowRight)],
            (Self::PlayOrStop, _) => &[(None, Key::Space)],
            (Self::NextRegion, Platform::Desktop) => &[(None, Key::F6)],
            (Self::PreviousRegion, Platform::Desktop) => &[(Shift, Key::F6)],
            (Self::NextRegion, Platform::Browser) => &[(Alt, Key::PageDown)],
            (Self::PreviousRegion, Platform::Browser) => &[(Alt, Key::PageUp)],
            (Self::NewProject | Self::OpenProject, Platform::Browser)
            | (Self::PreviousSound | Self::NextSound, Platform::Browser) => &[],
        }
    }

    /// The desktop's chords for the command.
    #[cfg(test)]
    pub fn chords(self) -> &'static [(ChordModifier, egui::Key)] {
        self.chords_on(Platform::Desktop)
    }

    /// The command a key press means on this platform. In the browser, the chord in the table
    /// whose modifiers are exactly the ones held, or none. On the desktop, as it always has: the
    /// command key's chords accept other modifiers held with it, Shift turning Undo into Redo;
    /// Alt's accept the logical Alt; Space plays whatever else is held.
    pub fn from_press(
        key: egui::Key,
        modifiers: egui::Modifiers,
        platform: Platform,
    ) -> Option<Self> {
        if platform == Platform::Desktop {
            return Self::from_desktop_press(key, modifiers);
        }
        let modifier = ChordModifier::of_press(modifiers)?;
        Self::ALL
            .into_iter()
            .find(|command| command.chords_on(platform).contains(&(modifier, key)))
    }

    fn from_desktop_press(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        use egui::Key;
        if key == Key::F6 && (modifiers.is_none() || modifiers.shift_only()) {
            return Some(if modifiers.shift {
                Self::PreviousRegion
            } else {
                Self::NextRegion
            });
        }
        if modifiers.command {
            return match key {
                Key::N => Some(Self::NewProject),
                Key::O => Some(Self::OpenProject),
                Key::S => Some(Self::SaveProject),
                Key::Z if modifiers.shift => Some(Self::Redo),
                Key::Z => Some(Self::Undo),
                Key::Y => Some(Self::Redo),
                _ => None,
            };
        }
        if modifiers.matches_logically(egui::Modifiers::ALT) {
            return match key {
                Key::ArrowLeft => Some(Self::PreviousSound),
                Key::ArrowRight => Some(Self::NextSound),
                _ => None,
            };
        }
        (key == Key::Space).then_some(Self::PlayOrStop)
    }

    /// The desktop's commands other than the region keys, which are the dispatch's own.
    #[cfg(test)]
    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        Self::from_press(key, modifiers, Platform::Desktop)
            .filter(|command| !matches!(command, Self::NextRegion | Self::PreviousRegion))
    }

    /// "Ctrl+N", the way a menu item writes the command's first chord, or nothing when this
    /// platform binds none.
    pub fn menu_shortcut(self, platform: Platform) -> Option<String> {
        let &(modifier, key) = self.chords_on(platform).first()?;
        Some(format!("{}{}", modifier.prefix("Ctrl"), key_name(key)))
    }

    /// "Ctrl/Cmd+Y", the way a hover text writes the command's chord on `key`, or nothing when
    /// this platform binds the command to no chord on that key.
    pub fn hover_shortcut(self, key: egui::Key, platform: Platform) -> Option<String> {
        let &(modifier, key) = self
            .chords_on(platform)
            .iter()
            .find(|(_, bound)| *bound == key)?;
        Some(format!("{}{}", modifier.prefix("Ctrl/Cmd"), key_name(key)))
    }
}

/// A menu item's label: its title and, past the gap that lines it up with its neighbours, the
/// command's chord, or the title alone where this platform binds the command to none.
pub fn menu_label(title: &str, gap: &str, command: AppCommand, platform: Platform) -> String {
    match command.menu_shortcut(platform) {
        Some(chord) => format!("{title}{gap}{chord}"),
        None => title.to_owned(),
    }
}

/// A button's hover text: what it does and, after a dot, the command's chord on `key`, or what
/// it does alone where this platform binds no such chord.
pub fn hover_text(does: &str, command: AppCommand, key: egui::Key, platform: Platform) -> String {
    match command.hover_shortcut(key, platform) {
        Some(chord) if does.is_empty() => chord,
        Some(chord) => format!("{does} · {chord}"),
        None => does.to_owned(),
    }
}

/// "F6", the chord that moves to the next region, as a notice says it.
pub fn next_region_chord(platform: Platform) -> String {
    region_chord(AppCommand::NextRegion, platform)
}

/// "F6 and Shift+F6", the two region chords, as a description says them.
pub fn region_chords(platform: Platform) -> String {
    format!(
        "{} and {}",
        region_chord(AppCommand::NextRegion, platform),
        region_chord(AppCommand::PreviousRegion, platform)
    )
}

fn region_chord(command: AppCommand, platform: Platform) -> String {
    command
        .chords_on(platform)
        .first()
        .map(|&(modifier, key)| format!("{}{}", modifier.prefix("Control"), key_name(key)))
        .unwrap_or_default()
}

/// The surfaces that have keyboard notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Application,
    Song,
    Roll,
}

impl Surface {
    pub const ALL: [Self; 3] = [Self::Application, Self::Song, Self::Roll];

    pub fn title(self) -> &'static str {
        match self {
            Self::Application => "Anywhere",
            Self::Song => "Song (the arranger)",
            Self::Roll => "Piano roll",
        }
    }

    /// The desktop's table.
    #[cfg(test)]
    pub fn notes(self) -> Vec<KeyNote> {
        self.notes_on(Platform::Desktop)
    }

    /// The surface's table on this platform, in the order its chords are learned.
    pub fn notes_on(self, platform: Platform) -> Vec<KeyNote> {
        match self {
            Self::Application => AppCommand::ALL
                .into_iter()
                .flat_map(|command| {
                    command
                        .chords_on(platform)
                        .iter()
                        .map(move |&(modifier, key)| KeyNote {
                            modifier,
                            key,
                            action: speech::app_command_action(command),
                        })
                })
                .collect(),
            Self::Song => SongCommand::WITH_CHORDS
                .into_iter()
                .filter_map(|command| {
                    command.chord().map(|(modifier, key)| KeyNote {
                        modifier,
                        key,
                        action: speech::song_command_action(command),
                    })
                })
                .collect(),
            Self::Roll => RollCommand::ALL
                .into_iter()
                .map(|command| {
                    let (modifier, key) = command.chord();
                    KeyNote {
                        modifier,
                        key,
                        action: speech::roll_command_action(command),
                    }
                })
                .collect(),
        }
    }
}

/// "Left previous note, Right next note": a table as one sentence, for an editor's description.
pub fn sentence(notes: &[KeyNote], command_key: &str) -> String {
    notes
        .iter()
        .map(|note| format!("{} {}", note.chord(command_key), note.action))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An editor's accessible description: its whole table, read once on focus, and the two ways out.
pub fn editor_description(surface: Surface, command_key: &str, platform: Platform) -> String {
    format!(
        "{}. {} move between the toolbar, the side panel and the editor; Tab leaves the {}.",
        sentence(&surface.notes_on(platform), command_key),
        region_chords(platform),
        match surface {
            Surface::Song => "arranger",
            Surface::Roll | Surface::Application => "roll",
        }
    )
}

/// The desktop's help window text, with `keep` choosing the lines.
#[cfg(test)]
pub fn help_text(command_key: &str, keep: impl Fn(&KeyNote) -> bool) -> String {
    help_text_on(command_key, Platform::Desktop, keep)
}

/// The help window's text on this platform: each surface's title, then one chord a line. `keep`
/// leaves out further chords.
pub fn help_text_on(
    command_key: &str,
    platform: Platform,
    keep: impl Fn(&KeyNote) -> bool,
) -> String {
    Surface::ALL
        .into_iter()
        .map(|surface| {
            let lines: Vec<String> = surface
                .notes_on(platform)
                .iter()
                .filter(|note| keep(note))
                .map(|note| format!("{}: {}", note.chord(command_key), note.action))
                .collect();
            format!("{}\n{}", surface.title(), lines.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers_of(modifier: ChordModifier) -> egui::Modifiers {
        match modifier {
            ChordModifier::None => egui::Modifiers::NONE,
            ChordModifier::Shift => egui::Modifiers::SHIFT,
            ChordModifier::Command => egui::Modifiers::COMMAND,
            ChordModifier::CommandShift => egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            ChordModifier::Alt => egui::Modifiers::ALT,
        }
    }

    #[test]
    fn every_application_chord_in_the_table_is_the_one_the_dispatch_reads() {
        for command in AppCommand::ALL {
            for &(modifier, key) in command.chords() {
                if key == egui::Key::F6 {
                    continue;
                }
                assert_eq!(
                    AppCommand::from_key(key, modifiers_of(modifier)),
                    Some(command),
                    "{command:?} on {modifier:?}+{key:?}"
                );
            }
        }
    }

    #[test]
    fn every_editor_chord_in_a_table_is_the_one_its_editor_reads() {
        for note in Surface::Roll.notes() {
            assert!(
                RollCommand::from_key(note.key, modifiers_of(note.modifier)).is_some(),
                "{note:?}"
            );
        }
        for note in Surface::Song.notes() {
            assert!(
                SongCommand::from_key(note.key, modifiers_of(note.modifier)).is_some(),
                "{note:?}"
            );
        }
    }

    #[test]
    fn the_help_has_every_surface_and_can_leave_out_a_chord() {
        let help = help_text("Control", |_| true);
        for surface in Surface::ALL {
            assert!(help.contains(surface.title()), "{surface:?}");
        }
        assert!(help.contains("Control+Shift+Z: "));
        assert!(help.contains("Alt+Left: "));
        let without = help_text("Control", |note| note.modifier != ChordModifier::Alt);
        assert!(!without.contains("Alt+"));
        assert!(without.contains("Control+Left: "));
    }
}
