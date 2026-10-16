#[cfg(test)]
use crate::domain::effects::components::MapAmbientVfx;
#[cfg(test)]
use crate::domain::world::gltf_map::LifEffectEmitter;
#[cfg(test)]
use crate::infrastructure::effect::EffectCatalog;
#[cfg(test)]
use bevy::prelude::*;
pub use lifthrasir_runtime::world::emitters::*;

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
            .add_plugins(lifthrasir_runtime::world::MapEmittersPlugin);

        if let Some(catalog) = catalog {
            app.insert_resource(catalog);
        }

        let owner = app
            .world_mut()
            .spawn(crate::domain::world::map_scoped::MapScoped)
            .id();
        for emitter in emitters {
            app.world_mut().spawn((
                emitter,
                lifthrasir_runtime::world::SceneOwner(owner),
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
