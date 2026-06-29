use super::super::components::{EffectType, PlayerAppearance, RenderLayer};
use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::character::components::Gender;
use crate::domain::sprite::tags::{equipment_slot_to_tag, Z_OFFSET_PER_LAYER};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::AccessoryDb;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct EquipmentChangeEvent {
    pub character: Entity,
    pub slot: EquipmentSlot,
    pub view_id: Option<u16>,
}

/// Resolve a headgear `view_id` to its SPR/ACT sprite paths via the accessory db.
/// Returns `None` for unknown/cosmetic view ids (caller fails soft).
fn resolve_headgear_paths(
    accessory_db: &AccessoryDb,
    gender: Gender,
    view_id: u16,
) -> Option<(String, String)> {
    let accname = accessory_db.accname(view_id)?;
    Some((
        patterns::headgear_sprite_path(gender, accname),
        patterns::headgear_action_path(gender, accname),
    ))
}

#[derive(Message)]
#[auto_add_message(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub struct StatusEffectVisualEvent {
    pub character: Entity,
    pub effect_type: EffectType,
    pub add: bool,
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
    mut players: Query<(Entity, &mut PlayerAppearance, &Children, &Gender)>,
    render_layers: Query<(Entity, &RenderLayer)>,
    asset_server: Res<AssetServer>,
    accessory_db: Option<Res<AccessoryDb>>,
    mut pending_animations: ResMut<PendingAnimations>,
) {
    for event in equipment_events.read() {
        let Ok((entity, mut appearance, children, gender)) = players.get_mut(event.character)
        else {
            warn!(
                "handle_equipment_changes: Entity {:?} not found or missing PlayerAppearance/Gender",
                event.character
            );
            continue;
        };
        let gender = *gender;

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

        let Some(view_id) = event.view_id else {
            debug!(
                "handle_equipment_changes: Removed equipment from entity {:?}, slot {:?}",
                entity, event.slot
            );
            continue;
        };

        let Some(accessory_db) = accessory_db.as_deref() else {
            warn!(
                "handle_equipment_changes: AccessoryDb not loaded yet, skipping view id {} for entity {:?}",
                view_id, entity
            );
            continue;
        };

        let Some((spr_path, act_path)) = resolve_headgear_paths(accessory_db, gender, view_id)
        else {
            warn!(
                "handle_equipment_changes: Unknown headgear view id {} for entity {:?}, skipping",
                view_id, entity
            );
            continue;
        };

        let layer_tag = equipment_slot_to_tag(&event.slot);

        let spr = asset_server.load(&spr_path);
        let act = asset_server.load(&act_path);

        pending_animations.request(spr, act, layer_tag, Some(entity));

        debug!(
            "handle_equipment_changes: Requested headgear animation for entity {:?}, slot {:?}, view id {}",
            entity, event.slot, view_id
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::AccessoryData;

    fn db() -> AccessoryDb {
        let mut data = AccessoryData::default();
        data.names.insert(1, "_고글".to_string());
        AccessoryDb::from_accessory_data(data)
    }

    #[test]
    fn resolves_known_view_id_to_headgear_paths() {
        let (spr, act) =
            resolve_headgear_paths(&db(), Gender::Male, 1).expect("known view id resolves");
        assert_eq!(spr, "ro://data/sprite/악세사리/남/남_고글.spr");
        assert_eq!(act, "ro://data/sprite/악세사리/남/남_고글.act");
    }

    #[test]
    fn unknown_view_id_resolves_to_none() {
        assert!(resolve_headgear_paths(&db(), Gender::Male, 9999).is_none());
    }
}
