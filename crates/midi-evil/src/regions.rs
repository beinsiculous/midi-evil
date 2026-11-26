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

/// The controls egui's Tab reaches this frame, in the order it reaches them: the order the frame
/// made them, across panels, menus and windows alike. The frame keeps that order for each layer:
/// the panels' layer in full; a menu's or a list's own layer, made right after the button that
/// opened it; and a window's, made after every panel.
pub fn tab_order(context: &egui::Context) -> Vec<egui::Id> {
    let order = context.viewport(|viewport| {
        let widgets = &viewport.this_pass.widgets;
        let stops = |rects: &[egui::WidgetRect]| -> Vec<(egui::Id, egui::LayerId)> {
            rects
                .iter()
                .filter(|rect| rect.enabled && rect.sense.is_focusable())
                .map(|rect| (rect.id, rect.layer_id))
                .collect()
        };
        let background = egui::LayerId::background();
        let mut layers: Vec<(egui::LayerId, Vec<(egui::Id, egui::LayerId)>)> = widgets
            .layers()
            .filter(|(layer, _)| **layer != background)
            .map(|(layer, rects)| (*layer, stops(rects)))
            .filter(|(_, stops)| !stops.is_empty())
            .collect();
        // A window's layer is walked where the frame made it: its controls' nodes hang from
        // a container the tree's root took as a child when the window was drawn, so the root's
        // children are in the order the windows were made.
        if let Some(state) = viewport.this_pass.accesskit_state.as_ref() {
            let made_at = creation_ranks(&state.nodes);
            layers.sort_by_key(|(_, stops)| {
                stops
                    .iter()
                    .filter_map(|(widget, _)| made_at.get(widget).copied())
                    .min()
                    .unwrap_or(usize::MAX)
            });
        }
        let mut order = Vec::new();
        for (widget, layer) in widgets
            .get_layer(background)
            .flat_map(|rect| stops(std::slice::from_ref(rect)))
        {
            order.push((widget, layer));
            // The id egui's `Popup::default_response_id` gives a button's popup.
            let opened = widget.with("popup");
            if let Some(position) = layers.iter().position(|(layer, _)| layer.id == opened) {
                order.extend(layers.remove(position).1);
            }
        }
        order.extend(layers.into_iter().flat_map(|(_, stops)| stops));
        order
    });
    order
        .into_iter()
        .filter(|(_, layer)| context.memory(|memory| memory.allows_interaction(*layer)))
        .map(|(widget, _)| widget)
        .collect()
}

/// For every node, the place among the tree root's children of the child it hangs from: the
/// order in which the frame made the top-level containers.
fn creation_ranks(nodes: &egui::IdMap<egui::accesskit::Node>) -> egui::IdMap<usize> {
    let ids: std::collections::HashMap<egui::accesskit::NodeId, egui::Id> =
        nodes.keys().map(|id| (id.accesskit_id(), *id)).collect();
    let mut ranks = egui::IdMap::default();
    let Some(root) = nodes.get(&egui::accesskit_root_id()) else {
        return ranks;
    };
    for (rank, child) in root.children().iter().enumerate() {
        let mut walk: Vec<egui::Id> = ids.get(child).copied().into_iter().collect();
        while let Some(widget) = walk.pop() {
            ranks.insert(widget, rank);
            if let Some(node) = nodes.get(&widget) {
                walk.extend(node.children().iter().filter_map(|id| ids.get(id).copied()));
            }
        }
    }
    ranks
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
