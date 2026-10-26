//! The Place tool: a ghost of the browser asset follows the terrain under the
//! cursor; a click adds it as one undoable object node under the map node.

use super::{TerrainCursor, ToolMode, return_to_move};
use crate::{
    EditorOnly,
    map::{layout::MapLayout, open::EditorScreen},
    shell::{
        browser::AssetRef,
        node::{EditorNodeId, EditorNodeIds, NodePayload, NodeSnapshot},
        selection::Selection,
        ui::{viewport_has_keyboard, viewport_has_pointer},
        undo::{SpawnNode, commit},
    },
};
use bevy::{light::NotShadowCaster, prelude::*, window::PrimaryWindow};
use lifthrasir_data::lif::{LifAudio, LifEffect, LifProp};
use lifthrasir_runtime::world::{
    EmitterOutput, LifEffectEmitter, LifPropRef, PropSceneContext, SceneOwner,
    emitters::GltfMapEffectSpawned, terrain::RO_UNITS_PER_CELL,
};
use std::path::Path;

const PROP_ANIM_TYPE: u32 = 0;
const PROP_ANIM_SPEED: f32 = 1.0;
const SOUND_VOLUME: f32 = 1.0;
const SOUND_RANGE: f32 = 50.0;
const SOUND_CYCLE: f32 = 4.0;
const EFFECT_EMIT_SPEED: f32 = 1.0;
const SOUND_GHOST_RADIUS: f32 = 1.0;
/// A right press and release closer than this (logical px) is a click, not
/// an orbit drag.
const RIGHT_CLICK_SLOP: f32 = 4.0;

/// The ghost of the asset being placed. `cell` is the GAT cell an effect
/// ghost last spawned its output in.
#[derive(Component)]
struct PlaceGhost {
    asset: AssetRef,
    cell: Option<IVec2>,
}

pub(super) fn plugin(app: &mut App) {
    let placing = |mode: Res<ToolMode>| matches!(*mode, ToolMode::Place(_));
    let tool_active = |mode: Res<ToolMode>| *mode != ToolMode::Select;
    app.add_systems(
        Update,
        (
            (swap_ghost, follow_cursor.run_if(placing)).chain(),
            leave_on_escape.run_if(tool_active.and_then(viewport_has_keyboard)),
            leave_on_right_click.run_if(tool_active.and_then(viewport_has_pointer)),
        )
            .run_if(in_state(EditorScreen::Editing)),
    )
    .add_systems(
        OnExit(EditorScreen::Editing),
        (leave_place_tool, despawn_ghosts),
    );
}

/// Models, sounds and effects can be placed; ground textures cannot.
pub fn is_placeable(asset: &AssetRef) -> bool {
    !matches!(asset, AssetRef::GroundTexture(_))
}

fn file_stem(uri: &str) -> String {
    Path::new(uri)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn default_prop(uri: &str) -> LifProp {
    LifProp {
        model: uri.to_string(),
        anim_type: PROP_ANIM_TYPE,
        anim_speed: PROP_ANIM_SPEED,
    }
}

fn default_effect(id: u32) -> LifEffect {
    LifEffect {
        name: format!("effect_{id}"),
        effect_type: id,
        emit_speed: EFFECT_EMIT_SPEED,
        params: [0.0; 4],
    }
}

/// The node `asset` becomes at world point `at`, with the default payload.
/// `None` for assets that cannot be placed.
fn snapshot(asset: &AssetRef, id: EditorNodeId, at: Vec3) -> Option<NodeSnapshot> {
    let (name, payload) = match asset {
        AssetRef::Model(uri) => (file_stem(uri), NodePayload::Prop(default_prop(uri))),
        AssetRef::Sound(uri) => (
            file_stem(uri),
            NodePayload::Sound(LifAudio {
                name: file_stem(uri),
                file: uri.clone(),
                volume: SOUND_VOLUME,
                range: SOUND_RANGE,
                cycle: SOUND_CYCLE,
            }),
        ),
        AssetRef::Effect(id) => {
            let effect = default_effect(*id);
            (effect.name.clone(), NodePayload::Effect(effect))
        }
        AssetRef::GroundTexture(_) => return None,
    };
    Some(NodeSnapshot {
        id,
        name,
        transform: Transform::from_translation(at),
        payload,
    })
}

/// Adds `asset` at world point `at` (the map node is identity) as one
/// undoable step, selects the new node and returns to the Select tool with
/// the Move gizmo.
pub fn place(world: &mut World, asset: &AssetRef, at: Vec3) {
    let Some(layout) = world.get_resource::<MapLayout>().cloned() else {
        return;
    };
    if !is_placeable(asset) {
        return;
    }
    let id = world.resource_mut::<EditorNodeIds>().allocate();
    let Some(snapshot) = snapshot(asset, id, at) else {
        return;
    };
    commit(
        world,
        Box::new(SpawnNode::new(layout.root, layout.map_node, snapshot, None)),
    );
    world.resource_mut::<Selection>().replace([id]);
    return_to_move(world);
}

/// Keeps exactly one ghost, for the asset being placed, and none otherwise.
fn swap_ghost(
    mut commands: Commands,
    mode: Res<ToolMode>,
    ghosts: Query<(Entity, &PlaceGhost)>,
    layout: Option<Res<MapLayout>>,
    contexts: Query<&PropSceneContext>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let wanted = match &*mode {
        ToolMode::Place(asset) => Some(asset),
        _ => None,
    };
    let mut alive = false;
    for (entity, ghost) in &ghosts {
        if Some(&ghost.asset) == wanted {
            alive = true;
            continue;
        }
        commands.entity(entity).despawn();
    }
    let Some(asset) = wanted.filter(|_| !alive) else {
        return;
    };
    let mut ghost = commands.spawn((
        Name::new("place ghost"),
        PlaceGhost {
            asset: asset.clone(),
            cell: None,
        },
        EditorOnly,
        Transform::default(),
        Visibility::Hidden,
    ));
    match asset {
        AssetRef::Model(uri) => {
            let context = layout
                .and_then(|layout| contexts.get(layout.root).ok().copied())
                .unwrap_or_default();
            ghost.insert((LifPropRef(default_prop(uri)), context));
        }
        AssetRef::Effect(id) => {
            // Spawned marker on: no output until the ghost is over the terrain.
            let entity = ghost.id();
            ghost.insert((
                LifEffectEmitter(default_effect(*id)),
                SceneOwner(entity),
                GltfMapEffectSpawned,
            ));
        }
        AssetRef::Sound(_) => {
            ghost.insert((
                Mesh3d(meshes.add(Sphere::new(SOUND_GHOST_RADIUS))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.2, 0.4, 1.0, 0.6),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                NotShadowCaster,
            ));
        }
        AssetRef::GroundTexture(_) => {}
    }
}

/// Identifies the GAT cell under `at`; only compared, so Z stays unflipped.
fn gat_cell(at: Vec3) -> IVec2 {
    (at.xz() / RO_UNITS_PER_CELL).floor().as_ivec2()
}

/// Moves the ghost to the terrain under the cursor and places the asset on a
/// left press. Off the terrain the ghost hides and an effect ghost's output
/// is despawned (it keeps its spawned marker, so nothing respawns); entering
/// a new GAT cell respawns the output there.
fn follow_cursor(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: TerrainCursor,
    mut ghosts: Query<(Entity, &mut PlaceGhost, &mut Transform, &mut Visibility)>,
    outputs: Query<(Entity, &EmitterOutput)>,
) {
    let hit = cursor.hit();
    for (entity, mut ghost, mut transform, mut visibility) in &mut ghosts {
        let Some(at) = hit else {
            visibility.set_if_neq(Visibility::Hidden);
            if ghost.cell.take().is_some() {
                for (output, _) in outputs.iter().filter(|(_, source)| source.0 == entity) {
                    commands.entity(output).despawn();
                }
            }
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        transform.translation = at;
        let cell = Some(gat_cell(at));
        if matches!(ghost.asset, AssetRef::Effect(_)) && ghost.cell != cell {
            ghost.cell = cell;
            commands.entity(entity).remove::<GltfMapEffectSpawned>();
        }
        if mouse.just_pressed(MouseButton::Left) {
            let asset = ghost.asset.clone();
            commands.queue(move |world: &mut World| place(world, &asset, at));
        }
    }
}

fn leave_place_tool(mut mode: ResMut<ToolMode>) {
    mode.set_if_neq(ToolMode::Select);
}

fn despawn_ghosts(mut commands: Commands, ghosts: Query<Entity, With<PlaceGhost>>) {
    for ghost in &ghosts {
        commands.entity(ghost).despawn();
    }
}

fn leave_on_escape(keys: Res<ButtonInput<KeyCode>>, mode: ResMut<ToolMode>) {
    if keys.just_pressed(KeyCode::Escape) {
        leave_place_tool(mode);
    }
}

/// A right click (not an orbit drag) leaves the tool.
fn leave_on_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut pressed_at: Local<Option<Vec2>>,
    mode: ResMut<ToolMode>,
) {
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    if mouse.just_pressed(MouseButton::Right) {
        *pressed_at = cursor;
    }
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    let click = pressed_at
        .take()
        .zip(cursor)
        .is_some_and(|(start, end)| start.distance(end) < RIGHT_CLICK_SLOP);
    if click {
        leave_place_tool(mode);
    }
}
