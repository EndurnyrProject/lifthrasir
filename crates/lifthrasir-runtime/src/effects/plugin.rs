use super::EffectLayer;
use super::material::EffectMaterial;
use super::overlays::{
    apply_body_state_tint, finalize_frozen_ice_assets, load_frozen_ice_assets, orbit_sight_visuals,
    orbit_spirit_spheres, sync_frozen_overlays,
};
use super::playback::*;
use super::procedural::{
    VfxSystems, ambient::MapAmbientVfxPlugin, impact::ImpactVfxPlugin, portal::PortalVfxPlugin,
    skill_fx::SkillFxPlugin,
};
use super::sprites::{EffectSpriteAssets, spawn_effect_sprites, sync_effect_sprites};
use bevy::prelude::*;
use bevy_hanabi::{EffectAsset, HanabiPlugin};
use lifthrasir_assets::animation::SpriteProcessingSystems;

/// STR and procedural playback after content assets and RuntimeCorePlugin.
/// Headless callers run CPU systems/data only; GPU particles require a RenderApp.
/// Install after DefaultPlugins when rendering; adding a renderer later is not supported.
pub struct EffectsPlaybackPlugin;

impl Plugin for EffectsPlaybackPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Assets<StandardMaterial>>() {
            app.init_asset::<StandardMaterial>();
        }
        // Hanabi's finish requires a real RenderApp; headless callers need only its data stores.
        if app.get_sub_app_mut(bevy::render::RenderApp).is_some() {
            app.add_plugins(HanabiPlugin);
        } else {
            app.init_asset::<EffectAsset>();
        }
        app.add_plugins((
            MaterialPlugin::<EffectMaterial>::default(),
            PortalVfxPlugin,
            ImpactVfxPlugin,
            SkillFxPlugin,
            MapAmbientVfxPlugin,
        ))
        .configure_sets(Update, VfxSystems)
        .init_resource::<EffectSpriteAssets>()
        .add_systems(Startup, load_frozen_ice_assets)
        .add_systems(
            Update,
            (
                finalize_frozen_ice_assets.after(SpriteProcessingSystems::Dispatch),
                sync_frozen_overlays,
                (spawn_effect_sprites, sync_effect_sprites)
                    .chain()
                    .after(SpriteProcessingSystems::Dispatch),
                apply_body_state_tint,
                orbit_sight_visuals,
                orbit_spirit_spheres,
            ),
        )
        .add_systems(Update, follow_effect_anchor)
        .add_systems(
            Update,
            (
                advance_effect_timers,
                initialize_effect_layers,
                rebuild_effect_layers.run_if(any_with_component::<EffectLayer>),
                order_effect_layers_by_depth,
                despawn_finished_effects,
            )
                .chain(),
        );
    }
}
