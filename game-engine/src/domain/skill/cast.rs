use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_add_system};
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::domain::input::TargetingMode;
use crate::infrastructure::networking::quic::{
    channels::GAMEPLAY,
    envelope::Body,
    proto::aesir::net::{GroundSkillCast, SkillCast},
    zone::{QuicZoneState, ZonePhase},
};
use crate::infrastructure::networking::zone_messages::ChatHeard;

use super::{form, target, Form, SkillCooldownTracker, SkillTreeState, Target};

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct SkillCastRequested {
    pub skill_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum CastTarget {
    Entity(u32),
    Ground(u16, u16),
}

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct SkillCastResolved {
    pub skill_id: u32,
    pub level: u32,
    pub target: CastTarget,
}

fn resolved_to_body(skill_id: u32, level: u32, target: CastTarget) -> Body {
    match target {
        CastTarget::Entity(gid) => Body::SkillCast(SkillCast {
            skill_id,
            level,
            target_id: gid,
        }),
        CastTarget::Ground(x, y) => Body::GroundSkillCast(GroundSkillCast {
            skill_id,
            level,
            x: x as u32,
            y: y as u32,
        }),
    }
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn send_skill_cast(
    mut resolved: MessageReader<SkillCastResolved>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        resolved.clear();
        return;
    }

    for cast in resolved.read() {
        let body = resolved_to_body(cast.skill_id, cast.level, cast.target);
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("Failed to send skill cast: {e}");
        }
    }
}

fn reject(chat: &mut MessageWriter<ChatHeard>, message: &str) {
    chat.write(ChatHeard {
        gid: 0,
        message: message.to_string(),
    });
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn resolve_skill_cast(
    mut requests: MessageReader<SkillCastRequested>,
    mut resolved: MessageWriter<SkillCastResolved>,
    mut chat: MessageWriter<ChatHeard>,
    mut targeting: ResMut<TargetingMode>,
    tree: Res<SkillTreeState>,
    cooldowns: Res<SkillCooldownTracker>,
    zone: Res<QuicZoneState>,
) {
    for request in requests.read() {
        let skill_id = request.skill_id;

        let Some(node) = tree.skills.get(&skill_id) else {
            reject(&mut chat, "You haven't learned that skill");
            continue;
        };

        if node.level == 0 {
            reject(&mut chat, "You haven't learned that skill");
            continue;
        }

        if form(node.inf_type) == Form::Passive {
            reject(&mut chat, "Cannot cast a passive skill");
            continue;
        }

        if cooldowns.is_on_cooldown(skill_id) {
            reject(&mut chat, "Skill not ready");
            continue;
        }

        let level = node.level;
        match target(node.inf_type) {
            Target::SelfTarget | Target::None => {
                resolved.write(SkillCastResolved {
                    skill_id,
                    level,
                    target: CastTarget::Entity(zone.auth.char_id),
                });
            }
            Target::Enemy | Target::Ally => {
                *targeting = TargetingMode::AwaitingEntity { skill_id, level };
            }
            Target::Ground => {
                *targeting = TargetingMode::AwaitingGround { skill_id, level };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::skill::cooldown::apply_skill_cooldown;
    use crate::domain::skill::SkillNode;
    use crate::infrastructure::networking::quic::zone::ZoneAuth;
    use crate::infrastructure::networking::zone_messages::SkillCooldownSet;

    const SKILL_ID: u32 = 28;
    const OWN_GID: u32 = 2_000_042;

    fn node(level: u32, inf_type: u32) -> SkillNode {
        SkillNode {
            level,
            max_level: 10,
            upgradable: false,
            requires: vec![],
            req_base_level: 0,
            req_job_level: 0,
            sp: 0,
            range: 0,
            inf_type,
            job_id: 0,
        }
    }

    fn resolve_app() -> App {
        let mut app = App::new();
        app.add_message::<SkillCastRequested>()
            .add_message::<SkillCastResolved>()
            .add_message::<ChatHeard>()
            .init_resource::<SkillTreeState>()
            .init_resource::<SkillCooldownTracker>()
            .init_resource::<TargetingMode>()
            .insert_resource(QuicZoneState {
                auth: ZoneAuth {
                    char_id: OWN_GID,
                    ..default()
                },
                ..default()
            })
            .add_systems(Update, resolve_skill_cast);
        app
    }

    fn seed(app: &mut App, node: SkillNode) {
        app.world_mut()
            .resource_mut::<SkillTreeState>()
            .skills
            .insert(SKILL_ID, node);
    }

    fn request(app: &mut App) {
        app.world_mut()
            .resource_mut::<Messages<SkillCastRequested>>()
            .write(SkillCastRequested { skill_id: SKILL_ID });
        app.update();
    }

    fn resolved_msgs(app: &App) -> Vec<SkillCastResolved> {
        app.world()
            .resource::<Messages<SkillCastResolved>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn chats(app: &App) -> Vec<ChatHeard> {
        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn mode(app: &App) -> TargetingMode {
        *app.world().resource::<TargetingMode>()
    }

    #[test]
    fn self_target_resolves_to_own_gid_instantly() {
        let mut app = resolve_app();
        seed(&mut app, node(2, 4));
        request(&mut app);

        let msgs = resolved_msgs(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].skill_id, SKILL_ID);
        assert_eq!(msgs[0].level, 2);
        let CastTarget::Entity(gid) = msgs[0].target else {
            panic!("expected an entity target");
        };
        assert_eq!(gid, OWN_GID);
        assert_eq!(mode(&app), TargetingMode::Idle);
        assert!(chats(&app).is_empty());
    }

    #[test]
    fn none_target_resolves_to_own_gid_instantly() {
        let mut app = resolve_app();
        seed(&mut app, node(1, 99));
        request(&mut app);

        let msgs = resolved_msgs(&app);
        assert_eq!(msgs.len(), 1);
        let CastTarget::Entity(gid) = msgs[0].target else {
            panic!("expected an entity target");
        };
        assert_eq!(gid, OWN_GID);
        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn enemy_target_arms_awaiting_entity() {
        let mut app = resolve_app();
        seed(&mut app, node(3, 1));
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(
            mode(&app),
            TargetingMode::AwaitingEntity {
                skill_id: SKILL_ID,
                level: 3,
            }
        );
    }

    #[test]
    fn ally_target_arms_awaiting_entity() {
        let mut app = resolve_app();
        seed(&mut app, node(4, 16));
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(
            mode(&app),
            TargetingMode::AwaitingEntity {
                skill_id: SKILL_ID,
                level: 4,
            }
        );
    }

    #[test]
    fn ground_target_arms_awaiting_ground() {
        let mut app = resolve_app();
        seed(&mut app, node(5, 2));
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(
            mode(&app),
            TargetingMode::AwaitingGround {
                skill_id: SKILL_ID,
                level: 5,
            }
        );
    }

    #[test]
    fn unlearned_skill_is_rejected() {
        let mut app = resolve_app();
        seed(&mut app, node(0, 4));
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(chats(&app).len(), 1);
        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn absent_skill_is_rejected() {
        let mut app = resolve_app();
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(chats(&app).len(), 1);
        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn passive_skill_is_rejected() {
        let mut app = resolve_app();
        seed(&mut app, node(1, 0));
        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(chats(&app).len(), 1);
        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn on_cooldown_skill_is_rejected() {
        let mut app = resolve_app();
        app.add_message::<SkillCooldownSet>()
            .add_systems(Update, apply_skill_cooldown);
        seed(&mut app, node(1, 4));

        app.world_mut()
            .resource_mut::<Messages<SkillCooldownSet>>()
            .write(SkillCooldownSet {
                skill_id: SKILL_ID,
                tick: 5000,
            });
        app.update();

        request(&mut app);

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(chats(&app).len(), 1);
        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn entity_target_bridges_to_skill_cast() {
        let Body::SkillCast(cast) = resolved_to_body(28, 3, CastTarget::Entity(4096)) else {
            panic!("expected Body::SkillCast for an entity target");
        };
        assert_eq!(cast.skill_id, 28);
        assert_eq!(cast.level, 3);
        assert_eq!(cast.target_id, 4096);
    }

    #[test]
    fn ground_target_bridges_to_ground_skill_cast() {
        let Body::GroundSkillCast(cast) = resolved_to_body(42, 5, CastTarget::Ground(120, 80))
        else {
            panic!("expected Body::GroundSkillCast for a ground target");
        };
        assert_eq!(cast.skill_id, 42);
        assert_eq!(cast.level, 5);
        assert_eq!(cast.x, 120);
        assert_eq!(cast.y, 80);
    }
}
