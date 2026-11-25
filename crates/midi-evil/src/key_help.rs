//! The keyboard notes: one table of chords per surface — the application's own, the arranger's
//! and the piano roll's. An editor's accessible description and the help window are both written
//! from these tables, so a build that must leave out the chords a browser keeps for itself can
//! filter a table instead of keeping a second copy of it.

use eframe::egui;

use crate::arranger::edits::SongCommand;
use crate::piano_roll::edits::RollCommand;
use crate::speech;

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
        format!("{}{}", self.modifier.prefix(command_key), self.key.name())
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

    /// Every chord bound to the command; Redo has two.
    pub fn chords(self) -> &'static [(ChordModifier, egui::Key)] {
        use ChordModifier::{Alt, Command, CommandShift, None, Shift};
        use egui::Key;
        match self {
            Self::NewProject => &[(Command, Key::N)],
            Self::OpenProject => &[(Command, Key::O)],
            Self::SaveProject => &[(Command, Key::S)],
            Self::Undo => &[(Command, Key::Z)],
            Self::Redo => &[(CommandShift, Key::Z), (Command, Key::Y)],
            Self::PreviousSound => &[(Alt, Key::ArrowLeft)],
            Self::NextSound => &[(Alt, Key::ArrowRight)],
            Self::PlayOrStop => &[(None, Key::Space)],
            Self::NextRegion => &[(None, Key::F6)],
            Self::PreviousRegion => &[(Shift, Key::F6)],
        }
    }

    /// The command a key press means. The command key's chords accept other modifiers held
    /// with it, as they always have, except that Shift turns Undo into Redo; the region keys
    /// are the dispatch's own and are not returned here.
    pub fn from_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Self> {
        use egui::Key;
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

    /// The surface's table, in the order its chords are learned.
    pub fn notes(self) -> Vec<KeyNote> {
        match self {
            Self::Application => AppCommand::ALL
                .into_iter()
                .flat_map(|command| {
                    command
                        .chords()
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
pub fn editor_description(surface: Surface, command_key: &str) -> String {
    format!(
        "{}. F6 and Shift+F6 move between the toolbar, the side panel and the editor; Tab leaves \
         the {}.",
        sentence(&surface.notes(), command_key),
        match surface {
            Surface::Song => "arranger",
            Surface::Roll | Surface::Application => "roll",
        }
    )
}

/// The help window's text: each surface's title, then one chord a line. `keep` leaves out the
/// chords a build cannot offer.
pub fn help_text(command_key: &str, keep: impl Fn(&KeyNote) -> bool) -> String {
    Surface::ALL
        .into_iter()
        .map(|surface| {
            let lines: Vec<String> = surface
                .notes()
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
