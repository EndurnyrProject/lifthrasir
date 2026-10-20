use anyhow::Result;
use bevy::{
    asset::AssetPlugin,
    camera::Exposure,
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    text::{FontSize, FontSourceTemplate},
};
use clap::Parser;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    source::{AssetConfig, register_ro_asset_source, setup_composite_source_from_config},
};
use lifthrasir_map_editor::{LifCollisionFlags, MapDocument};
use lifthrasir_runtime::{
    RuntimeCorePlugin, WorldRenderPlugin,
    world::{LifMapData, LifPropRef, MapSceneRequest},
};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Parser)]
#[command(name = "lifthrasir-map-editor")]
struct Args {
    /// Path relative to `assets/`, used by Bevy to render the GLB.
    #[arg(long, default_value = "data/maps/prontera/prontera.glb")]
    map: String,
}

#[derive(Resource)]
struct EditorDocument(MapDocument);

#[derive(Component)]
struct EditorCamera;

#[derive(Component)]
struct CollisionCursor;

#[derive(Component)]
struct PropCursor;

#[derive(Resource, Default)]
struct SelectedProp(Option<Entity>);

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

fn main() -> Result<()> {
    let args = Args::parse();
    let document_path = PathBuf::from("assets").join(&args.map);
    let document = MapDocument::open(&document_path)?;
    let config: AssetConfig = toml::from_str(&std::fs::read_to_string("assets/loader.toml")?)?;
    let source = setup_composite_source_from_config(&config)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let map_path = args
        .map
        .strip_prefix("data/")
        .unwrap_or(&args.map)
        .to_string();

    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app
        .insert_resource(EditorDocument(document))
        .insert_resource(MapAssetPath(map_path))
        .insert_resource(CursorCell::default())
        .init_resource::<SelectedProp>()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Cargo runs a package binary from its package directory;
                    // map content remains at the workspace-level assets root.
                    file_path: "../../assets".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Lifthrasir Map Editor — arrows select; Space toggles; Ctrl+Z/Y undo/redo; Ctrl+S saves".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                move_camera,
                look_camera,
                move_cursor,
                select_cursor_with_mouse,
                toggle_cursor_cell,
                undo_redo,
                refresh_collision_overlay,
                apply_map_lighting,
                move_selected_prop,
                sync_prop_cursor,
                save_document,
            ),
        )
        ;
    app.add_plugins((
        bevy::picking::mesh_picking::MeshPickingPlugin,
        ContentAssetsPlugin,
        RuntimeCorePlugin,
        WorldRenderPlugin,
    ));
    app.add_observer(select_prop);
    app.run();
    Ok(())
}

/// The client normally applies these values during map activation. The editor
/// loads the shared map runtime directly, so it owns the same final lighting
/// step to keep prop shading faithful to the converted map.
fn apply_map_lighting(
    mut commands: Commands,
    maps: Query<&LifMapData, Added<LifMapData>>,
    mut cameras: Query<(&mut Transform, &mut Exposure), With<EditorCamera>>,
) {
    for data in &maps {
        let width = data.altitude.width as f32 * 5.0;
        let height = data.altitude.height as f32 * 5.0;
        let center = Vec3::new(width * 0.5, 0.0, -height * 0.5);
        let distance = width.max(height).max(50.0);
        for (mut transform, mut exposure) in &mut cameras {
            *transform = Transform::from_translation(center + Vec3::new(0.0, distance, distance))
                .looking_at(center, Vec3::Y);
            exposure.ev100 = data.meta.exposure_ev100;
        }
        commands.insert_resource(GlobalAmbientLight {
            color: Color::srgb(
                data.meta.ambient_color[0],
                data.meta.ambient_color[1],
                data.meta.ambient_color[2],
            ),
            brightness: data.meta.ambient_brightness,
            affects_lightmapped_meshes: false,
        });
    }
}

/// Selects the owning map-prop node for any clicked mesh inside its spawned
/// model scene. The model meshes themselves are children several levels below
/// the `LifPropRef`, hence the explicit hierarchy walk.
fn select_prop(
    click: On<Pointer<Click>>,
    parents: Query<&ChildOf>,
    props: Query<(), With<LifPropRef>>,
    mut selected: ResMut<SelectedProp>,
) {
    let mut entity = click.entity;
    loop {
        if props.contains(entity) {
            selected.0 = Some(entity);
            return;
        }
        let Ok(parent) = parents.get(entity) else {
            return;
        };
        entity = parent.parent();
    }
}

fn sync_prop_cursor(
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    mut cursors: Query<(&mut Transform, &mut Visibility), With<PropCursor>>,
) {
    let Ok((mut cursor, mut visibility)) = cursors.single_mut() else {
        return;
    };
    let Some(entity) = selected.0 else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok(transform) = props.get(entity) else {
        *visibility = Visibility::Hidden;
        return;
    };
    cursor.translation = transform.translation();
    *visibility = Visibility::Visible;
}

/// Temporary keyboard transform tool while the dedicated gizmo widget is
/// built: I/K move forward/back in map Z, J/L move X, U/O move Y, Q/E rotate.
fn move_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    selected: Res<SelectedProp>,
    mut props: Query<&mut Transform, With<LifPropRef>>,
) {
    let Some(entity) = selected.0 else {
        return;
    };
    let Ok(mut transform) = props.get_mut(entity) else {
        return;
    };
    let distance = 20.0 * time.delta_secs();
    if keys.pressed(KeyCode::KeyI) {
        transform.translation.z -= distance;
    }
    if keys.pressed(KeyCode::KeyK) {
        transform.translation.z += distance;
    }
    if keys.pressed(KeyCode::KeyJ) {
        transform.translation.x -= distance;
    }
    if keys.pressed(KeyCode::KeyL) {
        transform.translation.x += distance;
    }
    if keys.pressed(KeyCode::KeyU) {
        transform.translation.y += distance;
    }
    if keys.pressed(KeyCode::KeyO) {
        transform.translation.y -= distance;
    }
    if keys.pressed(KeyCode::KeyQ) {
        transform.rotate_y(time.delta_secs());
    }
    if keys.pressed(KeyCode::KeyE) {
        transform.rotate_y(-time.delta_secs());
    }
}

#[derive(Resource)]
struct MapAssetPath(String);

#[derive(Resource, Default)]
struct CursorCell {
    x: u32,
    y: u32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map_path: Res<MapAssetPath>,
) {
    commands.spawn((
        Camera3d::default(),
        EditorCamera,
        Transform::from_xyz(600.0, 500.0, 700.0).looking_at(Vec3::new(600.0, 0.0, -600.0), Vec3::Y),
    ));
    commands.spawn(MapSceneRequest {
        path: format!("ro://{}", map_path.0),
    });
    commands.spawn((
        CollisionCursor,
        Mesh3d(meshes.add(Cuboid::new(5.0, 5.0, 5.0))),
        MeshMaterial3d(materials.add(Color::srgb(1.0, 0.1, 0.1))),
        Transform::from_xyz(2.5, 2.5, -2.5),
    ));
    commands.spawn((
        PropCursor,
        Visibility::Hidden,
        Mesh3d(meshes.add(Cuboid::new(12.0, 12.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.9, 0.05, 0.45),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::default(),
    ));
    commands.spawn_scene(command_overlay());
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

fn command_overlay() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            right: px(16),
            width: px(310),
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(Color::srgba(0.02, 0.025, 0.04, 0.88))
        BorderColor::all(Color::srgb(0.35, 0.45, 0.7))
        Pickable::IGNORE
        Children [
            (
                Text("Camera\nWASD - move camera\nRMB - look\nWheel  altitude\n\nCollision\nLMB / arrows - select cell\nSpace - toggle walkability\nCtrl+Z / Ctrl+Y - undo / redo\nCtrl+S - save\n\nProps\nClick a prop - select\nI/K, J/L, U/O - translate\nQ/E - rotate")
                TextFont {
                    font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
                    font_size: FontSize::Px(14.0),
                }
                TextColor(Color::srgb(0.9, 0.93, 1.0))
                Pickable::IGNORE
            )
        ]
    }
}

fn move_camera(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut cameras: Query<&mut Transform, With<EditorCamera>>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    let speed = 300.0 * time.delta_secs();
    // Flight-style movement: W/S follow the full camera forward vector, so
    // looking down and moving forward descends toward the terrain.
    let forward = transform.rotation * Vec3::NEG_Z;
    let right = transform.rotation * Vec3::X;
    if keys.pressed(KeyCode::KeyW) {
        transform.translation += forward * speed;
    }
    if keys.pressed(KeyCode::KeyS) {
        transform.translation -= forward * speed;
    }
    if keys.pressed(KeyCode::KeyA) {
        transform.translation -= right * speed;
    }
    if keys.pressed(KeyCode::KeyD) {
        transform.translation += right * speed;
    }
}

fn look_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut cameras: Query<&mut Transform, With<EditorCamera>>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    if buttons.pressed(MouseButton::Right) {
        let delta: Vec2 = motion.read().map(|event| event.delta).sum();
        transform.rotate_y(-delta.x * 0.005);
        transform.rotate_local_x(-delta.y * 0.005);
    } else {
        motion.clear();
    }
    for event in wheel.read() {
        transform.translation.y = (transform.translation.y - event.y * 25.0).max(10.0);
    }
}

fn toggle_cursor_cell(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<CursorCell>,
) {
    if !keys.just_pressed(KeyCode::Space) {
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
        return;
    }
}

fn move_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    document: Res<EditorDocument>,
    mut cursor: ResMut<CursorCell>,
    mut markers: Query<&mut Transform, With<CollisionCursor>>,
) {
    let collision = document.0.editor.collision;
    let previous = (cursor.x, cursor.y);
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
    if previous == (cursor.x, cursor.y) {
        return;
    }
    let Some(cell) = document.0.collision_cell(cursor.x, cursor.y) else {
        return;
    };
    let Ok(mut transform) = markers.single_mut() else {
        return;
    };
    transform.translation = Vec3::new(
        cursor.x as f32 * 5.0 + 2.5,
        1.5 - cell.height.iter().sum::<f32>() / 4.0,
        -(cursor.y as f32 * 5.0 + 2.5),
    );
}

fn select_cursor_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_events: MessageReader<CursorMoved>,
    document: Res<EditorDocument>,
    mut cursor: ResMut<CursorCell>,
    cameras: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut markers: Query<&mut Transform, With<CollisionCursor>>,
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
    let (x, y) = ((hit.x / 5.0).floor() as i32, (-hit.z / 5.0).floor() as i32);
    let collision = document.0.editor.collision;
    if x < 0 || y < 0 || x >= collision.width as i32 || y >= collision.height as i32 {
        return;
    }
    cursor.x = x as u32;
    cursor.y = y as u32;
    let Some(cell) = document.0.collision_cell(cursor.x, cursor.y) else {
        return;
    };
    let Ok(mut transform) = markers.single_mut() else {
        return;
    };
    transform.translation = Vec3::new(
        cursor.x as f32 * 5.0 + 2.5,
        1.5 - cell.height.iter().sum::<f32>() / 4.0,
        -(cursor.y as f32 * 5.0 + 2.5),
    );
}

fn save_document(keys: Res<ButtonInput<KeyCode>>, mut document: ResMut<EditorDocument>) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !control || !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    if let Err(error) = document.0.save() {
        error!("saving map: {error:#}");
    }
}

fn undo_redo(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<CursorCell>,
    mut markers: Query<&mut Transform, With<CollisionCursor>>,
) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !control {
        return;
    }
    let changed = if keys.just_pressed(KeyCode::KeyZ) {
        document.0.undo()
    } else if keys.just_pressed(KeyCode::KeyY) {
        document.0.redo()
    } else {
        false
    };
    if !changed {
        return;
    }
    let Some(cell) = document.0.collision_cell(cursor.x, cursor.y) else {
        return;
    };
    let Ok(mut transform) = markers.single_mut() else {
        return;
    };
    transform.translation.y = 1.5 - cell.height.iter().sum::<f32>() / 4.0;
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
                    x as f32 * 5.0 + 2.5,
                    // Collision and visual-terrain heights are independent.
                    // This is an intentionally flat, unmistakable editor
                    // layer rather than an attempt to merge the two grids.
                    6.0,
                    -(y as f32 * 5.0 + 2.5),
                ),
                ChildOf(root),
            ));
        }
    }
}
