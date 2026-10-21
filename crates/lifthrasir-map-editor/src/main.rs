use anyhow::Result;
use bevy::{
    asset::AssetPlugin,
    camera::Exposure,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
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
    world::{LifMapData, LifMapNodeIndex, LifPropRef, MapSceneRequest, PropSceneContext},
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

#[derive(Resource, Default)]
struct EditorNotice {
    message: String,
    expires: Option<Timer>,
}

impl EditorNotice {
    fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.expires = Some(Timer::from_seconds(3.0, TimerMode::Once));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GizmoMode {
    #[default]
    Translate,
    Rotate,
}

#[derive(Resource, Default)]
struct PropGizmo {
    mode: GizmoMode,
    drag: Option<(Entity, Entity)>,
}

#[derive(Component)]
struct PropGizmoRoot;

#[derive(Component, Clone, Copy)]
struct GizmoAxis {
    axis: Vec3,
}

#[derive(Component, Default, Clone)]
struct NoticeLabel;

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
    app.insert_resource(EditorDocument(document))
        .insert_resource(MapAssetPath(map_path))
        .insert_resource(CursorCell::default())
        .init_resource::<SelectedProp>()
        .init_resource::<EditorNotice>()
        .init_resource::<PropGizmo>()
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
                        title: "Lifthrasir Map Editor".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(FreeCameraPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                move_cursor,
                select_cursor_with_mouse,
                toggle_cursor_cell,
                undo_redo,
                refresh_collision_overlay,
                apply_map_lighting,
                switch_gizmo_mode,
                move_selected_prop,
                duplicate_selected_prop,
                delete_selected_prop,
                sync_prop_gizmo,
                draw_prop_gizmo,
                sync_prop_cursor,
                update_notice,
                save_document,
            ),
        );
    app.add_plugins((
        bevy::picking::mesh_picking::MeshPickingPlugin,
        ContentAssetsPlugin,
        RuntimeCorePlugin,
        WorldRenderPlugin,
    ));
    app.add_observer(select_prop);
    app.add_observer(gizmo_drag_start);
    app.add_observer(gizmo_drag);
    app.add_observer(gizmo_drag_end);
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

fn switch_gizmo_mode(keys: Res<ButtonInput<KeyCode>>, mut gizmo: ResMut<PropGizmo>) {
    if keys.just_pressed(KeyCode::KeyG) {
        gizmo.mode = GizmoMode::Translate;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        gizmo.mode = GizmoMode::Rotate;
    }
}

/// Keyboard fine-control remains available alongside the mouse gizmo.
/// I/K move along Z, J/L along X, U/O along Y, and Q/E rotate around Y.
fn move_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    selected: Res<SelectedProp>,
    mut document: ResMut<EditorDocument>,
    mut props: Query<(&mut Transform, &LifMapNodeIndex), With<LifPropRef>>,
) {
    let Some(entity) = selected.0 else { return };
    let moving = keys.any_pressed([
        KeyCode::KeyI,
        KeyCode::KeyK,
        KeyCode::KeyJ,
        KeyCode::KeyL,
        KeyCode::KeyU,
        KeyCode::KeyO,
        KeyCode::KeyQ,
        KeyCode::KeyE,
    ]);
    if !moving {
        return;
    }
    let Ok((mut transform, node)) = props.get_mut(entity) else {
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
    if let Err(error) = document.0.update_prop_transform(
        node.0,
        transform.translation.to_array(),
        transform.rotation.to_array(),
        transform.scale.to_array(),
    ) {
        error!("recording keyboard prop transform: {error:#}");
    }
}

fn duplicate_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedProp>,
    mut document: ResMut<EditorDocument>,
    props: Query<(&LifMapNodeIndex, &LifPropRef, &Transform), With<LifPropRef>>,
    mut commands: Commands,
    mut notice: ResMut<EditorNotice>,
) {
    if !keys.just_pressed(KeyCode::KeyD)
        || !keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
    {
        return;
    }
    let Some(entity) = selected.0 else { return };
    let Ok((node, prop, transform)) = props.get(entity) else {
        return;
    };
    match document.0.duplicate_prop(node.0) {
        Ok(index) => {
            let mut duplicate_transform = *transform;
            duplicate_transform.translation.x += 20.0;
            commands.spawn((
                LifPropRef(prop.0.clone()),
                LifMapNodeIndex(index),
                PropSceneContext::default(),
                duplicate_transform,
            ));
            notice.show(format!("Duplicated prop (node {index}) — Ctrl+S to save"));
        }
        Err(error) => notice.show(format!("Duplicate failed: {error}")),
    }
}

fn delete_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    selected: Res<SelectedProp>,
    mut props: Query<(&LifMapNodeIndex, &mut Visibility), With<LifPropRef>>,
    mut notice: ResMut<EditorNotice>,
) {
    if !keys.just_pressed(KeyCode::Delete) {
        return;
    }
    let Some(entity) = selected.0 else {
        return;
    };
    let Ok((node, mut visibility)) = props.get_mut(entity) else {
        return;
    };
    if let Err(error) = document.0.set_prop_deleted(node.0, true) {
        error!("deleting prop: {error:#}");
        notice.show(format!("Delete failed: {error}"));
        return;
    }
    *visibility = Visibility::Hidden;
    notice.show("Deleted prop — Ctrl+S to save");
}

fn gizmo_drag_start(
    drag: On<Pointer<DragStart>>,
    axes: Query<&GizmoAxis>,
    selected: Res<SelectedProp>,
    mut state: ResMut<PropGizmo>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Ok(_) = axes.get(drag.entity) else { return };
    let Some(prop) = selected.0 else { return };
    state.drag = Some((drag.entity, prop));
}

fn gizmo_drag(
    drag: On<Pointer<Drag>>,
    axes: Query<&GizmoAxis>,
    cameras: Query<&GlobalTransform, With<EditorCamera>>,
    mut props: Query<(&mut Transform, &LifMapNodeIndex), With<LifPropRef>>,
    mut document: ResMut<EditorDocument>,
    state: Res<PropGizmo>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some((axis_entity, prop_entity)) = state.drag else {
        return;
    };
    if axis_entity != drag.entity {
        return;
    }
    let Ok(axis) = axes.get(drag.entity) else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok((mut transform, node)) = props.get_mut(prop_entity) else {
        return;
    };
    let axis = axis.axis;
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    let screen_axis = Vec2::new(axis.dot(right), -axis.dot(up));
    let amount = if screen_axis.length_squared() > 0.001 {
        drag.delta.dot(screen_axis.normalize()) * 0.08
    } else {
        0.0
    };
    if state.mode == GizmoMode::Translate {
        transform.translation += axis * amount;
    } else {
        transform.rotation = Quat::from_axis_angle(axis, amount * 0.025) * transform.rotation;
    }
    if let Err(error) = document.0.update_prop_transform(
        node.0,
        transform.translation.to_array(),
        transform.rotation.to_array(),
        transform.scale.to_array(),
    ) {
        error!("recording gizmo transform: {error:#}");
    }
}

fn gizmo_drag_end(
    drag: On<Pointer<DragEnd>>,
    mut state: ResMut<PropGizmo>,
    mut document: ResMut<EditorDocument>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some((axis, _)) = state.drag else { return };
    if axis != drag.entity {
        return;
    }
    state.drag = None;
    document.0.commit_pending_prop_transform();
}

fn sync_prop_gizmo(
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<PropGizmoRoot>>,
    mut axes: Query<(&GizmoAxis, &mut Visibility), Without<PropGizmoRoot>>,
    gizmo: Res<PropGizmo>,
) {
    let Ok((mut root, mut visibility)) = roots.single_mut() else {
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
    root.translation = transform.translation();
    *visibility = Visibility::Visible;
    for (axis, mut visibility) in &mut axes {
        *visibility = if gizmo.mode == GizmoMode::Translate {
            Visibility::Visible
        } else {
            Visibility::Visible
        };
        let _ = axis;
    }
}

/// Immediate-mode presentation following Bevy's 3D gizmo example. The mesh
/// children remain only as transparent, pickable hit targets for axis dragging.
fn draw_prop_gizmo(
    mut gizmos: Gizmos,
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    gizmo: Res<PropGizmo>,
) {
    let Some(entity) = selected.0 else { return };
    let Ok(transform) = props.get(entity) else {
        return;
    };
    let origin = transform.translation();
    const AXES: [(Vec3, Color); 3] = [
        (Vec3::X, Color::srgb(0.95, 0.1, 0.1)),
        (Vec3::Y, Color::srgb(0.1, 0.95, 0.2)),
        (Vec3::Z, Color::srgb(0.1, 0.35, 1.0)),
    ];
    match gizmo.mode {
        GizmoMode::Translate => {
            for (axis, color) in AXES {
                gizmos.arrow(origin, origin + axis * 45.0, color);
            }
        }
        GizmoMode::Rotate => {
            for (axis, color) in AXES {
                gizmos
                    .circle(
                        Isometry3d::new(origin, Quat::from_rotation_arc(Vec3::Z, axis)),
                        35.0,
                        color,
                    )
                    .resolution(48);
            }
        }
    }
    gizmos.cross(origin, 6.0, Color::WHITE);
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
        // The upstream example's 5 units/s default is for its five-unit
        // plane; converted RO maps are hundreds to thousands of units wide.
        FreeCamera {
            walk_speed: 300.0,
            run_speed: 900.0,
            // Keep Q/E available for prop rotation. These match the common
            // Godot editor fly-camera convention instead.
            key_up: KeyCode::Space,
            key_down: KeyCode::ControlLeft,
            ..default()
        },
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
    let gizmo_root = commands
        .spawn((PropGizmoRoot, Visibility::Hidden, Transform::default()))
        .id();
    for (axis, color) in [
        (Vec3::X, Color::srgb(0.95, 0.1, 0.1)),
        (Vec3::Y, Color::srgb(0.1, 0.95, 0.2)),
        (Vec3::Z, Color::srgb(0.1, 0.35, 1.0)),
    ] {
        let length = 45.0;
        let mesh = meshes.add(Cuboid::new(
            if axis.x != 0.0 { length } else { 4.0 },
            if axis.y != 0.0 { length } else { 4.0 },
            if axis.z != 0.0 { length } else { 4.0 },
        ));
        commands.spawn((
            GizmoAxis { axis },
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                // `draw_prop_gizmo` is the visible handle. This mesh is only
                // a generous pick volume around its corresponding axis.
                base_color: color.with_alpha(0.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Pickable::default(),
            Transform::from_translation(axis * (length * 0.5)),
            ChildOf(gizmo_root),
        ));
    }
    commands.spawn_scene(command_overlay());
    commands.spawn_scene(notice_overlay());
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
                 Text("Camera\nWASD - move · Space/Ctrl - up/down\nRMB - look · Wheel - speed\n\nCollision\nLMB / arrows - select cell\nF - toggle walkability\nCtrl+Z / Ctrl+Y - undo / redo\nCtrl+S - save\n\nProps\nClick a prop, drag gizmo axis\nI/K J/L U/O - move\nQ/E - rotate · G/R - gizmo mode\nCtrl+D duplicate · Delete remove")
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

fn notice_overlay() -> impl Scene {
    bsn! {
        NoticeLabel
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: px(18),
            padding: UiRect::axes(px(12), px(8)),
        }
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0))
        Text("")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/space-mono.ttf"),
            font_size: FontSize::Px(16.0),
        }
        TextColor(Color::srgb(0.9, 0.93, 1.0))
        Pickable::IGNORE
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

fn save_document(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    mut notice: ResMut<EditorNotice>,
) {
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !control || !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    if let Err(error) = document.0.save() {
        error!("saving map: {error:#}");
        notice.show(format!("Save failed: {error}"));
    } else {
        notice.show("Saved map GLB");
    }
}

fn update_notice(
    time: Res<Time>,
    mut notice: ResMut<EditorNotice>,
    mut labels: Query<(&mut Text, &mut BackgroundColor), With<NoticeLabel>>,
) {
    let Some(timer) = notice.expires.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if timer.is_finished() {
        notice.message.clear();
        notice.expires = None;
    }
    let visible = !notice.message.is_empty();
    for (mut label, mut background) in &mut labels {
        label.0 = notice.message.clone();
        background.0 = if visible {
            Color::srgba(0.02, 0.025, 0.04, 0.88)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
    }
}

fn undo_redo(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<CursorCell>,
    selected: Res<SelectedProp>,
    mut markers: Query<&mut Transform, (With<CollisionCursor>, Without<LifPropRef>)>,
    mut props: Query<
        (&mut Transform, &LifMapNodeIndex, &mut Visibility),
        (With<LifPropRef>, Without<CollisionCursor>),
    >,
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
    for (_, node, mut visibility) in &mut props {
        *visibility = if document.0.prop_deleted(node.0) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if let Some(entity) = selected.0
        && let Ok((mut transform, node, mut visibility)) = props.get_mut(entity)
        && let Ok((translation, rotation, scale)) = document.0.prop_transform(node.0)
    {
        transform.translation = Vec3::from(translation);
        transform.rotation = Quat::from_array(rotation);
        transform.scale = Vec3::from(scale);
        *visibility = if document.0.prop_deleted(node.0) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
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
