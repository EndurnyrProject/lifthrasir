//! The ordered set of selected object nodes with one active node, selecting
//! the hovered node on press (modifier-click toggles), select all, invert,
//! delete, and the editor-only handles that make lights and emitters visible
//! and clickable.

use super::{
    gizmo::TransformGizmo,
    node::{EditorNodeId, NodeSnapshot, find_node},
    pick::{Hovered, hover},
    ui::{command_modifier, viewport_has_keyboard, viewport_has_pointer},
    undo::{Batch, DespawnNode, EditOp, commit},
};
use crate::{
    EditorOnly,
    map::{layout::MapLayout, open::EditorScreen, tools::ToolMode},
};
use bevy::{light::NotShadowCaster, prelude::*};
use lifthrasir_runtime::world::{LifAudioEmitter, LifEffectEmitter};

/// The selected object nodes in selection order, and the active one the
/// inspector and gizmo act on. The active node is always selected.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    nodes: Vec<EditorNodeId>,
    active: Option<EditorNodeId>,
}

impl Selection {
    pub fn active(&self) -> Option<EditorNodeId> {
        self.active
    }

    pub fn nodes(&self) -> &[EditorNodeId] {
        &self.nodes
    }

    pub fn contains(&self, id: EditorNodeId) -> bool {
        self.nodes.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.active = None;
    }

    /// Selects exactly `ids`, the last becoming active.
    pub fn replace(&mut self, ids: impl IntoIterator<Item = EditorNodeId>) {
        self.clear();
        self.extend(ids);
    }

    /// Adds `ids` after the current selection, the last new one becoming active.
    pub fn extend(&mut self, ids: impl IntoIterator<Item = EditorNodeId>) {
        for id in ids {
            if !self.contains(id) {
                self.nodes.push(id);
            }
            self.active = Some(id);
        }
    }

    /// Deselects `id` if selected, otherwise selects it and makes it active.
    /// Deselecting the active node activates the most recent remaining one.
    pub fn toggle(&mut self, id: EditorNodeId) {
        if self.contains(id) {
            self.remove(&[id]);
        } else {
            self.extend([id]);
        }
    }

    /// Removes `ids` from the selection, keeping the active node if it survives.
    pub fn remove(&mut self, ids: &[EditorNodeId]) {
        self.nodes.retain(|node| !ids.contains(node));
        if self.active.is_some_and(|active| ids.contains(&active)) {
            self.active = self.nodes.last().copied();
        }
    }

    /// Selects every node of `all` that is not selected now, in `all` order.
    pub fn invert(&mut self, all: &[EditorNodeId]) {
        let unselected: Vec<_> = all
            .iter()
            .copied()
            .filter(|id| !self.contains(*id))
            .collect();
        self.replace(unselected);
    }
}

/// Every selectable object node of the open map, in child order.
pub fn all_nodes(world: &World) -> Vec<EditorNodeId> {
    let Some(layout) = world.get_resource::<MapLayout>() else {
        return Vec::new();
    };
    world
        .get::<Children>(layout.map_node)
        .into_iter()
        .flatten()
        .filter(|&&child| Some(child) != layout.terrain_node)
        .filter_map(|&child| world.get::<EditorNodeId>(child).copied())
        .collect()
}

pub fn select_all(world: &mut World) {
    let all = all_nodes(world);
    world.resource_mut::<Selection>().replace(all);
}

pub fn invert_selection(world: &mut World) {
    let all = all_nodes(world);
    world.resource_mut::<Selection>().invert(&all);
}

/// Whether a modifier that extends the selection (Shift or Ctrl) is held.
pub fn extends_selection(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ])
}

/// The clickable stand-in for a light or emitter node, which has no mesh.
#[derive(Component)]
pub struct EditorHandle;

const HANDLE_RADIUS: f32 = 1.0;

/// Shared handle mesh and per-kind materials.
struct HandleAssets {
    mesh: Handle<Mesh>,
    light: Handle<StandardMaterial>,
    sound: Handle<StandardMaterial>,
    effect: Handle<StandardMaterial>,
}

type NewNode = (
    Entity,
    Option<&'static Children>,
    Has<LifAudioEmitter>,
    Has<LifEffectEmitter>,
);
type PointOrSpot = Or<(With<PointLight>, With<SpotLight>)>;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Selection>()
        .add_systems(OnExit(EditorScreen::Editing), clear_selection)
        .add_systems(
            Update,
            (
                attach_handles,
                select_on_press.after(hover).run_if(
                    in_state(EditorScreen::Editing)
                        .and_then(resource_equals(ToolMode::Select))
                        .and_then(viewport_has_pointer),
                ),
                selection_keys
                    .run_if(in_state(EditorScreen::Editing).and_then(viewport_has_keyboard)),
            ),
        );
}

fn clear_selection(mut selection: ResMut<Selection>) {
    selection.clear();
}

/// Gives every new light (point/spot) or emitter node one handle child:
/// yellow for lights, blue for sounds, magenta for effects. The sun has none;
/// it is picked by its origin or from the outliner.
fn attach_handles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<Option<HandleAssets>>,
    nodes: Query<NewNode, Added<EditorNodeId>>,
    lights: Query<(), PointOrSpot>,
) {
    for (node, children, sound, effect) in &nodes {
        let light = children.is_some_and(|children| children.iter().any(|c| lights.contains(c)));
        if !(light || sound || effect) {
            continue;
        }
        let assets = assets.get_or_insert_with(|| {
            let mut unlit = |color: Color| {
                materials.add(StandardMaterial {
                    base_color: color,
                    unlit: true,
                    ..default()
                })
            };
            HandleAssets {
                mesh: meshes.add(Sphere::new(HANDLE_RADIUS)),
                light: unlit(Color::srgb(1.0, 0.9, 0.1)),
                sound: unlit(Color::srgb(0.2, 0.4, 1.0)),
                effect: unlit(Color::srgb(1.0, 0.2, 1.0)),
            }
        });
        let material = match (sound, effect) {
            (true, _) => assets.sound.clone(),
            (_, true) => assets.effect.clone(),
            _ => assets.light.clone(),
        };
        commands.spawn((
            Name::new("editor_handle"),
            EditorHandle,
            EditorOnly,
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(material),
            NotShadowCaster,
            Transform::IDENTITY,
            ChildOf(node),
        ));
    }
}

/// The object node `entity` belongs to: the ancestor (or itself) whose parent
/// is the map node, never the terrain.
pub fn object_node(
    entity: Entity,
    parent_of: impl Fn(Entity) -> Option<Entity>,
    layout: &MapLayout,
) -> Option<Entity> {
    let mut entity = entity;
    loop {
        let parent = parent_of(entity)?;
        if parent == layout.map_node {
            return (Some(entity) != layout.terrain_node).then_some(entity);
        }
        entity = parent;
    }
}

/// A left press selects the hovered object node (Shift/Ctrl toggles it in the
/// selection). A press over nothing is left to the marquee, which clears the
/// selection on release. Presses on the transform gizmo are left to it.
fn select_on_press(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<Hovered>,
    ids: Query<&EditorNodeId>,
    gizmo: Option<Res<TransformGizmo>>,
    mut selection: ResMut<Selection>,
) {
    if !mouse.just_pressed(MouseButton::Left) || gizmo.is_some_and(|gizmo| gizmo.is_focused()) {
        return;
    }
    let Some(id) = hovered.0.and_then(|node| ids.get(node).ok().copied()) else {
        return;
    };
    if extends_selection(&keys) {
        selection.toggle(id);
    } else if selection.nodes() != [id] {
        selection.replace([id]);
    }
}

/// Escape clears the selection; Delete/Backspace deletes it; Ctrl+A selects
/// every node and Ctrl+I inverts the selection.
fn selection_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        selection.clear();
    }
    if keys.any_just_pressed([KeyCode::Delete, KeyCode::Backspace]) {
        commands.queue(delete_selected);
    }
    let ctrl = command_modifier(&keys);
    if ctrl && keys.just_pressed(KeyCode::KeyA) {
        commands.queue(select_all);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyI) {
        commands.queue(invert_selection);
    }
}

/// Deletes the selected nodes as one undoable step and deselects them.
/// Nodes a snapshot cannot restore (the sun) are refused and stay selected.
pub fn delete_selected(world: &mut World) {
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    let selected = world.resource::<Selection>().nodes().to_vec();
    let mut deleted = Vec::new();
    let mut ops: Vec<Box<dyn EditOp>> = Vec::new();
    for id in selected {
        let restorable =
            find_node(world, id).is_some_and(|node| NodeSnapshot::capture(world, node).is_some());
        if !restorable {
            warn!("node {id:?} cannot be deleted");
            continue;
        }
        deleted.push(id);
        ops.push(Box::new(DespawnNode::new(layout.root, layout.map_node, id)));
    }
    if ops.is_empty() {
        return;
    }
    commit(world, Box::new(Batch(ops)));
    world.resource_mut::<Selection>().remove(&deleted);
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: EditorNodeId = EditorNodeId(1);
    const B: EditorNodeId = EditorNodeId(2);
    const C: EditorNodeId = EditorNodeId(3);

    #[test]
    fn toggling_keeps_order_and_moves_the_active_node() {
        let mut selection = Selection::default();
        selection.extend([A, B, C]);
        assert_eq!(selection.active(), Some(C));

        selection.toggle(C);
        assert_eq!(selection.nodes(), [A, B]);
        assert_eq!(selection.active(), Some(B));

        selection.toggle(A);
        selection.toggle(A);
        assert_eq!(selection.nodes(), [B, A]);
        assert_eq!(selection.active(), Some(A));

        selection.toggle(A);
        selection.toggle(B);
        assert!(selection.is_empty());
        assert_eq!(selection.active(), None);
    }

    #[test]
    fn invert_selects_exactly_the_unselected_nodes() {
        let mut selection = Selection::default();
        selection.replace([B]);

        selection.invert(&[A, B, C]);

        assert_eq!(selection.nodes(), [A, C]);
        assert_eq!(selection.active(), Some(C));
        selection.invert(&[A, B, C]);
        assert_eq!(selection.nodes(), [B]);
    }
}
