use std::collections::HashMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::character::components::action_mapping::action_offsets;
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::character::components::visual::{ActionType, CombatMotion};
use crate::domain::entities::sprite_rendering::components::{PlayerSprite, RenderLayer};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;

/// A weapon animation "has content" at an action index when that action exists and
/// at least one of its frames draws a part; empty actions (`sprite_index -1`) do not.
fn action_has_content(anim: &RoAnimationAsset, action_index: usize) -> bool {
    anim.actions
        .get(action_index)
        .is_some_and(|action| action.frames.iter().any(|frame| !frame.parts.is_empty()))
}

fn resolve_attack(anim: &RoAnimationAsset) -> ActionType {
    [
        (action_offsets::ATTACK2, ActionType::Attack2),
        (action_offsets::ATTACK1, ActionType::Attack1),
        (action_offsets::ATTACK, ActionType::Attack),
    ]
    .into_iter()
    .find(|(index, _)| action_has_content(anim, *index))
    .map(|(_, action)| action)
    .unwrap_or(ActionType::Attack)
}

/// Derive each armed player's `CombatMotion` (the swing it plays when attacking)
/// from its equipped weapon's animation. Players without a loaded weapon layer lose
/// their `CombatMotion` so they fall back to the unarmed attack. Mobs never carry a
/// weapon layer, so they are never touched.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::AnimationSync)
)]
pub fn sync_weapon_combat_motion(
    mut commands: Commands,
    animations: Res<Assets<RoAnimationAsset>>,
    weapon_query: Query<(&RenderLayer, &ChildOf)>,
    character_query: Query<(Entity, Option<&CombatMotion>), With<PlayerSprite>>,
) {
    let desired: HashMap<Entity, CombatMotion> = weapon_query
        .iter()
        .filter(|(layer, _)| layer.equipment_slot == Some(EquipmentSlot::Weapon))
        .filter_map(|(layer, child_of)| {
            let anim = animations.get(&layer.animation)?;
            Some((
                child_of.parent(),
                CombatMotion {
                    attack: resolve_attack(anim),
                },
            ))
        })
        .collect();

    for (entity, current) in character_query.iter() {
        match desired.get(&entity) {
            Some(motion) => {
                if current.map(|c| c.attack) != Some(motion.attack) {
                    commands.entity(entity).insert(*motion);
                }
            }
            None => {
                if current.is_some() {
                    commands.entity(entity).remove::<CombatMotion>();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::ro_animation_asset::{ActionData, FrameData, FramePart};

    fn sample_part() -> FramePart {
        FramePart {
            texture_index: 0,
            transform: Mat4::IDENTITY,
            position: Vec2::ZERO,
            scale: Vec2::ONE,
            texture_size: Vec2::ONE,
            color: Color::WHITE,
            mirror: false,
        }
    }

    fn anim_with_content(indices: &[usize]) -> RoAnimationAsset {
        let len = indices.iter().copied().max().map_or(0, |m| m + 1);
        let mut actions = vec![ActionData::default(); len];
        for &index in indices {
            actions[index] = ActionData {
                frames: vec![FrameData {
                    parts: vec![sample_part()],
                    ..Default::default()
                }],
                ..Default::default()
            };
        }
        RoAnimationAsset {
            actions,
            ..Default::default()
        }
    }

    #[test]
    fn action_has_content_detects_drawn_and_empty_actions() {
        let anim = anim_with_content(&[action_offsets::STANDBY]);
        assert!(action_has_content(&anim, action_offsets::STANDBY));
        assert!(!action_has_content(&anim, action_offsets::IDLE));
        assert!(!action_has_content(&anim, 9_999));
    }

    #[test]
    fn empty_frames_do_not_count_as_content() {
        let anim = RoAnimationAsset {
            actions: vec![ActionData::default()],
            ..Default::default()
        };
        assert!(!action_has_content(&anim, 0));
    }

    #[test]
    fn attack_prefers_attack2_then_attack1_then_attack3() {
        assert_eq!(
            resolve_attack(&anim_with_content(&[
                action_offsets::ATTACK2,
                action_offsets::ATTACK1,
                action_offsets::ATTACK,
            ])),
            ActionType::Attack2
        );
        assert_eq!(
            resolve_attack(&anim_with_content(&[action_offsets::ATTACK1])),
            ActionType::Attack1
        );
        assert_eq!(
            resolve_attack(&anim_with_content(&[action_offsets::ATTACK])),
            ActionType::Attack
        );
        assert_eq!(
            resolve_attack(&anim_with_content(&[action_offsets::STANDBY])),
            ActionType::Attack
        );
    }
}
