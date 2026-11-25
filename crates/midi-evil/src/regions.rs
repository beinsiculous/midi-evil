//! The window's three regions — the toolbar, the side panel and the editor — each a named group
//! in the accessibility tree, and F6 / Shift+F6 to move between them. A widget's region is the
//! one whose panel built it, recorded while the panel runs; never guessed from where it is
//! drawn, because a scroll canvas's rectangle can lie anywhere.

use eframe::egui::{self, accesskit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Toolbar,
    SidePanel,
    Editor,
}

impl Region {
    pub const ORDER: [Self; 3] = [Self::Toolbar, Self::SidePanel, Self::Editor];

    pub fn name(self) -> &'static str {
        match self {
            Self::Toolbar => "Toolbar",
            Self::SidePanel => "Side panel",
            Self::Editor => "Editor",
        }
    }

    fn position(self) -> usize {
        match self {
            Self::Toolbar => 0,
            Self::SidePanel => 1,
            Self::Editor => 2,
        }
    }

    fn group_id(self) -> egui::Id {
        egui::Id::new("region").with(self.position())
    }
}

#[derive(Debug, Default, Clone)]
struct RegionStops {
    /// Focusable, enabled widgets in the order they were built.
    focusable: Vec<egui::Id>,
    /// Where F6 lands when it is there: the roll, the selected track.
    preferred: Option<egui::Id>,
}

#[derive(Debug, Default, Clone)]
struct Regions {
    building: [RegionStops; 3],
    built: [RegionStops; 3],
}

impl Regions {
    fn region_of(&self, widget: egui::Id) -> Option<Region> {
        Region::ORDER
            .into_iter()
            .find(|region| self.built[region.position()].focusable.contains(&widget))
    }

    /// The region's preferred stop while it still exists, else its first focusable widget.
    fn stop(&self, region: Region) -> Option<egui::Id> {
        let stops = &self.built[region.position()];
        stops
            .preferred
            .filter(|preferred| stops.focusable.contains(preferred))
            .or_else(|| stops.focusable.first().copied())
    }
}

// The map lives in egui's memory, beside the widgets it records, so a panel can be grouped
// from inside the application's own methods without a second borrow of the application.
fn with_regions<R>(context: &egui::Context, reader: impl FnOnce(&mut Regions) -> R) -> R {
    context.data_mut(|data| reader(data.get_temp_mut_or_default(egui::Id::new("regions"))))
}

/// Runs a panel's contents inside its region's group and records the widgets they build.
pub fn group<R>(
    ui: &mut egui::Ui,
    region: Region,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let group_id = region.group_id();
    ui.ctx().accesskit_node_builder(group_id, |node| {
        node.set_role(accesskit::Role::Group);
        node.set_label(region.name());
    });
    let first_widget = background_widget_count(ui.ctx());
    let inner = ui
        .scope_builder(
            egui::UiBuilder::new().accessibility_parent(group_id),
            add_contents,
        )
        .inner;
    let built = ui.ctx().viewport(|viewport| {
        viewport
            .this_pass
            .widgets
            .get_layer(egui::LayerId::background())
            .skip(first_widget)
            .filter(|widget| widget.enabled && widget.sense.is_focusable())
            .map(|widget| widget.id)
            .collect::<Vec<_>>()
    });
    with_regions(ui.ctx(), |regions| {
        regions.building[region.position()].focusable.extend(built);
    });
    inner
}

/// Names the widget F6 lands on in this region, for this frame.
pub fn prefer_stop(context: &egui::Context, region: Region, widget: egui::Id) {
    with_regions(context, |regions| {
        regions.building[region.position()].preferred = Some(widget);
    });
}

/// What this frame built becomes what the next frame's F6 moves between.
pub fn end_frame(context: &egui::Context) {
    with_regions(context, |regions| {
        regions.built = std::mem::take(&mut regions.building);
    });
}

#[cfg(test)]
pub fn region_of(context: &egui::Context, widget: egui::Id) -> Option<Region> {
    with_regions(context, |regions| regions.region_of(widget))
}

/// Where F6 (forwards) or Shift+F6 goes from the focused widget, skipping empty regions.
/// From outside every region, F6 starts at the first and Shift+F6 at the last.
pub fn next_stop(
    context: &egui::Context,
    focused: Option<egui::Id>,
    forwards: bool,
) -> Option<egui::Id> {
    with_regions(context, |regions| {
        let count = Region::ORDER.len();
        let current = focused.and_then(|widget| regions.region_of(widget));
        let start = match (current, forwards) {
            (Some(region), true) => region.position() + 1,
            (Some(region), false) => region.position() + count - 1,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        (0..count)
            .map(|step| {
                if forwards {
                    (start + step) % count
                } else {
                    (start + count - step) % count
                }
            })
            .find_map(|position| regions.stop(Region::ORDER[position]))
    })
}

#[cfg(test)]
fn stop(context: &egui::Context, region: Region) -> Option<egui::Id> {
    with_regions(context, |regions| regions.stop(region))
}

fn background_widget_count(context: &egui::Context) -> usize {
    context.viewport(|viewport| {
        viewport
            .this_pass
            .widgets
            .get_layer(egui::LayerId::background())
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(context: &egui::Context, editor_has_roll: bool) -> Vec<egui::Id> {
        let mut ids = Vec::new();
        let _ = context.run_ui(egui::RawInput::default(), |ui| {
            group(ui, Region::Toolbar, |ui| {
                ids.push(ui.button("Project").id);
            });
            group(ui, Region::SidePanel, |ui| {
                ids.push(ui.button("01 one").id);
                let selected = ui.button("02 two").id;
                ids.push(selected);
                prefer_stop(ui.ctx(), Region::SidePanel, selected);
            });
            group(ui, Region::Editor, |ui| {
                ids.push(ui.button("+ Add a track").id);
                if editor_has_roll {
                    let roll = ui
                        .allocate_response(egui::vec2(100.0, 3_000.0), egui::Sense::click())
                        .id;
                    ids.push(roll);
                    prefer_stop(ui.ctx(), Region::Editor, roll);
                }
            });
            end_frame(ui.ctx());
        });
        ids
    }

    #[test]
    fn f6_cycles_through_the_regions_by_the_panel_that_built_each_widget() {
        let context = egui::Context::default();
        let ids = frame(&context, true);
        let (toolbar, selected_track, roll) = (ids[0], ids[2], ids[4]);
        assert_eq!(next_stop(&context, None, true), Some(toolbar));
        assert_eq!(
            next_stop(&context, Some(toolbar), true),
            Some(selected_track)
        );
        assert_eq!(next_stop(&context, Some(selected_track), true), Some(roll));
        assert_eq!(next_stop(&context, Some(roll), true), Some(toolbar));
        assert_eq!(next_stop(&context, Some(toolbar), false), Some(roll));
        assert_eq!(region_of(&context, roll), Some(Region::Editor));
    }

    #[test]
    fn a_stop_that_no_longer_exists_falls_back_to_the_regions_first_widget() {
        let context = egui::Context::default();
        frame(&context, true);
        let ids = frame(&context, false);
        assert_eq!(stop(&context, Region::Editor), Some(ids[3]));
    }
}
