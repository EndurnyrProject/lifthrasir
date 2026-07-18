use super::super::components::{EffectType, PlayerAppearance, RenderLayer};
use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::entities::character::components::core::CharacterData;
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::character::components::Gender;
use crate::domain::sprite::tags::{equipment_slot_to_tag, Z_OFFSET_PER_LAYER};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use crate::{AccessoryDb, WeaponDb};
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

/// Resolve a weapon `view_id` to its SPR/ACT sprite paths via the weapon db.
/// Returns `None` for unknown view ids (caller fails soft).
fn resolve_weapon_paths(
    weapon_db: &WeaponDb,
    job_name: &str,
    gender: Gender,
    view_id: u16,
) -> Option<(String, String)> {
    let suffix = weapon_db.suffix(view_id)?;
    Some((
        patterns::weapon_sprite_path(gender, job_name, suffix),
        patterns::weapon_action_path(gender, job_name, suffix),
    ))
}

/// Resolve a shield `view_id` to its SPR/ACT sprite paths via the hardcoded
/// shield suffix table (classic names + numeric fallback). Never fails.
fn resolve_shield_paths(job_name: &str, gender: Gender, view_id: u16) -> (String, String) {
    let suffix = patterns::shield_suffix(view_id);
    (
        patterns::shield_sprite_path(gender, job_name, &suffix),
        patterns::shield_action_path(gender, job_name, &suffix),
    )
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
#[allow(clippy::too_many_arguments)]
pub fn handle_equipment_changes(
    mut commands: Commands,
    mut equipment_events: MessageReader<EquipmentChangeEvent>,
    mut players: Query<(
        Entity,
        &mut PlayerAppearance,
        &Children,
        &Gender,
        Option<&CharacterData>,
    )>,
    render_layers: Query<(Entity, &RenderLayer)>,
    asset_server: Res<AssetServer>,
    accessory_db: Option<Res<AccessoryDb>>,
    weapon_db: Option<Res<WeaponDb>>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut pending_animations: ResMut<PendingAnimations>,
) {
    for event in equipment_events.read() {
        let Ok((entity, mut appearance, children, gender, char_data)) =
            players.get_mut(event.character)
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

        let paths = match event.slot {
            EquipmentSlot::HeadTop | EquipmentSlot::HeadMid | EquipmentSlot::HeadBottom => {
                let Some(accessory_db) = accessory_db.as_deref() else {
                    warn!(
                        "handle_equipment_changes: AccessoryDb not loaded yet, skipping view id {} for entity {:?}",
                        view_id, entity
                    );
                    continue;
                };
                let Some(paths) = resolve_headgear_paths(accessory_db, gender, view_id) else {
                    warn!(
                        "handle_equipment_changes: Unknown headgear view id {} for entity {:?}, skipping",
                        view_id, entity
                    );
                    continue;
                };
                paths
            }
            EquipmentSlot::Weapon => {
                let Some(job_name) = resolve_job_name(job_registry.as_deref(), char_data) else {
                    warn!(
                        "handle_equipment_changes: No job sprite name for entity {:?}, skipping weapon view id {}",
                        entity, view_id
                    );
                    continue;
                };
                let Some(weapon_db) = weapon_db.as_deref() else {
                    warn!(
                        "handle_equipment_changes: WeaponDb not loaded yet, skipping view id {} for entity {:?}",
                        view_id, entity
                    );
                    continue;
                };
                let Some(paths) = resolve_weapon_paths(weapon_db, job_name, gender, view_id) else {
                    warn!(
                        "handle_equipment_changes: Unknown weapon view id {} for entity {:?}, skipping",
                        view_id, entity
                    );
                    continue;
                };
                paths
            }
            EquipmentSlot::Shield => {
                let Some(job_name) = resolve_job_name(job_registry.as_deref(), char_data) else {
                    warn!(
                        "handle_equipment_changes: No job sprite name for entity {:?}, skipping shield view id {}",
                        entity, view_id
                    );
                    continue;
                };
                resolve_shield_paths(job_name, gender, view_id)
            }
            other => {
                debug!(
                    "handle_equipment_changes: Slot {:?} not yet supported for entity {:?}, skipping",
                    other, entity
                );
                continue;
            }
        };

        let (spr_path, act_path) = paths;

        let layer_tag = equipment_slot_to_tag(&event.slot);

        let spr = asset_server.load(&spr_path);
        let act = asset_server.load(&act_path);

        pending_animations.request(spr, act, layer_tag, Some(entity));

        debug!(
            "handle_equipment_changes: Requested animation for entity {:?}, slot {:?}, view id {}",
            entity, event.slot, view_id
        );
    }
}

/// Look up the character's job sprite name from `CharacterData.job_id` via the
/// job registry. Returns `None` when either is unavailable (caller skips).
fn resolve_job_name<'a>(
    job_registry: Option<&'a JobSpriteRegistry>,
    char_data: Option<&CharacterData>,
) -> Option<&'a str> {
    let job_id = char_data?.job_id;
    job_registry?.get_sprite_name(job_id as u32)
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
    alive: Query<Entity>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Claim only equipment layers; body/head/cart completions belong to the
    // other finalizers sharing this queue and must stay untouched.
    let completed =
        pending_animations.take_completed_where(|tag| tag_to_equipment_slot(tag).is_some());
    if completed.is_empty() {
        return;
    }

    // Completions whose entity is alive but doesn't carry `PlayerAppearance`
    // yet (spawn-frame race): retried next frame instead of dropped.
    let mut deferred = Vec::new();

    for (pending, animation_handle) in completed {
        let Some(callback_entity) = pending.callback_entity else {
            continue;
        };

        let Ok((entity, mut appearance)) = players.get_mut(callback_entity) else {
            if alive.contains(callback_entity) {
                deferred.push((pending, animation_handle));
            }
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
                depth_bias: crate::domain::sprite::tags::layer_depth_bias(layer_tag),
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

    pending_animations.defer_completed(deferred);
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
    use lifthrasir_data::{AccessoryData, WeaponData};

    fn db() -> AccessoryDb {
        let mut data = AccessoryData::default();
        data.names.insert(1, "_고글".to_string());
        AccessoryDb::from_accessory_data(data)
    }

    fn weapon_db() -> WeaponDb {
        let mut data = WeaponData::default();
        data.names.insert(2, "_검".to_string());
        WeaponDb::from_weapon_data(data)
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

    #[test]
    fn resolves_known_view_id_to_weapon_paths() {
        let (spr, act) = resolve_weapon_paths(&weapon_db(), "검사", Gender::Male, 2)
            .expect("known view id resolves");
        assert_eq!(spr, "ro://data/sprite/인간족/검사/검사_남_검.spr");
        assert_eq!(act, "ro://data/sprite/인간족/검사/검사_남_검.act");
    }

    #[test]
    fn unknown_weapon_view_id_resolves_to_none() {
        assert!(resolve_weapon_paths(&weapon_db(), "검사", Gender::Male, 9999).is_none());
    }

    #[test]
    fn resolves_classic_shield_paths() {
        let (spr, act) = resolve_shield_paths("검사", Gender::Male, 1);
        assert_eq!(spr, "ro://data/sprite/방패/검사/검사_남_가드_방패.spr");
        assert_eq!(act, "ro://data/sprite/방패/검사/검사_남_가드_방패.act");
    }

    #[test]
    fn resolves_renewal_shield_paths() {
        let (spr, act) = resolve_shield_paths("검사", Gender::Male, 28901);
        assert_eq!(spr, "ro://data/sprite/방패/검사/검사_남_28901_방패.spr");
        assert_eq!(act, "ro://data/sprite/방패/검사/검사_남_28901_방패.act");
    }

    mod finalize {
        use super::super::*;
        use crate::domain::sprite::tags::{LAYER_BODY, LAYER_WEAPON};
        use crate::infrastructure::assets::animation_processing_system::PendingAnimation;
        use bevy::asset::AssetPlugin;
        use moonshine_tag::Tag;

        fn app() -> App {
            let mut app = App::new();
            app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
                .init_asset::<RoAnimationAsset>()
                .init_asset::<StandardMaterial>()
                .init_resource::<PendingAnimations>()
                .insert_resource(SharedSpriteQuad {
                    mesh: Handle::default(),
                })
                .add_systems(Update, finalize_equipment_layers);
            app
        }

        fn queue_completion(app: &mut App, tag: Tag, entity: Entity) {
            app.world_mut()
                .resource_mut::<PendingAnimations>()
                .defer_completed(vec![(
                    PendingAnimation {
                        sprite_handle: Handle::default(),
                        action_handle: Handle::default(),
                        layer_tag: tag,
                        callback_entity: Some(entity),
                    },
                    Handle::default(),
                )]);
        }

        fn queued_count(app: &mut App) -> usize {
            app.world_mut()
                .resource_mut::<PendingAnimations>()
                .take_completed_where(|_| true)
                .len()
        }

        #[test]
        fn equipment_finalizer_leaves_body_completions_queued() {
            // Regression: a body completion deferred by finalize_render_layers
            // (spawn-frame race) must survive finalize_equipment_layers instead
            // of being drained and silently dropped — that loss left characters
            // permanently invisible.
            let mut app = app();
            let entity = app.world_mut().spawn_empty().id();
            queue_completion(&mut app, LAYER_BODY, entity);

            app.update();

            assert_eq!(queued_count(&mut app), 1, "body completion must survive");
        }

        #[test]
        fn equipment_completion_for_unready_entity_is_deferred() {
            let mut app = app();
            let entity = app.world_mut().spawn_empty().id();
            queue_completion(&mut app, LAYER_WEAPON, entity);

            app.update();

            assert_eq!(queued_count(&mut app), 1, "retried, not dropped");
        }

        #[test]
        fn equipment_completion_for_dead_entity_is_dropped() {
            let mut app = app();
            let entity = app.world_mut().spawn_empty().id();
            app.world_mut().despawn(entity);
            queue_completion(&mut app, LAYER_WEAPON, entity);

            app.update();

            assert_eq!(queued_count(&mut app), 0);
        }
    }
}
