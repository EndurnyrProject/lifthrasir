use bevy::prelude::*;
use lifthrasir_map_editor::LifCollisionFlags;

use super::viewport::{CELL_SIZE, EditorCamera};
use crate::application::{EditorDocument, EditorSystems};

#[derive(Component)]
struct CollisionCursor;

#[derive(Resource, Default)]
struct CursorCell {
    x: u32,
    y: u32,
}

#[derive(Component)]
struct CollisionOverlay {
    center: Option<(u32, u32)>,
}

#[derive(Resource)]
struct OverlayAssets {
    mesh: Handle<Mesh>,
    walkable: Handle<StandardMaterial>,
    blocked: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
}

pub(super) struct CollisionEditorPlugin;

impl Plugin for CollisionEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorCell>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (move_cursor, select_cursor_with_mouse, toggle_cursor_cell)
                    .chain()
                    .in_set(EditorSystems::Edit),
            )
            .add_systems(
                Update,
                (sync_cursor, refresh_collision_overlay).in_set(EditorSystems::Present),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        CollisionCursor,
        Mesh3d(meshes.add(Cuboid::new(CELL_SIZE, CELL_SIZE, CELL_SIZE))),
        MeshMaterial3d(materials.add(Color::srgb(1.0, 0.1, 0.1))),
        Transform::from_xyz(2.5, 2.5, -2.5),
    ));
    commands.insert_resource(OverlayAssets {
        mesh: meshes.add(Cuboid::new(4.7, 0.15, 4.7)),
        walkable: materials.add(overlay_material(Color::srgb(0.1, 0.95, 0.2))),
        blocked: materials.add(overlay_material(Color::srgb(1.0, 0.08, 0.08))),
        water: materials.add(overlay_material(Color::srgb(0.05, 0.35, 1.0))),
    });
    commands.spawn((CollisionOverlay { center: None }, Transform::default()));
}

fn overlay_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn toggle_cursor_cell(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<CursorCell>,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(mut cell) = document.0.collision_cell(cursor.x, cursor.y) else {
        return;
    };
    cell.flags = if cell.flags.contains(LifCollisionFlags::WALKABLE) {
        LifCollisionFlags::empty()
    } else {
        LifCollisionFlags::WALKABLE
    };
    if let Err(error) = document.0.set_collision_cell(cursor.x, cursor.y, cell) {
        error!("editing collision: {error:#}");
    }
}

fn move_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    document: Res<EditorDocument>,
    mut cursor: ResMut<CursorCell>,
) {
    let collision = document.0.editor.collision;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        cursor.x = cursor.x.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        cursor.x = (cursor.x + 1).min(collision.width - 1);
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        cursor.y = cursor.y.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        cursor.y = (cursor.y + 1).min(collision.height - 1);
    }
}

fn select_cursor_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_events: MessageReader<CursorMoved>,
    document: Res<EditorDocument>,
    mut cursor: ResMut<CursorCell>,
    cameras: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) {
    let Some(position) = cursor_events.read().last().map(|event| event.position) else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, position) else {
        return;
    };
    if ray.direction.y.abs() < f32::EPSILON {
        return;
    }
    let distance = -ray.origin.y / ray.direction.y;
    if distance < 0.0 {
        return;
    }
    let hit = ray.origin + *ray.direction * distance;
    let (x, y) = (
        (hit.x / CELL_SIZE).floor() as i32,
        (-hit.z / CELL_SIZE).floor() as i32,
    );
    let collision = document.0.editor.collision;
    if x < 0 || y < 0 || x >= collision.width as i32 || y >= collision.height as i32 {
        return;
    }
    cursor.x = x as u32;
    cursor.y = y as u32;
}

/// Document/history synchronization is separate from input, including undo of
/// a collision edit while a prop is selected.
fn sync_cursor(
    document: Res<EditorDocument>,
    cursor: Res<CursorCell>,
    mut markers: Query<&mut Transform, With<CollisionCursor>>,
) {
    if !document.is_changed() && !cursor.is_changed() {
        return;
    }
    let Some(cell) = document.0.collision_cell(cursor.x, cursor.y) else {
        return;
    };
    let Ok(mut transform) = markers.single_mut() else {
        return;
    };
    transform.translation = Vec3::new(
        cursor.x as f32 * CELL_SIZE + CELL_SIZE * 0.5,
        1.5 - cell.height.iter().sum::<f32>() / 4.0,
        -(cursor.y as f32 * CELL_SIZE + CELL_SIZE * 0.5),
    );
}

fn refresh_collision_overlay(
    mut commands: Commands,
    document: Res<EditorDocument>,
    cursor: Res<CursorCell>,
    assets: Res<OverlayAssets>,
    mut overlays: Query<(Entity, &mut CollisionOverlay, &Children)>,
) {
    let Ok((root, mut overlay, children)) = overlays.single_mut() else {
        return;
    };
    let center = (cursor.x, cursor.y);
    if overlay.center == Some(center) && !document.is_changed() {
        return;
    }
    for child in children.iter() {
        commands.entity(child).despawn();
    }
    overlay.center = Some(center);

    const RADIUS: i32 = 8;
    let collision = document.0.editor.collision;
    for y in (center.1 as i32 - RADIUS)..=(center.1 as i32 + RADIUS) {
        for x in (center.0 as i32 - RADIUS)..=(center.0 as i32 + RADIUS) {
            if x < 0 || y < 0 || x >= collision.width as i32 || y >= collision.height as i32 {
                continue;
            }
            let Some(cell) = document.0.collision_cell(x as u32, y as u32) else {
                continue;
            };
            let material = if cell.flags.contains(LifCollisionFlags::WATER) {
                assets.water.clone()
            } else if cell.flags.contains(LifCollisionFlags::WALKABLE) {
                assets.walkable.clone()
            } else {
                assets.blocked.clone()
            };
            commands.spawn((
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(
                    x as f32 * CELL_SIZE + CELL_SIZE * 0.5,
                    // Collision and visual terrain heights are independent.
                    6.0,
                    -(y as f32 * CELL_SIZE + CELL_SIZE * 0.5),
                ),
                ChildOf(root),
            ));
        }
    }
}
