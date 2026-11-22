use bevy::prelude::*;
use net_contract::dto::{HomunculusAction, HomunculusSnapshot, HomunculusTarget};

use super::{HomunculusIntent, HomunculusState};
use crate::domain::{
    input::targeting::{HomunculusUnitTargetAction, TargetingMode},
    vending::VendingState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomunculusSkillSpec {
    Passive,
    SelfTarget,
    Enemy,
}

/// Matches the pre-renewal and renewal Aesir definitions for the classic families.
pub fn homunculus_skill_spec(id: u32) -> Option<HomunculusSkillSpec> {
    use HomunculusSkillSpec::*;
    Some(match id {
        8003 | 8007 | 8015 => Passive,
        8009 | 8012 | 8013 => Enemy,
        8001 | 8002 | 8004..=8006 | 8008 | 8010..=8011 | 8014 | 8016 => SelfTarget,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, Message)]
pub struct HomunculusCastIntent {
    pub skill_id: u32,
    pub level: Option<u32>,
}

fn rank(snapshot: &HomunculusSnapshot, cast: HomunculusCastIntent) -> Option<u32> {
    let learned = snapshot
        .skills
        .iter()
        .find(|info| info.skill_id == cast.skill_id)?
        .level;
    let level = cast.level.unwrap_or(learned);
    (level > 0 && level <= learned).then_some(level)
}

pub(super) fn resolve_cast(
    mut casts: MessageReader<HomunculusCastIntent>,
    state: Res<HomunculusState>,
    now: Res<Time<Real>>,
    mut target: ResMut<TargetingMode>,
    vending: Option<Res<VendingState>>,
    mut intents: MessageWriter<HomunculusIntent>,
) {
    for &cast in casts.read() {
        let (Some(snapshot), Some(gid), Some(actor)) = (
            state.snapshot(),
            state.controlled_gid(),
            state.controlled_actor(),
        ) else {
            continue;
        };
        if !state.can_mutate()
            || vending.as_ref().is_some_and(|s| s.gameplay_locked())
            || snapshot.cooldowns.iter().any(|c| {
                c.skill_id == cast.skill_id && state.remaining(c.remaining_ms, now.elapsed()) > 0
            })
        {
            continue;
        }
        let (Some(level), Some(spec)) =
            (rank(snapshot, cast), homunculus_skill_spec(cast.skill_id))
        else {
            continue;
        };
        let durable_id = snapshot.durable_id;
        match spec {
            HomunculusSkillSpec::Passive => {}
            HomunculusSkillSpec::SelfTarget => {
                *target = TargetingMode::Idle;
                intents.write(HomunculusIntent::Act {
                    durable_id,
                    action: HomunculusAction::CastSkill {
                        skill_id: cast.skill_id,
                        level,
                        target: HomunculusTarget::SelfTarget,
                    },
                });
                break;
            }
            HomunculusSkillSpec::Enemy => {
                *target = TargetingMode::HomunculusUnit {
                    durable_id,
                    gid,
                    actor,
                    action: HomunculusUnitTargetAction::CastSkill {
                        skill_id: cast.skill_id,
                        level,
                    },
                };
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::HomunculusSkillMetadata;

    #[test]
    fn companion_skill_uses_its_own_rank_and_target() {
        let snapshot = HomunculusSnapshot {
            skills: vec![HomunculusSkillMetadata {
                skill_id: 8009,
                level: 3,
                ..default()
            }],
            ..default()
        };
        assert_eq!(
            rank(
                &snapshot,
                HomunculusCastIntent {
                    skill_id: 8009,
                    level: None
                }
            ),
            Some(3)
        );
        assert_eq!(
            rank(
                &snapshot,
                HomunculusCastIntent {
                    skill_id: 8009,
                    level: Some(2)
                }
            ),
            Some(2)
        );
        assert_eq!(
            rank(
                &snapshot,
                HomunculusCastIntent {
                    skill_id: 8009,
                    level: Some(4)
                }
            ),
            None
        );
        assert_eq!(
            homunculus_skill_spec(8009),
            Some(HomunculusSkillSpec::Enemy)
        );
        assert_eq!(
            homunculus_skill_spec(8001),
            Some(HomunculusSkillSpec::SelfTarget)
        );
        assert_eq!(
            homunculus_skill_spec(8003),
            Some(HomunculusSkillSpec::Passive)
        );
    }
}
