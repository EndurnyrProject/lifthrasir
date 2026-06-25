//! Spawns the `EFFECT` objects baked into a map's RSW as persistent,
//! position-anchored STR effects, reusing the skill-effect runtime.
//!
//! RSW `effect_type` is the rAthena `e_special_effects` (EF_*) id — the same
//! namespace aesir's `SpecialEffect` packet keys on. Only the subset of effects
//! that map to an STR file is playable here; the classic ambient ones (torch,
//! smoke, firefly, …) are hardcoded particle systems in the original client and
//! have no STR, so they stay unmapped. Unmapped `effect_type`s are `warn!`-ed so
//! we can discover which ones real maps actually use and grow `map_effects.ron`.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::components::EffectAnchor;
use super::systems::spawn_effect;
use super::triggers::{descriptor_tint, load_effect};
use crate::domain::world::components::MapLoader;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset};
use crate::infrastructure::effect::MapEffectCatalog;
use crate::infrastructure::ro_formats::RswObject;
use crate::utils::coordinates::rsw_position_to_bevy;
use crate::utils::get_map_dimensions_from_ground;

/// Marks a `MapLoader` whose RSW effect objects have been spawned, so we do it
/// once per map load (mirrors `ModelsSpawned`).
#[derive(Component)]
pub struct MapEffectsSpawned;

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update
)]
pub fn spawn_map_effects(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    catalog: Option<Res<MapEffectCatalog>>,
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

            let Some(descriptor) = catalog.get(effect.effect_type) else {
                *unmapped.entry(effect.effect_type).or_default() += 1;
                continue;
            };

            let position = rsw_position_to_bevy(effect.position, map_width, map_height);
            let handle = load_effect(&asset_server, descriptor);
            let spawned = spawn_effect(
                &mut commands,
                handle,
                EffectAnchor::Position(position),
                true,
                descriptor_tint(descriptor),
                None,
            );
            commands.entity(spawned).insert(MapScoped);
        }

        for (effect_type, count) in unmapped {
            warn!(
                "No map effect mapping for effect_type {effect_type} ({count} objects); skipping"
            );
        }

        commands.entity(entity).insert(MapEffectsSpawned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::components::ActiveEffect;
    use crate::infrastructure::effect::{LoadedEffectAsset, SkillEffectDataAsset};
    use crate::infrastructure::ro_formats::{
        RoGround, RoWorld, RswEffect, RswGround, RswLight, RswWater,
    };

    fn seeded_map_catalog() -> MapEffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/map_effects.ron");
        let asset = ron::from_str::<SkillEffectDataAsset>(ron).expect("seed RON");
        MapEffectCatalog::from_effect_data(asset.0)
    }

    fn effect_obj(effect_type: u32) -> RswObject {
        RswObject::Effect(RswEffect {
            name: "fx".to_string(),
            position: [0.0, 0.0, 0.0],
            effect_type,
            emit_speed: 0.0,
            params: [0.0; 4],
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
            .insert_resource(seeded_map_catalog())
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
}
