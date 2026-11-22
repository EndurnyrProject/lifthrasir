//! Owner commands use the authenticated zone session and gameplay channel.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::QuinnetClient;
use net_contract::{
    commands::HomunculusCommand,
    dto::{HomunculusAction as Action, HomunculusTarget},
    events::HomunculusSendFailed,
};

use crate::{
    channels::GAMEPLAY,
    envelope::Body,
    proto::aesir::net,
    zone::{QuicZoneState, ZonePhase, mapping::homunculus::wire_ai_config},
};

pub fn homunculus_request_body(request: &HomunculusCommand) -> Result<Body, &'static str> {
    use net::{homunculus_cast_skill_command::Target, homunculus_request::Command};
    if request.request_id == 0 {
        return Err("Invalid request ID.");
    }
    let command = match &request.action {
        Action::Inspect => Command::Inspect(net::HomunculusInspectCommand {}),
        Action::Move { x, y } if u16::try_from(*x).is_ok() && u16::try_from(*y).is_ok() => {
            Command::Move(net::HomunculusMoveCommand { x: *x, y: *y })
        }
        Action::Follow => Command::Follow(net::HomunculusFollowCommand {}),
        Action::Attack { target_id } if *target_id != 0 => {
            Command::Attack(net::HomunculusAttackCommand {
                target_id: *target_id,
            })
        }
        Action::Standby => Command::Standby(net::HomunculusStandbyCommand {}),
        Action::CastSkill {
            skill_id,
            level,
            target,
        } if *skill_id != 0 && *level != 0 => {
            let target = match *target {
                HomunculusTarget::SelfTarget => Target::Self_(true),
                HomunculusTarget::Unit(id) if id != 0 => Target::TargetId(id),
                HomunculusTarget::Unit(_) => return Err("Invalid skill target."),
            };
            Command::CastSkill(net::HomunculusCastSkillCommand {
                skill_id: *skill_id,
                level: *level,
                target: Some(target),
            })
        }
        Action::Feed => Command::Feed(net::HomunculusFeedCommand {}),
        Action::Rename { name } => {
            Command::Rename(net::HomunculusRenameCommand { name: name.clone() })
        }
        Action::Rest => Command::Rest(net::HomunculusRestCommand {}),
        Action::Delete { confirmed } => Command::Delete(net::HomunculusDeleteCommand {
            confirmed: *confirmed,
        }),
        Action::ReplaceAi { config } => Command::ReplaceAi(net::HomunculusReplaceAiCommand {
            config: Some(wire_ai_config(config)),
        }),
        Action::LearnSkill { skill_id } if *skill_id != 0 => {
            Command::LearnSkill(net::HomunculusLearnSkillCommand {
                skill_id: *skill_id,
            })
        }
        _ => return Err("Invalid Homunculus command."),
    };
    Ok(Body::HomunculusRequest(net::HomunculusRequest {
        request_id: request.request_id,
        command: Some(command),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope;
    use net_contract::{
        dto::{HomunculusAiConfig, HomunculusAiSkillConfig},
        state::ZoneSessionGeneration,
    };

    #[test]
    fn all_homunculus_request_oneofs_round_trip() {
        use net::homunculus_request::Command as Wire;
        let actions = [
            Action::Inspect,
            Action::Move { x: 1, y: 32 },
            Action::Follow,
            Action::Attack { target_id: 7 },
            Action::Standby,
            Action::CastSkill {
                skill_id: 8001,
                level: 3,
                target: HomunculusTarget::SelfTarget,
            },
            Action::Feed,
            Action::Rename { name: "Lif".into() },
            Action::Rest,
            Action::Delete { confirmed: true },
            Action::ReplaceAi {
                config: HomunculusAiConfig {
                    skills: vec![HomunculusAiSkillConfig {
                        skill_id: 8001,
                        self_hp_threshold: Some(0),
                        ..default()
                    }],
                    ..default()
                },
            },
            Action::LearnSkill { skill_id: 8002 },
        ];
        for (slot, action) in actions.into_iter().enumerate() {
            let request = HomunculusCommand {
                generation: ZoneSessionGeneration(9),
                request_id: u64::MAX,
                action,
            };
            let body = homunculus_request_body(&request).unwrap();
            let Some(Body::HomunculusRequest(decoded)) =
                envelope::decode(&envelope::encode(7, body)).unwrap().body
            else {
                panic!("not a Homunculus request")
            };
            assert_eq!(decoded.request_id, u64::MAX);
            let matched_slot = match decoded.command.unwrap() {
                Wire::Inspect(_) => 0,
                Wire::Move(p) => {
                    assert_eq!((p.x, p.y), (1, 32));
                    1
                }
                Wire::Follow(_) => 2,
                Wire::Attack(p) => {
                    assert_eq!(p.target_id, 7);
                    3
                }
                Wire::Standby(_) => 4,
                Wire::CastSkill(p) => {
                    assert_eq!(
                        p.target,
                        Some(net::homunculus_cast_skill_command::Target::Self_(true))
                    );
                    assert_eq!((p.skill_id, p.level), (8001, 3));
                    5
                }
                Wire::Feed(_) => 6,
                Wire::Rename(p) => {
                    assert_eq!(p.name, "Lif");
                    7
                }
                Wire::Rest(_) => 8,
                Wire::Delete(p) => {
                    assert!(p.confirmed);
                    9
                }
                Wire::ReplaceAi(p) => {
                    let config = p.config.unwrap();
                    assert_eq!(config.skills[0].self_hp_threshold.unwrap().percent, 0);
                    assert!(config.skills[0].owner_hp_threshold.is_none());
                    10
                }
                Wire::LearnSkill(p) => {
                    assert_eq!(p.skill_id, 8002);
                    11
                }
            };
            assert_eq!(slot, matched_slot);
        }
    }
}

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Last)]
pub fn send_homunculus_commands(
    mut requests: MessageReader<HomunculusCommand>,
    mut failures: MessageWriter<HomunculusSendFailed>,
    mut zone: ResMut<QuicZoneState>,
    mut client: ResMut<QuinnetClient>,
) {
    for request in requests.read() {
        let result = if request.generation.0 != zone.connection_epoch
            || zone.phase != ZonePhase::Playing
            || !client.is_connected()
        {
            Err("The zone is not ready for this command.".into())
        } else {
            homunculus_request_body(request)
                .map_err(String::from)
                .and_then(|body| {
                    zone.send(&mut client, GAMEPLAY, body)
                        .map_err(|error| error.to_string())
                })
        };
        if let Err(reason) = result {
            failures.write(HomunculusSendFailed {
                generation: request.generation,
                request_id: request.request_id,
                reason,
            });
        }
    }
}
