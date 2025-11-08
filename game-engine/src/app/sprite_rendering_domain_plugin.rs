use bevy_auto_plugin::prelude::*;

/// Plugin for sprite rendering domain logic.
///
/// This plugin automatically registers:
/// - Resources: SpriteHierarchyConfig, EntitySpriteNames, AnimationSettings, RoFrameCache
/// - Events: SpawnSpriteEvent, EquipmentChangeEvent, StatusEffectVisualEvent, SpriteAnimationChangeEvent
/// - Systems: All sprite rendering and animation systems with proper dependency ordering
///
/// 1. spawn_sprite_hierarchy
/// 2. populate_sprite_assets (after spawn)
/// 3. handle_equipment_changes, handle_status_effect_visuals, handle_sprite_animation_changes (after populate)
/// 4. sync_character_animations_to_controllers, update_generic_sprite_direction (after handlers)
/// 5. add_animated_marker → remove_animated_marker → advance_animations → update_sprite_transforms → ro_animation_player_system
///
/// Additionally includes a periodic cleanup system that runs every 5 seconds.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct SpriteRenderingDomainPlugin;
