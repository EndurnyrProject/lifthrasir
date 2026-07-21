use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{GroundSkillCastRequested, SkillCastRequested};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{GroundSkillCast, SkillCast};
use crate::zone::{QuicZoneState, ZonePhase};

fn skill_cast_body(c: &SkillCastRequested) -> Body {
    Body::SkillCast(SkillCast {
        skill_id: c.skill_id,
        level: c.level,
        target_id: c.target_id,
    })
}

fn ground_skill_cast_body(c: &GroundSkillCastRequested) -> Body {
    Body::GroundSkillCast(GroundSkillCast {
        skill_id: c.skill_id,
        level: c.level,
        x: c.x,
        y: c.y,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_skill_cast_requests(
    mut events: MessageReader<SkillCastRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, skill_cast_body(ev)) {
            error!("failed to send SkillCast: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_ground_skill_cast_requests(
    mut events: MessageReader<GroundSkillCastRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, ground_skill_cast_body(ev)) {
            error!("failed to send GroundSkillCast: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_cast_body_carries_skill_level_and_target() {
        let body = skill_cast_body(&SkillCastRequested {
            skill_id: 28,
            level: 3,
            target_id: 4096,
        });
        match body {
            Body::SkillCast(SkillCast {
                skill_id,
                level,
                target_id,
            }) => {
                assert_eq!(skill_id, 28u32);
                assert_eq!(level, 3u32);
                assert_eq!(target_id, 4096u32);
            }
            other => panic!("expected Body::SkillCast, got {other:?}"),
        }
    }

    #[test]
    fn ground_skill_cast_body_carries_skill_level_and_cell() {
        let body = ground_skill_cast_body(&GroundSkillCastRequested {
            skill_id: 42,
            level: 5,
            x: 120,
            y: 80,
        });
        match body {
            Body::GroundSkillCast(GroundSkillCast {
                skill_id,
                level,
                x,
                y,
            }) => {
                assert_eq!(skill_id, 42u32);
                assert_eq!(level, 5u32);
                assert_eq!(x, 120u32);
                assert_eq!(y, 80u32);
            }
            other => panic!("expected Body::GroundSkillCast, got {other:?}"),
        }
    }
}
