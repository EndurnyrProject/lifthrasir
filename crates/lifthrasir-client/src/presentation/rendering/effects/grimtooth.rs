//! Grimtooth's ground release trail, reconstructed from instant skill damage.
//! The contract has no primary-target or release event, so the first victim
//! supplies the direction and nearby splash notifications share one trail.

use std::collections::HashMap;

use bevy::prelude::*;
use lifthrasir_assets::effects::LoadedEffectAsset;
use lifthrasir_assets::loaders::RoAltitudeAsset;
use lifthrasir_runtime::effects::procedural::VfxSystems;
use lifthrasir_runtime::effects::{EffectAnchor, load_str_effect, spawn_effect};
use net_contract::events::SkillDamageReceived;

use crate::core::GameState;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::world::components::CurrentMapAltitude;
use crate::domain::world::map_scoped::MapScoped;

const GRIMTOOTH: u32 = 137;
const TRAIL_SPEED: f32 = 60.0;
const TOOTH_SPACING: f32 = 2.5;
const OVERSHOOT: f32 = 15.0;
/// Splash results have individual server ticks, not a shared cast identifier.
const SPLASH_WINDOW_SECS: f64 = 0.1;

pub(super) struct GrimtoothVfxPlugin;

impl Plugin for GrimtoothVfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_trails, advance_trails)
                .chain()
                .in_set(VfxSystems)
                .run_if(in_state(GameState::InGame)),
        );
    }
}

#[derive(Component)]
struct GrimtoothTrail {
    origin: Vec3,
    direction: Vec3,
    length: f32,
    next_tooth: f32,
    elapsed: f32,
    effect: Handle<LoadedEffectAsset>,
}

/// Start once per instant splash, independently of the hidden caster's visibility.
fn spawn_trails(
    mut events: MessageReader<SkillDamageReceived>,
    mut commands: Commands,
    time: Res<Time>,
    registry: Res<EntityRegistry>,
    transforms: Query<&Transform>,
    asset_server: Res<AssetServer>,
    mut recent: Local<HashMap<u32, f64>>,
) {
    let now = time.elapsed_secs_f64();
    recent.retain(|_, started| now - *started < SPLASH_WINDOW_SECS);

    for event in events.read().filter(|event| event.skill_id == GRIMTOOTH) {
        if recent.contains_key(&event.src_id) {
            continue;
        }
        let Some(source) = registry.get_entity(event.src_id) else {
            continue;
        };
        let Some(target) = registry.get_entity(event.target_id) else {
            continue;
        };
        let (Ok(source), Ok(target)) = (transforms.get(source), transforms.get(target)) else {
            continue;
        };
        let offset = target.translation - source.translation;
        let horizontal = Vec3::new(offset.x, 0.0, offset.z);
        let Some(direction) = horizontal.try_normalize() else {
            continue;
        };

        recent.insert(event.src_id, now);
        commands.spawn((
            GrimtoothTrail {
                origin: source.translation,
                direction,
                length: horizontal.length() + OVERSHOOT,
                next_tooth: TOOTH_SPACING,
                elapsed: 0.0,
                effect: load_str_effect(&asset_server, "grimtooth_trail.strfx.ron"),
            },
            Transform::default(),
            Visibility::default(),
            MapScoped,
        ));
    }
}

/// March terrain-snapped teeth past the target; each tooth owns its one-shot life.
fn advance_trails(
    mut commands: Commands,
    time: Res<Time>,
    mut trails: Query<(Entity, &mut GrimtoothTrail, Option<&Children>)>,
    altitude: Option<Res<CurrentMapAltitude>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
) {
    let altitude = altitude.and_then(|map| altitude_assets.get(&map.0));
    for (entity, mut trail, children) in &mut trails {
        if trail.next_tooth > trail.length && children.is_none_or(|children| children.is_empty()) {
            commands.entity(entity).despawn();
            continue;
        }

        trail.elapsed += time.delta_secs();
        let reached = (trail.elapsed * TRAIL_SPEED).min(trail.length);
        while trail.next_tooth <= reached {
            let mut position = trail.origin + trail.direction * trail.next_tooth;
            if let Some(height) =
                altitude.and_then(|map| map.altitude.get_terrain_height_at_position(position))
            {
                position.y = height;
            }
            let tooth = spawn_effect(
                &mut commands,
                trail.effect.clone(),
                EffectAnchor::Position(position),
                false,
                Color::WHITE,
                None,
            );
            commands.entity(tooth).insert(ChildOf(entity));
            trail.next_tooth += TOOTH_SPACING;
        }
    }
}
