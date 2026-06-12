use super::super::components::{EffectType, PlayerAppearance, RenderLayer};
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::sprite::tags::{equipment_slot_to_tag, Z_OFFSET_PER_LAYER};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct EquipmentChangeEvent {
    pub character: Entity,
    pub slot: EquipmentSlot,
    pub new_item_id: Option<u32>,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct StatusEffectVisualEvent {
    pub character: Entity,
    pub effect_type: EffectType,
    pub add: bool,
}

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct SpriteAnimationChangeEvent {
    pub character_entity: Entity,
    pub action_type: crate::domain::entities::character::components::visual::ActionType,
}

/// Handle equipment changes by updating PlayerAppearance and spawning/despawning render layers.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationEvents)
)]
pub fn handle_equipment_changes(
    mut commands: Commands,
    mut equipment_events: MessageReader<EquipmentChangeEvent>,
    mut players: Query<(Entity, &mut PlayerAppearance, &Children)>,
    render_layers: Query<(Entity, &RenderLayer)>,
    asset_server: Res<AssetServer>,
    mut pending_animations: ResMut<PendingAnimations>,
) {
    for event in equipment_events.read() {
        let Ok((entity, mut appearance, children)) = players.get_mut(event.character) else {
            warn!(
                "handle_equipment_changes: Entity {:?} not found or missing PlayerAppearance",
                event.character
            );
            continue;
        };

        for child in children.iter() {
            let Ok((child_entity, render_layer)) = render_layers.get(child) else {
                continue;
            };

            if render_layer.equipment_slot == Some(event.slot) {
                commands.entity(child_entity).despawn();
                break;
            }
        }

        appearance.remove_equipment(event.slot);

        if let Some(item_id) = event.new_item_id {
            let spr_path = format!("data/sprite/equipment/{}.spr", item_id);
            let act_path = format!("data/sprite/equipment/{}.act", item_id);

            let layer_tag = equipment_slot_to_tag(&event.slot);

            let spr = asset_server.load(&spr_path);
            let act = asset_server.load(&act_path);

            pending_animations.request(spr, act, layer_tag, Some(entity));

            debug!(
                "handle_equipment_changes: Requested equipment animation for entity {:?}, slot {:?}, item {}",
                entity, event.slot, item_id
            );
        } else {
            debug!(
                "handle_equipment_changes: Removed equipment from entity {:?}, slot {:?}",
                entity, event.slot
            );
        }
    }
}

/// Finalize equipment render layers when animations are loaded.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationEvents, after = handle_equipment_changes)
)]
pub fn finalize_equipment_layers(
    mut commands: Commands,
    mut pending_animations: ResMut<PendingAnimations>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut players: Query<(Entity, &mut PlayerAppearance)>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let completed = pending_animations.take_completed();
    if completed.is_empty() {
        return;
    }

    for (pending, animation_handle) in completed {
        let Some(callback_entity) = pending.callback_entity else {
            continue;
        };

        let Ok((entity, mut appearance)) = players.get_mut(callback_entity) else {
            continue;
        };

        let Some(animation) = animations.get(&animation_handle) else {
            continue;
        };

        let layer_tag = animation.layer;

        let slot = tag_to_equipment_slot(layer_tag);
        if let Some(slot) = slot {
            appearance.set_equipment(slot, animation_handle.clone());

            let z_offset =
                crate::domain::sprite::tags::layer_order(layer_tag) as f32 * Z_OFFSET_PER_LAYER;

            let first_texture = animation.textures.first().cloned().unwrap_or_default();

            let material = materials.add(StandardMaterial {
                base_color_texture: Some(first_texture),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });

            let child = commands
                .spawn((
                    Mesh3d(shared_quad.mesh.clone()),
                    MeshMaterial3d(material),
                    Billboard,
                    RenderLayer::equipment(
                        animation_handle,
                        layer_tag,
                        slot,
                        animation.textures.clone(),
                    ),
                    Transform::from_translation(Vec3::new(0.0, 0.0, z_offset)),
                    GlobalTransform::default(),
                    Visibility::default(),
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                ))
                .id();

            commands.entity(entity).add_child(child);

            debug!(
                "finalize_equipment_layers: Spawned equipment layer for entity {:?}, slot {:?}",
                entity, slot
            );
        }
    }
}

fn tag_to_equipment_slot(tag: moonshine_tag::Tag) -> Option<EquipmentSlot> {
    use crate::domain::sprite::tags::*;

    if tag == LAYER_WEAPON {
        Some(EquipmentSlot::Weapon)
    } else if tag == LAYER_SHIELD {
        Some(EquipmentSlot::Shield)
    } else if tag == LAYER_GARMENT {
        Some(EquipmentSlot::Garment)
    } else if tag == LAYER_HEAD_TOP {
        Some(EquipmentSlot::HeadTop)
    } else if tag == LAYER_HEAD_MID {
        Some(EquipmentSlot::HeadMid)
    } else if tag == LAYER_HEAD_BOTTOM {
        Some(EquipmentSlot::HeadBottom)
    } else {
        None
    }
}

/// Placeholder for status effect visual handling.
/// Effects are handled by a separate system (not sprite layers).
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationEvents)
)]
pub fn handle_status_effect_visuals(mut effect_events: MessageReader<StatusEffectVisualEvent>) {
    for event in effect_events.read() {
        debug!(
            "Status effect for {:?}: type={:?}, add={} (effects handled separately)",
            event.character, event.effect_type, event.add
        );
    }
}

/// Handle animation change events by updating RoSprite action directly.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationEvents)
)]
pub fn handle_sprite_animation_changes(
    time: Res<Time>,
    mut animation_events: MessageReader<SpriteAnimationChangeEvent>,
    mut sprites: Query<&mut crate::infrastructure::assets::ro_animation_asset::RoSprite>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;

    for event in animation_events.read() {
        let Ok(mut ro_sprite) = sprites.get_mut(event.character_entity) else {
            continue;
        };

        let action = action_type_to_index(event.action_type);
        ro_sprite.set_action(action, game_time_ms);

        debug!(
            "handle_sprite_animation_changes: Set action {} for entity {:?}",
            action, event.character_entity
        );
    }
}

fn action_type_to_index(
    action_type: crate::domain::entities::character::components::visual::ActionType,
) -> u8 {
    use crate::domain::entities::character::components::visual::ActionType;
    match action_type {
        ActionType::Idle => 0,
        ActionType::Walk => 1,
        ActionType::Sit => 2,
        ActionType::Attack => 3,
        ActionType::Hit => 4,
        ActionType::Dead => 5,
        ActionType::Cast => 6,
        ActionType::Special => 7,
    }
}
