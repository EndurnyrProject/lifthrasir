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

use super::components::{EffectAnchor, MapAmbientVfx};
use super::systems::spawn_effect;
use super::triggers::{descriptor_tint, load_effect};
use crate::domain::world::gltf_map::LifEffectEmitter;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::effect::EffectCatalog;

/// Marks a `MapLoader` whose RSW effect objects have been spawned, so we do it
/// once per map load (mirrors `ModelsSpawned`).
#[derive(Component)]
pub struct MapEffectsSpawned;

/// One map effect at an already world-space position. Both the RSW path (which
/// converts the RSW coordinates first) and the glb path (whose emitter nodes
/// carry the final world transform) go through here.
///
/// Returns `false` when the catalog has no descriptor for `effect_type`, so
/// callers can warn once per distinct id instead of once per object.
fn spawn_map_effect(
    commands: &mut Commands,
    asset_server: &AssetServer,
    catalog: &EffectCatalog,
    position: Vec3,
    effect_type: u32,
    emit_speed: f32,
    params: [f32; 4],
) -> bool {
    let Some(descriptor) = catalog.special(effect_type) else {
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
        commands.entity(spawned).insert(MapScoped);
        return true;
    }

    let Some(key) = descriptor.bespoke_key() else {
        debug!("Map effect {effect_type} has neither a Str nor a Bespoke layer; skipping");
        return true;
    };

    commands.spawn((
        Transform::from_translation(position),
        MapScoped,
        MapAmbientVfx {
            key: key.to_string(),
            emit_speed,
            params,
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
/// scene child of the `MapScoped` loader entity, so the marker dies with the map
/// -- exactly how [`MapEffectsSpawned`] behaves on the RSW path.
#[derive(Component)]
pub struct GltfMapEffectSpawned;

/// The glb counterpart of [`spawn_map_effects`]: emitter nodes carry their final
/// world position in `GlobalTransform`, so this must run after transform
/// propagation ([`GltfMapPlugin`] schedules it there).
///
/// [`GltfMapPlugin`]: crate::domain::world::GltfMapPlugin
pub fn spawn_gltf_map_effects(
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    emitters: Query<(Entity, &LifEffectEmitter, &GlobalTransform), Without<GltfMapEffectSpawned>>,
) {
    let Some(catalog) = catalog else { return };

    let mut unmapped: BTreeMap<u32, usize> = BTreeMap::new();

    for (entity, emitter, transform) in emitters.iter() {
        let effect = &emitter.0;

        let spawned = spawn_map_effect(
            &mut commands,
            &asset_server,
            &catalog,
            transform.translation(),
            effect.effect_type,
            effect.emit_speed,
            effect.params,
        );

        if !spawned {
            *unmapped.entry(effect.effect_type).or_default() += 1;
        }

        commands.entity(entity).insert(GltfMapEffectSpawned);
    }

    warn_unmapped(unmapped);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::GameState;
    use crate::domain::effects::components::ActiveEffect;
    use crate::domain::entities::registry::EntityRegistry;
    use crate::domain::world::map_scoped::despawn_map_scoped;
    use crate::infrastructure::effect::{EffectDataAsset, LoadedEffectAsset};
    use bevy::state::app::StatesPlugin;
    use lifthrasir_data::lif::LifEffect;

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::build(&asset.0).expect("seed catalog builds")
    }

    fn effect_emitter(effect_type: u32) -> LifEffectEmitter {
        effect_emitter_with_emit(effect_type, 0.0, [0.0; 4])
    }

    fn effect_emitter_with_emit(
        effect_type: u32,
        emit_speed: f32,
        params: [f32; 4],
    ) -> LifEffectEmitter {
        LifEffectEmitter(LifEffect {
            name: "fx".to_string(),
            effect_type,
            emit_speed,
            params,
        })
    }

    fn test_app(emitters: Vec<LifEffectEmitter>, catalog: Option<EffectCatalog>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<LoadedEffectAsset>()
            .add_systems(Update, spawn_gltf_map_effects);

        if let Some(catalog) = catalog {
            app.insert_resource(catalog);
        }

        for emitter in emitters {
            app.world_mut().spawn((
                emitter,
                GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            ));
        }

        app
    }

    fn active_effects(app: &mut App) -> usize {
        app.world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn mapped_gltf_effect_spawns_one_active_effect() {
        let mut app = test_app(vec![effect_emitter(89)], Some(seeded_catalog()));
        app.update();

        assert_eq!(active_effects(&mut app), 1);
        assert_eq!(
            app.world_mut()
                .query::<&GltfMapEffectSpawned>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn no_catalog_is_a_noop() {
        let mut app = test_app(vec![effect_emitter(89)], None);
        app.update();

        assert_eq!(active_effects(&mut app), 0);
    }

    #[test]
    fn unmapped_effect_type_is_skipped() {
        let mut app = test_app(
            vec![effect_emitter(89), effect_emitter(9999)],
            Some(seeded_catalog()),
        );
        app.update();

        assert_eq!(active_effects(&mut app), 1);
    }

    #[test]
    fn emits_one_effect_per_mapped_gltf_emitter() {
        let mut app = test_app(
            vec![effect_emitter(89), effect_emitter(89)],
            Some(seeded_catalog()),
        );
        app.update();

        assert_eq!(active_effects(&mut app), 2);
    }

    #[test]
    fn gltf_emitters_do_not_respawn_on_later_frames() {
        let mut app = test_app(vec![effect_emitter(89)], Some(seeded_catalog()));
        app.update();
        app.update();

        assert_eq!(active_effects(&mut app), 1);
    }

    #[test]
    fn spawned_effects_are_torn_down_on_map_exit() {
        let mut app = test_app(vec![effect_emitter(89)], Some(seeded_catalog()));
        app.add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .init_resource::<EntityRegistry>()
            .add_systems(OnExit(GameState::InGame), despawn_map_scoped);
        app.update();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();

        assert_eq!(active_effects(&mut app), 0);
    }

    #[test]
    fn bespoke_descriptor_spawns_map_ambient_vfx_entity() {
        let mut app = test_app(vec![effect_emitter(44)], Some(seeded_catalog()));
        app.update();

        let mut query = app.world_mut().query::<(&MapAmbientVfx, &Transform)>();
        let matches: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0.key, "smoke");
        assert_eq!(matches[0].1.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(active_effects(&mut app), 0);
    }

    #[test]
    fn bespoke_descriptor_copies_emit_speed_and_params() {
        let params = [1.0, 2.0, 3.0, 4.0];
        let mut app = test_app(
            vec![effect_emitter_with_emit(974, 5.5, params)],
            Some(seeded_catalog()),
        );
        app.update();

        let mut query = app.world_mut().query::<&MapAmbientVfx>();
        let matches: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].key, "emitter");
        assert_eq!(matches[0].emit_speed, 5.5);
        assert_eq!(matches[0].params, params);
    }
}
