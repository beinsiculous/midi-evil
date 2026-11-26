//! The chords a page hands to the application rather than to the browser, from the tables above
//! it: every chord the application takes, and, while a field is edited, the field's own on the
//! visitor's system.

use eframe::egui;

use super::{ChordModifier, Surface};
use crate::platform::Platform;

/// The modifiers a chord is pressed with, exactly: a page's key is held by the table only when
/// these are the ones down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    alt: bool,
    shift: bool,
    command: bool,
}

impl Held {
    const COMMAND: Self = Self {
        alt: false,
        shift: false,
        command: true,
    };
    const ALT: Self = Self {
        alt: true,
        shift: false,
        command: false,
    };

    fn of(modifier: ChordModifier) -> Self {
        let (alt, shift, command) = match modifier {
            ChordModifier::None => (false, false, false),
            ChordModifier::Shift => (false, true, false),
            ChordModifier::Command => (false, false, true),
            ChordModifier::CommandShift => (false, true, true),
            ChordModifier::Alt => (true, false, false),
        };
        Self {
            alt,
            shift,
            command,
        }
    }

    fn with_shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }
}

/// The chords a text field takes for itself while it is being edited, on the visitor's system as
/// egui's text editor reads it: select-all, copy, cut, paste, undo and redo with the command key;
/// a word moved over or deleted with Control, or with Option on a Mac; a line's start and end
/// with Command on a Mac; and each of the movements extending a selection with Shift. In the
/// browser these stay the application's while a field is edited, because the input eframe keeps
/// holds none of the field's text for the browser to do them on.
fn text_entry_chords(system: egui::os::OperatingSystem) -> Vec<(Held, egui::Key)> {
    use egui::Key;
    let mac = system == egui::os::OperatingSystem::Mac;
    let word = if mac { Held::ALT } else { Held::COMMAND };
    let mut chords: Vec<(Held, Key)> = [Key::A, Key::C, Key::X, Key::V, Key::Z, Key::Y]
        .into_iter()
        .map(|key| (Held::COMMAND, key))
        .collect();
    chords.push((Held::COMMAND.with_shift(), Key::Z));
    let by_word = [Key::ArrowLeft, Key::ArrowRight, Key::Backspace, Key::Delete];
    let by_line = [Key::Home, Key::End, Key::ArrowUp, Key::ArrowDown];
    let mut moves: Vec<(Held, Key)> = by_word.into_iter().map(|key| (word, key)).collect();
    moves.extend(by_line.into_iter().map(|key| (Held::COMMAND, key)));
    if mac {
        moves.extend(by_word.into_iter().map(|key| (Held::COMMAND, key)));
    }
    for (held, key) in moves {
        chords.push((held, key));
        chords.push((held.with_shift(), key));
    }
    chords
}

/// Every chord the application takes on this platform, for a page that must hand every other
/// chord back to the browser: the application's own, the arranger's and the roll's, and, while a
/// text field is being edited, the field's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTable {
    chords: Vec<(Held, egui::Key)>,
}

impl KeyTable {
    pub fn of(platform: Platform, editing_text: bool, system: egui::os::OperatingSystem) -> Self {
        let mut chords: Vec<_> = Surface::ALL
            .into_iter()
            .flat_map(|surface| surface.notes_on(platform))
            .map(|note| (Held::of(note.modifier), note.key))
            .collect();
        if editing_text {
            chords.extend(text_entry_chords(system));
        }
        Self { chords }
    }

    /// Whether a key pressed with exactly these modifiers is one of the application's chords.
    /// `command` is the command key as the page reads it; Control or the logo key held without
    /// counting as it is a modifier no chord here holds.
    pub fn holds(&self, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        if (modifiers.ctrl || modifiers.mac_cmd) && !modifiers.command {
            return false;
        }
        let held = Held {
            alt: modifiers.alt,
            shift: modifiers.shift,
            command: modifiers.command,
        };
        self.chords.contains(&(held, key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_being_edited_keeps_its_systems_word_and_line_chords() {
        use egui::Key;
        use egui::os::OperatingSystem::{Mac, Nix};
        let held = |alt: bool, shift: bool, command: bool, logo: bool| egui::Modifiers {
            alt,
            shift,
            ctrl: command && !logo,
            mac_cmd: logo,
            command,
        };
        let mac = KeyTable::of(Platform::Browser, true, Mac);
        assert!(
            mac.holds(Key::A, held(false, false, true, true)),
            "Command+A"
        );
        assert!(
            mac.holds(Key::Z, held(false, false, true, true)),
            "Command+Z"
        );
        assert!(
            mac.holds(Key::ArrowLeft, held(true, false, false, false)),
            "Option+Left"
        );
        assert!(
            mac.holds(Key::ArrowRight, held(true, true, false, false)),
            "Option+Shift+Right"
        );
        assert!(
            mac.holds(Key::Backspace, held(true, false, false, false)),
            "Option+Backspace"
        );
        assert!(
            mac.holds(Key::ArrowLeft, held(false, true, true, true)),
            "Command+Shift+Left"
        );
        let linux = KeyTable::of(Platform::Browser, true, Nix);
        assert!(
            linux.holds(Key::ArrowLeft, held(false, false, true, false)),
            "Control+Left"
        );
        assert!(
            linux.holds(Key::Backspace, held(false, false, true, false)),
            "Control+Backspace"
        );
        assert!(
            !linux.holds(Key::ArrowLeft, held(true, false, false, false)),
            "Alt+Left is Back"
        );
        assert!(
            !linux.holds(Key::W, held(false, false, true, false)),
            "Control+W is the browser's"
        );
        let outside = KeyTable::of(Platform::Browser, false, Nix);
        assert!(
            !outside.holds(Key::Space, held(false, false, false, true)),
            "logo+Space"
        );
        assert!(
            !outside.holds(Key::A, held(false, false, true, false)),
            "Control+A outside a field"
        );
    }
}
