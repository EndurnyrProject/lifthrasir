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
use bevy_auto_plugin::prelude::*;

use super::components::{EffectAnchor, MapAmbientVfx};
use super::systems::spawn_effect;
use super::triggers::{descriptor_tint, load_effect};
use crate::domain::world::components::MapLoader;
#[cfg(feature = "map-gltf")]
use crate::domain::world::gltf_map::LifEffectEmitter;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset};
use crate::infrastructure::effect::EffectCatalog;
use crate::infrastructure::ro_formats::RswObject;
use crate::utils::coordinates::rsw_position_to_bevy;
use crate::utils::get_map_dimensions_from_ground;

/// Marks a `MapLoader` whose RSW effect objects have been spawned, so we do it
/// once per map load (mirrors `ModelsSpawned`).
#[derive(Component)]
pub struct MapEffectsSpawned;

#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update
)]
pub fn spawn_map_effects(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &MapLoader), Without<MapEffectsSpawned>>,
) {
    let Some(catalog) = catalog else { return };

    for (entity, map_loader) in query.iter() {
        let Some(world_handle) = &map_loader.world else {
            continue;
        };
        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };
        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let (map_width, map_height) = get_map_dimensions_from_ground(&ground_asset.ground);

        // Count unmapped effect_types so we warn once per distinct id, not once
        // per object (a map can carry dozens of the same emitter).
        let mut unmapped: BTreeMap<u32, usize> = BTreeMap::new();

        for obj in &world_asset.world.objects {
            let RswObject::Effect(effect) = obj else {
                continue;
            };

            let spawned = spawn_map_effect(
                &mut commands,
                &asset_server,
                &catalog,
                rsw_position_to_bevy(effect.position, map_width, map_height),
                effect.effect_type,
                effect.emit_speed,
                effect.params,
            );

            if !spawned {
                *unmapped.entry(effect.effect_type).or_default() += 1;
            }
        }

        warn_unmapped(unmapped);

        commands.entity(entity).insert(MapEffectsSpawned);
    }
}

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
#[cfg(feature = "map-gltf")]
#[derive(Component)]
pub struct GltfMapEffectSpawned;

/// The glb counterpart of [`spawn_map_effects`]: emitter nodes carry their final
/// world position in `GlobalTransform`, so this must run after transform
/// propagation ([`GltfMapPlugin`] schedules it there).
///
/// [`GltfMapPlugin`]: crate::domain::world::GltfMapPlugin
#[cfg(feature = "map-gltf")]
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
    use crate::domain::effects::components::ActiveEffect;
    use crate::infrastructure::effect::{EffectDataAsset, LoadedEffectAsset};
    use crate::infrastructure::ro_formats::{
        RoGround, RoWorld, RswEffect, RswGround, RswLight, RswWater,
    };

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::build(&asset.0).expect("seed catalog builds")
    }

    fn effect_obj(effect_type: u32) -> RswObject {
        effect_obj_with_emit(effect_type, 0.0, [0.0; 4])
    }

    fn effect_obj_with_emit(effect_type: u32, emit_speed: f32, params: [f32; 4]) -> RswObject {
        RswObject::Effect(RswEffect {
            name: "fx".to_string(),
            position: [0.0, 0.0, 0.0],
            effect_type,
            emit_speed,
            params,
        })
    }

    fn test_world(objects: Vec<RswObject>) -> RoWorld {
        RoWorld {
            version: "1.0".to_string(),
            ini_file: String::new(),
            gnd_file: String::new(),
            gat_file: String::new(),
            src_file: None,
            water: RswWater::default(),
            light: RswLight::default(),
            ground: RswGround::default(),
            objects,
        }
    }

    fn test_ground() -> RoGround {
        RoGround {
            version: "1.0".to_string(),
            width: 100,
            height: 100,
            textures: Vec::new(),
            texture_indexes: Vec::new(),
            tiles: Vec::new(),
            surfaces: Vec::new(),
        }
    }

    fn test_app(objects: Vec<RswObject>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<RoWorldAsset>()
            .init_asset::<RoGroundAsset>()
            .init_asset::<LoadedEffectAsset>()
            .insert_resource(seeded_catalog())
            .add_systems(Update, spawn_map_effects);

        let world_handle =
            app.world_mut()
                .resource_mut::<Assets<RoWorldAsset>>()
                .add(RoWorldAsset {
                    world: test_world(objects),
                });
        let ground_handle =
            app.world_mut()
                .resource_mut::<Assets<RoGroundAsset>>()
                .add(RoGroundAsset {
                    ground: test_ground(),
                });

        app.world_mut().spawn(MapLoader {
            ground: ground_handle,
            altitude: None,
            world: Some(world_handle),
        });
        app
    }

    fn active_effects(app: &mut App) -> usize {
        app.world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn spawns_one_effect_per_mapped_effect_object() {
        let mut app = test_app(vec![effect_obj(89), effect_obj(89)]);
        app.update();
        assert_eq!(active_effects(&mut app), 2);
    }

    #[test]
    fn unmapped_effect_type_is_skipped() {
        let mut app = test_app(vec![effect_obj(89), effect_obj(9999)]);
        app.update();
        assert_eq!(active_effects(&mut app), 1, "only the mapped effect spawns");
    }

    #[test]
    fn loader_is_marked_and_effects_do_not_respawn() {
        let mut app = test_app(vec![effect_obj(89)]);
        app.update();
        app.update();
        assert_eq!(
            active_effects(&mut app),
            1,
            "MapEffectsSpawned guards against respawning on later frames"
        );
    }

    #[test]
    fn spawned_effects_are_map_scoped() {
        let mut app = test_app(vec![effect_obj(89)]);
        app.update();
        let scoped = app
            .world_mut()
            .query_filtered::<&MapScoped, With<ActiveEffect>>()
            .iter(app.world())
            .count();
        assert_eq!(scoped, 1, "map effects despawn with the map");
    }

    #[test]
    fn no_catalog_is_a_noop() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<RoWorldAsset>()
            .init_asset::<RoGroundAsset>()
            .init_asset::<LoadedEffectAsset>()
            .add_systems(Update, spawn_map_effects);

        let world_handle =
            app.world_mut()
                .resource_mut::<Assets<RoWorldAsset>>()
                .add(RoWorldAsset {
                    world: test_world(vec![effect_obj(89)]),
                });
        let ground_handle =
            app.world_mut()
                .resource_mut::<Assets<RoGroundAsset>>()
                .add(RoGroundAsset {
                    ground: test_ground(),
                });
        app.world_mut().spawn(MapLoader {
            ground: ground_handle,
            altitude: None,
            world: Some(world_handle),
        });

        app.update();
        assert_eq!(
            active_effects(&mut app),
            0,
            "nothing spawns without a catalog"
        );
    }

    #[test]
    fn bespoke_descriptor_spawns_map_ambient_vfx_entity() {
        let mut app = test_app(vec![effect_obj(44)]);
        app.update();

        let mut query = app.world_mut().query::<(&MapAmbientVfx, &Transform)>();
        let matches: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(matches.len(), 1, "exactly one MapAmbientVfx spawns");
        let (vfx, transform) = matches[0];
        assert_eq!(vfx.key, "smoke");
        assert_eq!(
            transform.translation,
            rsw_position_to_bevy([0.0; 3], 100.0, 100.0)
        );
        assert_eq!(
            active_effects(&mut app),
            0,
            "Bespoke path spawns no ActiveEffect"
        );
    }

    #[test]
    fn bespoke_descriptor_entity_is_map_scoped() {
        let mut app = test_app(vec![effect_obj(44)]);
        app.update();
        let scoped = app
            .world_mut()
            .query_filtered::<&MapScoped, With<MapAmbientVfx>>()
            .iter(app.world())
            .count();
        assert_eq!(scoped, 1, "map ambient vfx despawns with the map");
    }

    #[test]
    fn bespoke_descriptor_copies_emit_speed_and_params() {
        let params = [1.0, 2.0, 3.0, 4.0];
        let mut app = test_app(vec![effect_obj_with_emit(974, 5.5, params)]);
        app.update();

        let mut query = app.world_mut().query::<&MapAmbientVfx>();
        let matches: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].key, "emitter");
        assert_eq!(matches[0].emit_speed, 5.5);
        assert_eq!(matches[0].params, params);
    }

    #[test]
    fn str_layer_still_spawns_active_effect() {
        let mut app = test_app(vec![effect_obj(89)]);
        app.update();
        assert_eq!(active_effects(&mut app), 1, "STR path is unchanged");

        let vfx_entities = app
            .world_mut()
            .query::<&MapAmbientVfx>()
            .iter(app.world())
            .count();
        assert_eq!(vfx_entities, 0, "STR path spawns no MapAmbientVfx");
    }
}
