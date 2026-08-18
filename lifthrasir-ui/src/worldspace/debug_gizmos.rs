//! Dev-only map-object gizmos. Alt+Shift+D toggles wireframe gizmos plus a
//! screen-projected info label over every point light, effect emitter and the
//! directional (sun) light on the loaded map, so their placement and parameters
//! are visible while iterating on a map.

use bevy::color::palettes::css;
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::world::gltf_map::LifEffectEmitter;

use crate::worldspace::{WorldCameraFilter, WorldspaceFont, viewport_to_ui};

const LABEL_FONT_SIZE: f32 = 11.0;
const LABEL_WIDTH: f32 = 260.0;
/// Above the world camera, below nameplates (`100`) so nameplates stay readable.
const LABEL_Z: i32 = 90;
/// Pixels below the object's projected origin, so the label clears the gizmo.
const LABEL_GAP: f32 = 4.0;
const LIGHT_GIZMO_RADIUS: f32 = 0.6;
const EFFECT_GIZMO_SIZE: f32 = 1.0;
const SUN_ARROW_LEN: f32 = 4.0;

/// Whether the dev overlay is currently showing. Flipped by Alt+Shift+D.
#[derive(Resource, Default)]
pub struct DevGizmosEnabled(pub bool);

/// A screen-projected info label bound to a map object entity.
#[derive(Component)]
struct DebugLabel {
    target: Entity,
}

/// Query filter matching any map object the overlay tracks.
type MapObjectFilter = Or<(
    With<PointLight>,
    With<LifEffectEmitter>,
    With<DirectionalLight>,
)>;

pub struct MapDebugGizmosPlugin;

impl Plugin for MapDebugGizmosPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevGizmosEnabled>();
        app.add_systems(Update, toggle_dev_gizmos);
        app.add_systems(
            Update,
            (draw_map_gizmos, sync_debug_labels, follow_debug_labels)
                .run_if(dev_gizmos_enabled)
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_labels);
    }
}

fn dev_gizmos_enabled(enabled: Res<DevGizmosEnabled>) -> bool {
    enabled.0
}

/// Edge-triggered Alt+Shift+D flips the overlay. Turning it off despawns the
/// labels immediately; the gizmos vanish on their own once the draw system
/// stops running.
fn toggle_dev_gizmos(
    keys: Res<ButtonInput<KeyCode>>,
    mut enabled: ResMut<DevGizmosEnabled>,
    mut commands: Commands,
    labels: Query<Entity, With<DebugLabel>>,
) {
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !(alt && shift && keys.just_pressed(KeyCode::KeyD)) {
        return;
    }

    enabled.0 = !enabled.0;
    if !enabled.0 {
        for entity in &labels {
            commands.entity(entity).despawn();
        }
    }
}

fn draw_map_gizmos(
    mut gizmos: Gizmos,
    lights: Query<(&GlobalTransform, &PointLight)>,
    effects: Query<&GlobalTransform, With<LifEffectEmitter>>,
    sun: Query<&GlobalTransform, With<DirectionalLight>>,
) {
    for (transform, light) in &lights {
        gizmos.sphere(
            Isometry3d::from_translation(transform.translation()),
            LIGHT_GIZMO_RADIUS,
            light.color,
        );
    }
    for transform in &effects {
        gizmos.cube(
            Transform::from_translation(transform.translation())
                .with_scale(Vec3::splat(EFFECT_GIZMO_SIZE)),
            css::ORANGE,
        );
    }
    for transform in &sun {
        let start = transform.translation();
        gizmos.arrow(
            start,
            start + transform.forward() * SUN_ARROW_LEN,
            css::YELLOW,
        );
    }
}

/// Keep exactly one label per map object: spawn for objects that lack one,
/// despawn labels whose target is no longer a light/effect/sun.
#[allow(clippy::too_many_arguments)]
fn sync_debug_labels(
    mut commands: Commands,
    font: Res<WorldspaceFont>,
    ambient: Option<Res<GlobalAmbientLight>>,
    lights: Query<(Entity, &PointLight)>,
    effects: Query<(Entity, &LifEffectEmitter)>,
    sun: Query<(Entity, &DirectionalLight)>,
    labels: Query<&DebugLabel>,
    stale: Query<(Entity, &DebugLabel)>,
    valid: Query<(), MapObjectFilter>,
) {
    let ambient_brightness = ambient.map(|a| a.brightness).unwrap_or_default();
    let has_label = |target: Entity| labels.iter().any(|label| label.target == target);

    for (entity, light) in &lights {
        if has_label(entity) {
            continue;
        }
        spawn_label(
            &mut commands,
            &font,
            entity,
            format!("L int:{:.0} range:{:.1}", light.intensity, light.range),
            css::LIGHT_YELLOW,
        );
    }
    for (entity, emitter) in &effects {
        if has_label(entity) {
            continue;
        }
        let fx = &emitter.0;
        spawn_label(
            &mut commands,
            &font,
            entity,
            format!(
                "FX {} type:{} spd:{:.2}",
                fx.name, fx.effect_type, fx.emit_speed
            ),
            css::ORANGE,
        );
    }
    for (entity, light) in &sun {
        if has_label(entity) {
            continue;
        }
        spawn_label(
            &mut commands,
            &font,
            entity,
            format!(
                "SUN lux:{:.0} amb:{:.2}",
                light.illuminance, ambient_brightness
            ),
            css::YELLOW,
        );
    }

    for (entity, label) in &stale {
        if valid.get(label.target).is_err() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_label(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    text: String,
    color: Srgba,
) {
    commands.spawn((
        DebugLabel { target },
        Text::new(text),
        TextFont {
            font: font.0.clone().into(),
            font_size: LABEL_FONT_SIZE.into(),
            ..default()
        },
        TextColor(color.into()),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(LABEL_WIDTH),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(LABEL_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

/// Project each label's target world position into UI space every frame.
fn follow_debug_labels(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut labels: Query<(Entity, &DebugLabel, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(target_transform) = targets.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - LABEL_WIDTH / 2.0);
                node.top = Val::Px(pos.y + LABEL_GAP);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all_labels(mut commands: Commands, labels: Query<Entity, With<DebugLabel>>) {
    for entity in &labels {
        commands.entity(entity).despawn();
    }
}
