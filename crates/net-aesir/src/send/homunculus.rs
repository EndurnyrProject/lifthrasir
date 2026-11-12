use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{HomunculusCommand, HomunculusRequested, HomunculusSkillTarget};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net as wire;
use crate::zone::{QuicZoneState, ZonePhase};

fn request_body(request: &HomunculusRequested) -> Body {
    use wire::homunculus_request::Command;
    let command = match &request.command {
        HomunculusCommand::Inspect => Command::Inspect(wire::HomunculusInspectCommand {}),
        HomunculusCommand::Move { x, y } => {
            Command::Move(wire::HomunculusMoveCommand { x: *x, y: *y })
        }
        HomunculusCommand::Follow => Command::Follow(wire::HomunculusFollowCommand {}),
        HomunculusCommand::Attack { target_id } => Command::Attack(wire::HomunculusAttackCommand {
            target_id: *target_id,
        }),
        HomunculusCommand::Standby => Command::Standby(wire::HomunculusStandbyCommand {}),
        HomunculusCommand::CastSkill {
            skill_id,
            level,
            target,
        } => {
            let target = match target {
                HomunculusSkillTarget::Actor(id) => {
                    wire::homunculus_cast_skill_command::Target::TargetId(*id)
                }
                HomunculusSkillTarget::SelfTarget => {
                    wire::homunculus_cast_skill_command::Target::Self_(true)
                }
            };
            Command::CastSkill(wire::HomunculusCastSkillCommand {
                skill_id: *skill_id,
                level: *level,
                target: Some(target),
            })
        }
        HomunculusCommand::Feed => Command::Feed(wire::HomunculusFeedCommand {}),
        HomunculusCommand::Rename { name } => {
            Command::Rename(wire::HomunculusRenameCommand { name: name.clone() })
        }
        HomunculusCommand::Rest => Command::Rest(wire::HomunculusRestCommand {}),
        HomunculusCommand::Delete { confirmed } => Command::Delete(wire::HomunculusDeleteCommand {
            confirmed: *confirmed,
        }),
        HomunculusCommand::LearnSkill { skill_id } => {
            Command::LearnSkill(wire::HomunculusLearnSkillCommand {
                skill_id: *skill_id,
            })
        }
    };
    Body::HomunculusRequest(wire::HomunculusRequest {
        request_id: request.request_id,
        command: Some(command),
    })
}

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update, config(run_if = client_connected))]
fn send_homunculus_requests(
    mut requests: MessageReader<HomunculusRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        requests.clear();
        return;
    }
    for request in requests.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, request_body(request)) {
            error!("failed to send HomunculusRequest: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_request_preserves_target_and_correlation() {
        let body = request_body(&HomunculusRequested {
            request_id: 42,
            command: HomunculusCommand::CastSkill {
                skill_id: 8001,
                level: 3,
                target: HomunculusSkillTarget::SelfTarget,
            },
        });
        let Body::HomunculusRequest(request) = body else {
            panic!("wrong body")
        };
        assert_eq!(request.request_id, 42);
        assert_eq!(
            request.command,
            Some(wire::homunculus_request::Command::CastSkill(
                wire::HomunculusCastSkillCommand {
                    skill_id: 8001,
                    level: 3,
                    target: Some(wire::homunculus_cast_skill_command::Target::Self_(true)),
                }
            ))
        );
    }
}
