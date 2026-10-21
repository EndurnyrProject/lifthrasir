//! The outliner: every terrain chunk, prop, sound, effect and light of the
//! open map. Clicking an entry selects it (object nodes) and points the
//! camera at it.

use super::{
    camera::{EditorCamera, focus_on},
    node::{EditorNodeId, KindFlags, LightFlags, NodeKind, node_kind},
    pick::node_bounds,
    selection::Selection,
    ui::EditorUiSet,
};
use crate::map::{layout::MapLayout, open::EditorScreen};
use bevy::{gltf::GltfMaterialName, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use lifthrasir_runtime::world::LifPropRef;

/// Camera distance per unit of the target's bounds diagonal.
const FRAMING: f32 = 1.5;
/// Camera distance for targets without bounds or smaller than this.
const MIN_FRAMING_DISTANCE: f32 = 40.0;

/// Whether the outliner window is open, and its filter.
#[derive(Resource)]
pub struct Outliner {
    pub open: bool,
    filter: String,
}

impl Default for Outliner {
    fn default() -> Self {
        Self {
            open: true,
            filter: String::new(),
        }
    }
}

/// One row: the entity to go to, its label, a hover hint and whether it is
/// the selected node.
struct Entry {
    entity: Entity,
    label: String,
    hint: Option<String>,
    selected: bool,
}

type NodeRow<'a> = (
    &'a EditorNodeId,
    Option<&'a Name>,
    Option<&'a LifPropRef>,
    KindFlags<'a>,
);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Outliner>().add_systems(
        EguiPrimaryContextPass,
        outliner_window
            .in_set(EditorUiSet::Windows)
            .run_if(in_state(EditorScreen::Editing)),
    );
}

/// Selects `entity` (clears the selection for anything that is not an object
/// node, like a terrain chunk) and points the camera at it from a distance
/// that fits its bounds, keeping the camera's rotation.
pub fn go_to(world: &mut World, entity: Entity) {
    let id = world.get::<EditorNodeId>(entity).copied();
    world.resource_mut::<Selection>().0 = id;
    let Some(origin) = world
        .get::<GlobalTransform>(entity)
        .map(|g| g.translation())
    else {
        return;
    };
    let bounds = world
        .run_system_cached_with(node_bounds, entity)
        .ok()
        .flatten();
    let (target, size) = bounds.map_or((origin, Vec3::ZERO), |(min, max)| {
        ((min + max) * 0.5, max - min)
    });
    let distance = (size.length() * FRAMING).max(MIN_FRAMING_DISTANCE);
    let mut cameras = world.query::<(&mut Transform, &mut EditorCamera)>();
    for (mut transform, mut editor) in cameras.iter_mut(world) {
        focus_on(&mut transform, &mut editor, target, distance);
    }
}

#[allow(clippy::too_many_arguments)]
fn outliner_window(
    mut contexts: EguiContexts,
    mut state: ResMut<Outliner>,
    mut commands: Commands,
    layout: Option<Res<MapLayout>>,
    selection: Res<Selection>,
    children: Query<&Children>,
    nodes: Query<NodeRow<'static>>,
    lights: Query<LightFlags>,
    materials: Query<&GltfMaterialName>,
) -> Result {
    let Some(layout) = layout.filter(|_| state.open) else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let state = &mut *state;
    let filter = state.filter.to_lowercase();
    let mut groups = [
        ("Terrain", Vec::new()),
        ("Props", Vec::new()),
        ("Sounds", Vec::new()),
        ("Effects", Vec::new()),
        ("Lights", Vec::new()),
    ];
    let chunks = layout
        .terrain_node
        .and_then(|terrain| children.get(terrain).ok())
        .into_iter()
        .flatten();
    for (index, &chunk) in chunks.enumerate() {
        let texture = materials.get(chunk).map_or("?", |name| name.0.as_str());
        groups[0].1.push(Entry {
            entity: chunk,
            label: format!("chunk {index}: {texture}"),
            hint: None,
            selected: false,
        });
    }
    for &node in children.get(layout.map_node).into_iter().flatten() {
        let Ok((id, name, prop, flags)) = nodes.get(node) else {
            continue;
        };
        let Some(kind) = node_kind(flags, &lights) else {
            continue;
        };
        let group = match kind {
            NodeKind::Prop => 1,
            NodeKind::Sound => 2,
            NodeKind::Effect => 3,
            NodeKind::PointLight | NodeKind::SpotLight | NodeKind::Sun => 4,
        };
        groups[group].1.push(Entry {
            entity: node,
            label: name.map_or_else(|| format!("{kind:?}"), Name::to_string),
            hint: prop.map(|prop| prop.0.model.clone()),
            selected: selection.0 == Some(*id),
        });
    }
    let mut clicked = None;
    egui::Window::new("Outliner")
        .open(&mut state.open)
        .default_pos([280.0, 80.0])
        .default_size([260.0, 420.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.text_edit_singleline(&mut state.filter);
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (title, entries) in &groups {
                    let shown: Vec<&Entry> = entries
                        .iter()
                        .filter(|entry| entry.label.to_lowercase().contains(&filter))
                        .collect();
                    egui::CollapsingHeader::new(format!("{title} ({})", shown.len()))
                        .id_salt(*title)
                        .default_open(*title != "Terrain")
                        .show(ui, |ui| {
                            for entry in shown {
                                let mut row = ui.selectable_label(entry.selected, &entry.label);
                                if let Some(hint) = &entry.hint {
                                    row = row.on_hover_text(hint);
                                }
                                if row.clicked() {
                                    clicked = Some(entry.entity);
                                }
                            }
                        });
                }
            });
        });
    if let Some(entity) = clicked {
        commands.queue(move |world: &mut World| go_to(world, entity));
    }
    Ok(())
}
