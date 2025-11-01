use super::components::{EntitySpriteNames, SpriteHierarchyConfig};
use super::events::SpawnSpriteEvent;
use super::systems::{
    advance_animations, cleanup_orphaned_sprites, handle_equipment_changes,
    handle_sprite_animation_changes, handle_status_effect_visuals, populate_sprite_assets,
    spawn_sprite_hierarchy, sync_character_animations_to_controllers,
    update_generic_sprite_direction, update_sprite_transforms, EquipmentChangeEvent,
    SpriteAnimationChangeEvent, StatusEffectVisualEvent,
};
use crate::domain::entities::animation::{
    add_animated_marker, remove_animated_marker, ro_animation_player_system, AnimationSettings,
    RoFrameCache,
};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

pub struct GenericSpriteRenderingPlugin;

impl Plugin for GenericSpriteRenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpriteHierarchyConfig>()
            .init_resource::<EntitySpriteNames>()
            .init_resource::<AnimationSettings>()
            .init_resource::<RoFrameCache>()
            .add_message::<SpawnSpriteEvent>()
            .add_message::<EquipmentChangeEvent>()
            .add_message::<StatusEffectVisualEvent>()
            .add_message::<SpriteAnimationChangeEvent>()
            .add_systems(
                Update,
                (spawn_sprite_hierarchy, populate_sprite_assets).chain(),
            )
            .add_systems(
                Update,
                (
                    handle_equipment_changes,
                    handle_status_effect_visuals,
                    handle_sprite_animation_changes,
                )
                    .after(populate_sprite_assets),
            )
            .add_systems(
                Update,
                (
                    sync_character_animations_to_controllers,
                    update_generic_sprite_direction,
                )
                    .after(handle_sprite_animation_changes),
            )
            .add_systems(
                Update,
                (
                    add_animated_marker,
                    remove_animated_marker,
                    advance_animations,
                    update_sprite_transforms,
                    ro_animation_player_system,
                )
                    .chain()
                    .after(update_generic_sprite_direction),
            )
            .add_systems(
                Update,
                cleanup_orphaned_sprites
                    .run_if(on_timer(Duration::from_secs(5)))
                    .after(update_sprite_transforms),
            );
    }
}
