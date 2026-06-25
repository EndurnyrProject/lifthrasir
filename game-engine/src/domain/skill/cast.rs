use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_event, auto_add_system};
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::infrastructure::networking::quic::{
    channels::GAMEPLAY,
    envelope::Body,
    proto::aesir::net::{GroundSkillCast, SkillCast},
    zone::{QuicZoneState, ZonePhase},
};

#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct SkillCastRequested {
    pub skill_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum CastTarget {
    Entity(u32),
    Ground(u16, u16),
}

#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
