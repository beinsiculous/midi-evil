//! The application's own custom-painted widgets — the piano roll, the automation and mixer lanes,
//! the transport readout. A focused widget of this kind takes its keys from the application's
//! key dispatch and leaves Space to the transport, where egui's own widgets — a button, a
//! checkbox — act on Space themselves. Each names itself here while it is drawn; the next
//! frame's key dispatch, which runs before anything is drawn, reads what the frame before
//! recorded.

use eframe::egui;

#[derive(Debug, Default, Clone)]
struct Editors {
    pass: u64,
    this_pass: Vec<egui::Id>,
    previous_pass: Vec<egui::Id>,
}

fn with_editors<R>(context: &egui::Context, reader: impl FnOnce(&mut Editors) -> R) -> R {
    let pass = context.cumulative_pass_nr();
    context.data_mut(|data| {
        let editors: &mut Editors = data.get_temp_mut_or_default(egui::Id::new("custom-editors"));
        if editors.pass != pass {
            let recorded = std::mem::take(&mut editors.this_pass);
            // Only the pass just before this one counts: an editor not drawn since is gone.
            editors.previous_pass = if editors.pass + 1 == pass {
                recorded
            } else {
                Vec::new()
            };
            editors.pass = pass;
        }
        reader(editors)
    })
}

/// Records, for this frame, that the widget is one of the application's own editors.
pub fn register(context: &egui::Context, widget: egui::Id) {
    with_editors(context, |editors| editors.this_pass.push(widget));
}

/// Whether the widget was drawn as one of the application's own editors last frame.
pub fn was_registered(context: &egui::Context, widget: egui::Id) -> bool {
    with_editors(context, |editors| editors.previous_pass.contains(&widget))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_editor_registered_while_drawn_is_known_to_the_next_frame() {
        let context = egui::Context::default();
        let editor = egui::Id::new("lane");
        let _ = context.run_ui(egui::RawInput::default(), |ui| register(ui.ctx(), editor));
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(was_registered(ui.ctx(), editor));
            assert!(!was_registered(ui.ctx(), egui::Id::new("a button")));
        });
        let _ = context.run_ui(egui::RawInput::default(), |_| {});
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(
                !was_registered(ui.ctx(), editor),
                "it was not drawn last frame"
            );
        });
    }
}
