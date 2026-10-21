//! The selected object node, click picking, delete, and the editor-only
//! handles that make lights and emitters clickable.

use super::{
    node::{EditorNodeId, NodeSnapshot, find_node},
    ui::{viewport_has_keyboard, viewport_has_pointer},
    undo::{DespawnNode, commit},
};
use crate::{
    EditorOnly,
    map::{layout::MapLayout, open::EditorScreen, tools::ToolMode},
};
use bevy::{light::NotShadowCaster, picking::events::Click, prelude::*};
use lifthrasir_runtime::world::{LifAudioEmitter, LifEffectEmitter};

/// The selected object node, if any.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection(pub Option<EditorNodeId>);

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
        .add_message::<Pointer<Click>>()
        .add_systems(OnExit(EditorScreen::Editing), clear_selection)
        .add_systems(
            Update,
            (
                attach_handles,
                select_on_click.run_if(
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
    selection.0 = None;
}

/// Gives every new light (point/spot) or emitter node one handle child:
/// yellow for lights, blue for sounds, magenta for effects. The sun is
/// selected from the inspector instead.
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

/// A primary click selects the clicked object node, or clears the selection
/// when it hits anything else.
fn select_on_click(
    mut clicks: MessageReader<Pointer<Click>>,
    parents: Query<&ChildOf>,
    ids: Query<&EditorNodeId>,
    layout: Option<Res<MapLayout>>,
    mut selection: ResMut<Selection>,
) {
    let Some(layout) = layout else {
        clicks.clear();
        return;
    };
    for click in clicks
        .read()
        .filter(|click| click.event.button == PointerButton::Primary)
    {
        let node = object_node(
            click.entity,
            |entity| parents.get(entity).ok().map(ChildOf::parent),
            &layout,
        );
        selection.set_if_neq(Selection(node.and_then(|node| ids.get(node).ok().copied())));
    }
}

/// Escape clears the selection; Delete/Backspace deletes the selected node.
fn selection_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        selection.0 = None;
    }
    if keys.any_just_pressed([KeyCode::Delete, KeyCode::Backspace]) {
        commands.queue(delete_selected);
    }
}

/// Deletes the selected node as one undoable step and clears the selection.
/// Refuses nodes a snapshot cannot restore (the sun).
pub fn delete_selected(world: &mut World) {
    let Some(id) = world.resource::<Selection>().0 else {
        return;
    };
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    let Some(node) = find_node(world, id) else {
        return;
    };
    if NodeSnapshot::capture(world, node).is_none() {
        warn!("node {id:?} cannot be deleted");
        return;
    }
    commit(
        world,
        Box::new(DespawnNode::new(layout.root, layout.map_node, id)),
    );
    world.resource_mut::<Selection>().0 = None;
}
