use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{client_connected, QuinnetClient};
use net_contract::commands::{
    AttackRequested, LearnSkillRequested, SitToggled, StatRaiseRequested,
};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{ActionRequest, LearnSkill, StatUp};
use crate::zone::{QuicZoneState, ZonePhase};

/// CZ_REQUEST_ACT2 action codes: attack = 0, sit = 2, stand = 3.
const ACTION_ATTACK: u32 = 0;
const ACTION_SIT: u32 = 2;
const ACTION_STAND: u32 = 3;

fn attack_body(a: &AttackRequested) -> Body {
    Body::ActionRequest(ActionRequest {
        target_id: a.target_id,
        action: ACTION_ATTACK,
    })
}

fn sit_body(s: &SitToggled) -> Body {
    Body::ActionRequest(ActionRequest {
        target_id: 0,
        action: if s.sit { ACTION_SIT } else { ACTION_STAND },
    })
}

fn stat_up_body(s: &StatRaiseRequested) -> Body {
    Body::StatUp(StatUp {
        stat_id: s.stat_id,
        amount: s.amount,
    })
}

fn learn_skill_body(l: &LearnSkillRequested) -> Body {
    Body::LearnSkill(LearnSkill {
        skill_id: l.skill_id,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_attack_requests(
    mut events: MessageReader<AttackRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, attack_body(ev)) {
            error!("failed to send attack ActionRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_sit_requests(
    mut events: MessageReader<SitToggled>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, sit_body(ev)) {
            error!("failed to send sit/stand ActionRequest: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_stat_raise_requests(
    mut events: MessageReader<StatRaiseRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, stat_up_body(ev)) {
            error!("failed to send StatUp: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_learn_skill_requests(
    mut events: MessageReader<LearnSkillRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, learn_skill_body(ev)) {
            error!("failed to send LearnSkill: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_body_carries_target_with_attack_action() {
        let body = attack_body(&AttackRequested { target_id: 42 });
        match body {
            Body::ActionRequest(ActionRequest { target_id, action }) => {
                assert_eq!(target_id, 42u32);
                assert_eq!(action, ACTION_ATTACK);
            }
            other => panic!("expected Body::ActionRequest, got {other:?}"),
        }
    }

    #[test]
    fn sit_body_maps_sit_flag_to_legacy_action_codes() {
        match sit_body(&SitToggled { sit: true }) {
            Body::ActionRequest(ActionRequest { target_id, action }) => {
                assert_eq!(target_id, 0u32);
                assert_eq!(action, ACTION_SIT);
            }
            other => panic!("expected Body::ActionRequest, got {other:?}"),
        }
        match sit_body(&SitToggled { sit: false }) {
            Body::ActionRequest(ActionRequest { action, .. }) => {
                assert_eq!(action, ACTION_STAND);
            }
            other => panic!("expected Body::ActionRequest, got {other:?}"),
        }
    }

    #[test]
    fn stat_up_body_carries_stat_and_amount() {
        let body = stat_up_body(&StatRaiseRequested {
            stat_id: 13,
            amount: 2,
        });
        match body {
            Body::StatUp(StatUp { stat_id, amount }) => {
                assert_eq!(stat_id, 13u32);
                assert_eq!(amount, 2u32);
            }
            other => panic!("expected Body::StatUp, got {other:?}"),
        }
    }

    #[test]
    fn learn_skill_body_carries_skill_id() {
        let body = learn_skill_body(&LearnSkillRequested { skill_id: 28 });
        match body {
            Body::LearnSkill(LearnSkill { skill_id }) => {
                assert_eq!(skill_id, 28u32);
            }
            other => panic!("expected Body::LearnSkill, got {other:?}"),
        }
    }
}
