//! Spawns the `EFFECT` objects baked into a map's RSW as persistent,
//! position-anchored effects.
//!
//! RSW `effect_type` is the rAthena `e_special_effects` (EF_*) id — the same
//! namespace aesir's `SpecialEffect` packet keys on. A descriptor with a `Str`
//! layer plays as an STR effect, reusing the skill-effect runtime. A descriptor
//! with a `Bespoke` layer instead (the classic hardcoded-particle ambient
//! effects like smoke and generic emitters, which have no STR in the original
//! client) spawns a `MapAmbientVfx` bridge entity for the presentation layer to
//! attach a hanabi particle system to. Unmapped `effect_type`s are `warn!`-ed so
//! we can discover which ones real maps actually use and grow the `special`
//! section of `effects.ron`.

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::components::{EmitterOutput, LifEffectEmitter, SceneOwner};
use crate::audio::map_sounds::{GltfMapSoundSpawned, MapSound, MapSoundSource};
use crate::effects::spawn_effect;
use crate::effects::{EffectAnchor, MapAmbientVfx};
use crate::effects::{descriptor_tint, load_effect};
use lifthrasir_assets::catalogs::EffectCatalog;

/// Creates scene-owned world-space effect/audio emitters without installing an audio device.
pub struct MapEmittersPlugin;
impl Plugin for MapEmittersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                release_stale_emitter_outputs,
                (
                    spawn_gltf_map_effects,
                    crate::audio::map_sounds::spawn_gltf_map_sounds,
                ),
            )
                .chain()
                .after(bevy::transform::TransformSystems::Propagate),
        )
        .add_systems(
            Update,
            cleanup_scene_emitters.before(crate::audio::map_sounds::drive_map_sound_cycle),
        );
    }
}

/// Remove orphaned scene-owned outputs, stopping live sound instances before deletion.
fn cleanup_scene_emitters(
    mut commands: Commands,
    alive: Query<Entity>,
    owned: Query<(Entity, &SceneOwner, Option<&MapSoundSource>)>,
    mut instances: Option<ResMut<Assets<bevy_kira_audio::AudioInstance>>>,
) {
    for (entity, owner, sound) in &owned {
        if alive.contains(owner.0) {
            continue;
        }
        stop_sound(sound, instances.as_deref_mut());
        commands.entity(entity).despawn();
    }
}

/// Despawns an output whose emitter node is gone or lost its spawned marker,
/// so the spawn systems that run next rebuild it at the node's position.
pub fn release_stale_emitter_outputs(
    mut commands: Commands,
    emitters: Query<(Has<GltfMapSoundSpawned>, Has<GltfMapEffectSpawned>)>,
    outputs: Query<(
        Entity,
        &EmitterOutput,
        Has<MapSound>,
        Option<&MapSoundSource>,
    )>,
    mut instances: Option<ResMut<Assets<bevy_kira_audio::AudioInstance>>>,
) {
    for (entity, output, is_sound, sound) in &outputs {
        let spawned = emitters
            .get(output.0)
            .is_ok_and(|(sound_spawned, effect_spawned)| {
                if is_sound {
                    sound_spawned
                } else {
                    effect_spawned
                }
            });
        if spawned {
            continue;
        }
        stop_sound(sound, instances.as_deref_mut());
        commands.entity(entity).despawn();
    }
}

fn stop_sound(
    sound: Option<&MapSoundSource>,
    instances: Option<&mut Assets<bevy_kira_audio::AudioInstance>>,
) {
    if let Some(handle) = sound.and_then(|sound| sound.instance.as_ref())
        && let Some(instances) = instances
        && let Some(mut instance) = instances.get_mut(handle)
    {
        instance.stop(bevy_kira_audio::AudioTween::linear(
            std::time::Duration::from_secs_f32(0.1),
        ));
    }
}

/// One effect at its already world-space imported emitter position.
///
/// Returns `false` when the catalog has no descriptor for `effect_type`, so
/// callers can warn once per distinct id instead of once per object.
fn spawn_map_effect(
    commands: &mut Commands,
    asset_server: &AssetServer,
    catalog: &EffectCatalog,
    position: Vec3,
    effect: &lifthrasir_data::lif::LifEffect,
    owner: Entity,
    emitter: Entity,
) -> bool {
    let Some(descriptor) = catalog.special(effect.effect_type) else {
        return false;
    };

    if let Some(handle) = load_effect(asset_server, descriptor) {
        let spawned = spawn_effect(
            commands,
            handle,
            EffectAnchor::Position(position),
            true,
            descriptor_tint(descriptor),
            None,
        );
        commands
            .entity(spawned)
            .insert((SceneOwner(owner), EmitterOutput(emitter)));
        return true;
    }

    let Some(key) = descriptor.bespoke_key() else {
        debug!(
            "Map effect {} has neither a Str nor a Bespoke layer; skipping",
            effect.effect_type
        );
        return true;
    };

    commands.spawn((
        Transform::from_translation(position),
        SceneOwner(owner),
        EmitterOutput(emitter),
        MapAmbientVfx {
            key: key.to_string(),
            emit_speed: effect.emit_speed,
            params: effect.params,
        },
    ));

    true
}

fn warn_unmapped(unmapped: BTreeMap<u32, usize>) {
    for (effect_type, count) in unmapped {
        warn!("No map effect mapping for effect_type {effect_type} ({count} objects); skipping");
    }
}

/// Marks a glb effect-emitter node whose effect has been spawned. The node is a
/// scene child of the caller-owned root, so the marker dies with that scene.
#[derive(Component)]
pub struct GltfMapEffectSpawned;

/// The glb counterpart of [`spawn_gltf_map_effects`]: emitter nodes carry their final
/// world position in `GlobalTransform`, so this must run after transform
/// propagation ([`GltfMapPlugin`] schedules it there).
///
/// [`GltfMapPlugin`]: MapEmittersPlugin
pub fn spawn_gltf_map_effects(
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    emitters: Query<
        (Entity, &LifEffectEmitter, &GlobalTransform, &SceneOwner),
        Without<GltfMapEffectSpawned>,
    >,
) {
    let Some(catalog) = catalog else { return };

    let mut unmapped: BTreeMap<u32, usize> = BTreeMap::new();

    for (entity, emitter, transform, owner) in emitters.iter() {
        let effect = &emitter.0;

        let spawned = spawn_map_effect(
            &mut commands,
            &asset_server,
            &catalog,
            transform.translation(),
            effect,
            owner.0,
            entity,
        );

        if !spawned {
            *unmapped.entry(effect.effect_type).or_default() += 1;
        }

        commands.entity(entity).insert(GltfMapEffectSpawned);
    }

    warn_unmapped(unmapped);
}
