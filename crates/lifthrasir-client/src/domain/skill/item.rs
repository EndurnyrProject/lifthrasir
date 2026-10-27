//! Item-granted casts bypass the learned-skill gate without granting a skill.

use crate::{
    core::state::GameState,
    domain::{
        input::TargetingMode,
        skill::{CastTarget, SkillCastResolved},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::{
    dto::ItemSkillTargetKind,
    events::{ChatHeard, ItemSkillOffered},
    state::ZoneSession,
};

#[auto_add_system(
    plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(
        run_if = in_state(GameState::InGame),
        after = super::cast::resolve_skill_cast,
        before = super::cast::send_skill_cast
    )
)]
pub fn handle_item_skill_offer(
    mut offers: MessageReader<ItemSkillOffered>,
    mut targeting: ResMut<TargetingMode>,
    mut resolved: MessageWriter<SkillCastResolved>,
    mut chat: MessageWriter<ChatHeard>,
    session: Res<ZoneSession>,
) {
    for offer in offers.read() {
        if offer.skill_id == 0 || offer.level == 0 || session.char_id == 0 {
            warn!("invalid item skill offer or unavailable zone identity: {offer:?}");
            continue;
        }
        let ItemSkillOffered {
            skill_id,
            level,
            target_kind,
        } = *offer;
        *targeting = match target_kind {
            ItemSkillTargetKind::SelfTarget => {
                resolved.write(SkillCastResolved {
                    skill_id,
                    level,
                    target: CastTarget::Entity(session.char_id),
                });
                TargetingMode::Idle
            }
            ItemSkillTargetKind::Unit => TargetingMode::AwaitingEntity { skill_id, level },
            ItemSkillTargetKind::Ground => TargetingMode::AwaitingGround { skill_id, level },
        };
        if target_kind != ItemSkillTargetKind::SelfTarget {
            chat.write(ChatHeard {
                gid: 0,
                message: "Choose a target in range. The item is consumed when casting, even if the target is out of range.".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skill::SkillTreeState;

    #[test]
    fn item_offers_cast_self_or_arm_targeting_without_learning() {
        for (kind, expected) in [
            (ItemSkillTargetKind::SelfTarget, TargetingMode::Idle),
            (
                ItemSkillTargetKind::Unit,
                TargetingMode::AwaitingEntity {
                    skill_id: 28,
                    level: 7,
                },
            ),
            (
                ItemSkillTargetKind::Ground,
                TargetingMode::AwaitingGround {
                    skill_id: 28,
                    level: 7,
                },
            ),
        ] {
            let mut app = App::new();
            app.add_message::<ItemSkillOffered>()
                .add_message::<SkillCastResolved>()
                .add_message::<ChatHeard>()
                .init_resource::<TargetingMode>()
                .init_resource::<SkillTreeState>()
                .insert_resource(ZoneSession {
                    char_id: 42,
                    ..default()
                })
                .add_systems(Update, handle_item_skill_offer);
            app.world_mut().write_message(ItemSkillOffered {
                skill_id: 28,
                level: 7,
                target_kind: kind,
            });
            app.update();
            assert_eq!(*app.world().resource::<TargetingMode>(), expected);
            assert!(app.world().resource::<SkillTreeState>().skills.is_empty());
            let casts: Vec<_> = app
                .world()
                .resource::<Messages<SkillCastResolved>>()
                .iter_current_update_messages()
                .collect();
            if kind == ItemSkillTargetKind::SelfTarget {
                assert_eq!(casts.len(), 1);
                assert_eq!((casts[0].skill_id, casts[0].level), (28, 7));
                assert!(matches!(casts[0].target, CastTarget::Entity(42)));
            } else {
                assert!(casts.is_empty());
                assert!(
                    app.world()
                        .resource::<Messages<ChatHeard>>()
                        .iter_current_update_messages()
                        .any(|line| line.message.contains("range"))
                );
            }
        }
    }
}
