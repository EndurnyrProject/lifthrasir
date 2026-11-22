use crate::{
    channels::{BULK, CONTROL, GAMEPLAY},
    dispatch::IncomingMessage,
    envelope::Body,
    proto::aesir::net,
};
use net_contract::{dto as d, events::HomunculusIngressPayload as Ingress};

pub(in crate::zone) fn ingress(message: &IncomingMessage) -> Option<Ingress> {
    match (&message.body, message.channel) {
        (Body::MapMove(_), CONTROL) => Some(Ingress::MapBarrier),
        (Body::HomunculusPrivateState(p), BULK)
            if p.removed && p.durable_id != 0 && p.world_gid == 0 =>
        {
            Some(Ingress::Removed {
                durable_id: p.durable_id,
            })
        }
        (Body::HomunculusPrivateState(p), BULK) => snapshot(p.clone()).map(Ingress::State),
        (Body::HomunculusResult(r), GAMEPLAY) => Some(Ingress::Result {
            request_id: r.request_id,
            success: r.success,
            error: error(r.error),
            state: match r.state.clone() {
                Some(p) => Some(snapshot(p)?),
                None => None,
            },
        }),
        _ => None,
    }
}

fn snapshot(p: net::HomunculusPrivateState) -> Option<d::HomunculusSnapshot> {
    if p.removed || p.durable_id == 0 {
        return None;
    }
    Some(d::HomunculusSnapshot {
        durable_id: p.durable_id,
        name: p.name,
        rename_eligible: p.rename_eligible,
        species_id: p.species_id,
        evolved: p.evolved,
        appearance_id: p.appearance_id,
        level: p.level,
        exp: p.exp,
        next_exp: p.next_exp,
        skill_points: p.skill_points,
        hp: p.hp,
        max_hp: p.max_hp,
        sp: p.sp,
        max_sp: p.max_sp,
        hunger: p.hunger,
        intimacy_hundredths: p.intimacy_hundredths,
        food_item_id: p.food_item_id,
        active_remaining_ms: p.active_remaining_ms,
        world_gid: (p.world_gid != 0).then_some(p.world_gid),
        current_target_id: (p.current_target_id != 0).then_some(p.current_target_id),
        lifecycle: lifecycle(p.lifecycle),
        activity: activity(p.activity),
        intimacy_grade: intimacy_grade(p.intimacy_grade),
        stats: p.stats.map(|s| d::HomunculusDisplayedStats {
            str: s.str,
            agi: s.agi,
            vit: s.vit,
            int: s.int,
            dex: s.dex,
            luk: s.luk,
            atk: s.atk,
            matk: s.matk,
            def: s.def,
            mdef: s.mdef,
            hit: s.hit,
            flee: s.flee,
            critical: s.critical,
            aspd: s.aspd,
        }),
        skills: p
            .skills
            .into_iter()
            .map(|s| d::HomunculusSkillMetadata {
                skill_id: s.skill_id,
                level: s.level,
                max_level: s.max_level,
                learnable: s.learnable,
                intimacy_required_hundredths: s.intimacy_required_hundredths,
            })
            .collect(),
        cooldowns: p
            .cooldowns
            .into_iter()
            .map(|c| d::HomunculusCooldown {
                skill_id: c.skill_id,
                remaining_ms: c.remaining_ms,
            })
            .collect(),
        ai_config: p.ai_config.map(ai_config),
    })
}

fn ai_config(a: net::HomunculusAiConfig) -> d::HomunculusAiConfig {
    d::HomunculusAiConfig {
        stance: ai_stance(a.stance),
        leash_distance: a.leash_distance,
        join_owner_target: a.join_owner_target,
        retaliate: a.retaliate,
        avoid_bosses: a.avoid_bosses,
        allowed_mob_class_ids: a.allowed_mob_class_ids,
        denied_mob_class_ids: a.denied_mob_class_ids,
        auto_feed: a.auto_feed,
        auto_feed_threshold: a.auto_feed_threshold,
        auto_cast_sp_reserve_percent: a.auto_cast_sp_reserve_percent,
        skills: a
            .skills
            .into_iter()
            .map(|s| d::HomunculusAiSkillConfig {
                skill_id: s.skill_id,
                mode: ai_skill_mode(s.mode),
                priority: s.priority,
                self_hp_threshold: s.self_hp_threshold.map(|t| t.percent),
                owner_hp_threshold: s.owner_hp_threshold.map(|t| t.percent),
                target_hp_range: s.target_hp_range.map(|r| d::HomunculusHpRange {
                    min_percent: r.min_percent,
                    max_percent: r.max_percent,
                }),
            })
            .collect(),
    }
}

pub(crate) fn wire_ai_config(a: &d::HomunculusAiConfig) -> net::HomunculusAiConfig {
    use d::{HomunculusAiSkillMode as Mode, HomunculusAiStance as Stance};
    net::HomunculusAiConfig {
        stance: match a.stance {
            Stance::Unspecified => net::HomunculusAiStance::Unspecified as i32,
            Stance::Passive => net::HomunculusAiStance::Passive as i32,
            Stance::Assist => net::HomunculusAiStance::Assist as i32,
            Stance::Aggressive => net::HomunculusAiStance::Aggressive as i32,
            Stance::Unknown(value) => value,
        },
        leash_distance: a.leash_distance,
        join_owner_target: a.join_owner_target,
        retaliate: a.retaliate,
        avoid_bosses: a.avoid_bosses,
        allowed_mob_class_ids: a.allowed_mob_class_ids.clone(),
        denied_mob_class_ids: a.denied_mob_class_ids.clone(),
        auto_feed: a.auto_feed,
        auto_feed_threshold: a.auto_feed_threshold,
        auto_cast_sp_reserve_percent: a.auto_cast_sp_reserve_percent,
        skills: a
            .skills
            .iter()
            .map(|s| net::HomunculusAiSkillConfig {
                skill_id: s.skill_id,
                mode: match s.mode {
                    Mode::Unspecified => net::HomunculusAiSkillMode::Unspecified as i32,
                    Mode::Manual => net::HomunculusAiSkillMode::Manual as i32,
                    Mode::Auto => net::HomunculusAiSkillMode::Auto as i32,
                    Mode::Unknown(value) => value,
                },
                priority: s.priority,
                self_hp_threshold: s
                    .self_hp_threshold
                    .map(|percent| net::HomunculusHpThreshold { percent }),
                owner_hp_threshold: s
                    .owner_hp_threshold
                    .map(|percent| net::HomunculusHpThreshold { percent }),
                target_hp_range: s.target_hp_range.map(|r| net::HomunculusHpRange {
                    min_percent: r.min_percent,
                    max_percent: r.max_percent,
                }),
            })
            .collect(),
    }
}

fn error(raw: i32) -> d::HomunculusError {
    match net::HomunculusError::try_from(raw) {
        Ok(net::HomunculusError::None) => d::HomunculusError::None,
        Ok(net::HomunculusError::NoCompanion) => d::HomunculusError::NoCompanion,
        Ok(net::HomunculusError::MalformedCommand) => d::HomunculusError::MalformedCommand,
        Ok(net::HomunculusError::WrongChannel) => d::HomunculusError::WrongChannel,
        Ok(net::HomunculusError::InvalidLifecycle) => d::HomunculusError::InvalidLifecycle,
        Ok(net::HomunculusError::InvalidPosition) => d::HomunculusError::InvalidPosition,
        Ok(net::HomunculusError::InvalidTarget) => d::HomunculusError::InvalidTarget,
        Ok(net::HomunculusError::OutOfRange) => d::HomunculusError::OutOfRange,
        Ok(net::HomunculusError::SkillNotLearned) => d::HomunculusError::SkillNotLearned,
        Ok(net::HomunculusError::InvalidSkillRank) => d::HomunculusError::InvalidSkillRank,
        Ok(net::HomunculusError::OnCooldown) => d::HomunculusError::OnCooldown,
        Ok(net::HomunculusError::InsufficientSp) => d::HomunculusError::InsufficientSp,
        Ok(net::HomunculusError::MissingItem) => d::HomunculusError::MissingItem,
        Ok(net::HomunculusError::HpGate) => d::HomunculusError::HpGate,
        Ok(net::HomunculusError::RenameNotAllowed) => d::HomunculusError::RenameNotAllowed,
        Ok(net::HomunculusError::InvalidName) => d::HomunculusError::InvalidName,
        Ok(net::HomunculusError::ConfirmationRequired) => d::HomunculusError::ConfirmationRequired,
        Ok(net::HomunculusError::InvalidAiConfig) => d::HomunculusError::InvalidAiConfig,
        Ok(net::HomunculusError::InsufficientSkillPoints) => {
            d::HomunculusError::InsufficientSkillPoints
        }
        Ok(net::HomunculusError::PrerequisitesNotMet) => d::HomunculusError::PrerequisitesNotMet,
        Ok(net::HomunculusError::Busy) => d::HomunculusError::Busy,
        Err(_) => d::HomunculusError::Unknown(raw),
    }
}

fn lifecycle(raw: i32) -> d::HomunculusLifecycle {
    match net::HomunculusLifecycle::try_from(raw) {
        Ok(net::HomunculusLifecycle::Unspecified) => d::HomunculusLifecycle::Unspecified,
        Ok(net::HomunculusLifecycle::Active) => d::HomunculusLifecycle::Active,
        Ok(net::HomunculusLifecycle::Rested) => d::HomunculusLifecycle::Rested,
        Ok(net::HomunculusLifecycle::Dead) => d::HomunculusLifecycle::Dead,
        Err(_) => d::HomunculusLifecycle::Unknown(raw),
    }
}

fn activity(raw: i32) -> d::HomunculusActivity {
    match net::HomunculusActivity::try_from(raw) {
        Ok(net::HomunculusActivity::Unspecified) => d::HomunculusActivity::Unspecified,
        Ok(net::HomunculusActivity::Idle) => d::HomunculusActivity::Idle,
        Ok(net::HomunculusActivity::Following) => d::HomunculusActivity::Following,
        Ok(net::HomunculusActivity::Moving) => d::HomunculusActivity::Moving,
        Ok(net::HomunculusActivity::Attacking) => d::HomunculusActivity::Attacking,
        Ok(net::HomunculusActivity::Casting) => d::HomunculusActivity::Casting,
        Ok(net::HomunculusActivity::Standby) => d::HomunculusActivity::Standby,
        Err(_) => d::HomunculusActivity::Unknown(raw),
    }
}

fn intimacy_grade(raw: i32) -> d::HomunculusIntimacyGrade {
    match net::HomunculusIntimacyGrade::try_from(raw) {
        Ok(net::HomunculusIntimacyGrade::Unspecified) => d::HomunculusIntimacyGrade::Unspecified,
        Ok(net::HomunculusIntimacyGrade::HateWithPassion) => {
            d::HomunculusIntimacyGrade::HateWithPassion
        }
        Ok(net::HomunculusIntimacyGrade::Hate) => d::HomunculusIntimacyGrade::Hate,
        Ok(net::HomunculusIntimacyGrade::Awkward) => d::HomunculusIntimacyGrade::Awkward,
        Ok(net::HomunculusIntimacyGrade::Shy) => d::HomunculusIntimacyGrade::Shy,
        Ok(net::HomunculusIntimacyGrade::Neutral) => d::HomunculusIntimacyGrade::Neutral,
        Ok(net::HomunculusIntimacyGrade::Cordial) => d::HomunculusIntimacyGrade::Cordial,
        Ok(net::HomunculusIntimacyGrade::Loyal) => d::HomunculusIntimacyGrade::Loyal,
        Err(_) => d::HomunculusIntimacyGrade::Unknown(raw),
    }
}

fn ai_stance(raw: i32) -> d::HomunculusAiStance {
    match net::HomunculusAiStance::try_from(raw) {
        Ok(net::HomunculusAiStance::Unspecified) => d::HomunculusAiStance::Unspecified,
        Ok(net::HomunculusAiStance::Passive) => d::HomunculusAiStance::Passive,
        Ok(net::HomunculusAiStance::Assist) => d::HomunculusAiStance::Assist,
        Ok(net::HomunculusAiStance::Aggressive) => d::HomunculusAiStance::Aggressive,
        Err(_) => d::HomunculusAiStance::Unknown(raw),
    }
}

fn ai_skill_mode(raw: i32) -> d::HomunculusAiSkillMode {
    match net::HomunculusAiSkillMode::try_from(raw) {
        Ok(net::HomunculusAiSkillMode::Unspecified) => d::HomunculusAiSkillMode::Unspecified,
        Ok(net::HomunculusAiSkillMode::Manual) => d::HomunculusAiSkillMode::Manual,
        Ok(net::HomunculusAiSkillMode::Auto) => d::HomunculusAiSkillMode::Auto,
        Err(_) => d::HomunculusAiSkillMode::Unknown(raw),
    }
}
