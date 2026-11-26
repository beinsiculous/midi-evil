//! What a front end that speaks by writing text asks of the workstation after each frame. The
//! announcer makes the words; the application adds what only it knows: where Tab's ends are this
//! frame, and whether anything is unsaved.

use super::*;
use crate::key_help::KeyTable;
use crate::spoken::SpokenFrame;

impl StudioApp {
    /// This frame, as a front end that writes text is told it: the focused control, what to
    /// announce, the ends of Tab's walk and whether the document is unsaved. Called after the
    /// frame, while its accessibility tree is still the context's.
    pub fn spoken_frame(&self, context: &egui::Context) -> SpokenFrame {
        let words = self.announcer.frame_words().clone();
        let order = crate::regions::tab_order(context);
        let (first_stop, last_stop) = (order.first().copied(), order.last().copied());
        // A field whose place is kept is where Tab moves on from.
        let focus = words.focused.as_ref().map(|control| control.id);
        // The text chords go to egui for a text that can only be read too: selecting and
        // copying it are egui's, and a browser would select the page around the canvas.
        let editing_text = words
            .focused
            .as_ref()
            .is_some_and(|control| control.is_text_entry || control.holds_selectable_text);
        SpokenFrame {
            focused: words.focused,
            current: words.current,
            happened: words.happened,
            announcement: words.announcement,
            at_first_stop: focus.is_some() && focus == first_stop,
            at_last_stop: focus.is_some() && focus == last_stop,
            first_stop,
            last_stop,
            unsaved: self.dirty || self.recorder.is_some() || self.has_sound_drafts(),
            keys: KeyTable::of(crate::platform::of(context), editing_text, context.os()),
        }
    }
}
