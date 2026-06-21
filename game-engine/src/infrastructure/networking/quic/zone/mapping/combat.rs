use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{
    CastCancelled, DamageReceived, GroundSkillPlaced, KnockedBack, SkillCastStarted,
    SkillCooldownSet, SkillDamageReceived, SkillEffectShown, SkillListReceived, ZoneSkillInfo,
};

pub fn damage_dealt(d: net::DamageDealt) -> DamageReceived {
    DamageReceived {
        src_id: d.src_id,
        target_id: d.target_id,
        server_tick: d.server_tick,
        src_speed: d.src_speed,
        dmg_speed: d.dmg_speed,
        damage: d.damage,
        div: d.div,
        type_: d.r#type,
        damage2: d.damage2,
    }
}

pub fn knockback(k: net::Knockback) -> KnockedBack {
    KnockedBack {
        unit_id: k.unit_id,
        dst_x: k.dst_x,
        dst_y: k.dst_y,
    }
}

pub fn skill_casting(s: net::SkillCasting) -> SkillCastStarted {
    SkillCastStarted {
        src_id: s.src_id,
        target_id: s.target_id,
        x: s.x,
        y: s.y,
        skill_id: s.skill_id,
        property: s.property,
        cast_time: s.cast_time,
    }
}

pub fn skill_damage(s: net::SkillDamage) -> SkillDamageReceived {
    SkillDamageReceived {
        skill_id: s.skill_id,
        level: s.level,
        src_id: s.src_id,
        target_id: s.target_id,
        server_tick: s.server_tick,
        damage: s.damage,
        div: s.div,
        type_: s.r#type,
        src_delay: s.src_delay,
        dst_delay: s.dst_delay,
    }
}

pub fn skill_effect(s: net::SkillEffect) -> SkillEffectShown {
    SkillEffectShown {
        skill_id: s.skill_id,
        level: s.level,
        src_id: s.src_id,
        target_id: s.target_id,
        result: s.result,
    }
}

pub fn cast_cancel(c: net::CastCancel) -> CastCancelled {
    CastCancelled { gid: c.gid }
}

pub fn skill_cooldown(s: net::SkillCooldown) -> SkillCooldownSet {
    SkillCooldownSet {
        skill_id: s.skill_id,
        tick: s.tick,
    }
}

pub fn ground_skill(g: net::GroundSkill) -> GroundSkillPlaced {
    GroundSkillPlaced {
        skill_id: g.skill_id,
        src_id: g.src_id,
        level: g.level,
        x: g.x,
        y: g.y,
        server_tick: g.server_tick,
    }
}

pub fn skill_list(l: net::SkillList) -> SkillListReceived {
    SkillListReceived {
        skills: l.skills.into_iter().map(skill_info).collect(),
    }
}

fn skill_info(s: net::SkillInfo) -> ZoneSkillInfo {
    ZoneSkillInfo {
        skill_id: s.skill_id,
        type_: s.r#type,
        level: s.level,
        sp: s.sp,
        range: s.range,
        name: s.name,
        upgradable: s.upgradable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_dealt_preserves_sign_and_type() {
        let received = damage_dealt(net::DamageDealt {
            src_id: 150001,
            target_id: 150002,
            server_tick: 123456,
            src_speed: 150,
            dmg_speed: 200,
            damage: -50,
            div: 1,
            r#type: 4,
            damage2: -25,
            is_sp_damage: false,
        });

        assert_eq!(received.src_id, 150001);
        assert_eq!(received.target_id, 150002);
        assert_eq!(received.server_tick, 123456);
        assert_eq!(received.src_speed, 150);
        assert_eq!(received.dmg_speed, 200);
        assert_eq!(received.damage, -50);
        assert_eq!(received.div, 1);
        assert_eq!(received.type_, 4);
        assert_eq!(received.damage2, -25);
    }

    #[test]
    fn knockback_maps_destination() {
        let knocked = knockback(net::Knockback {
            unit_id: 150001,
            dst_x: 88,
            dst_y: 99,
        });

        assert_eq!(knocked.unit_id, 150001);
        assert_eq!(knocked.dst_x, 88);
        assert_eq!(knocked.dst_y, 99);
    }

    #[test]
    fn skill_casting_maps_cast_bar() {
        let started = skill_casting(net::SkillCasting {
            src_id: 1,
            target_id: 2,
            x: 10,
            y: 20,
            skill_id: 5,
            property: 3,
            cast_time: 1500,
        });

        assert_eq!(started.src_id, 1);
        assert_eq!(started.target_id, 2);
        assert_eq!(started.x, 10);
        assert_eq!(started.y, 20);
        assert_eq!(started.skill_id, 5);
        assert_eq!(started.property, 3);
        assert_eq!(started.cast_time, 1500);
    }

    #[test]
    fn skill_damage_preserves_sign_and_type() {
        let received = skill_damage(net::SkillDamage {
            skill_id: 5,
            level: 10,
            src_id: 1,
            target_id: 2,
            server_tick: 777,
            damage: -100,
            div: 3,
            r#type: 8,
            src_delay: 50,
            dst_delay: 60,
        });

        assert_eq!(received.skill_id, 5);
        assert_eq!(received.level, 10);
        assert_eq!(received.damage, -100);
        assert_eq!(received.type_, 8);
        assert_eq!(received.div, 3);
        assert_eq!(received.src_delay, 50);
        assert_eq!(received.dst_delay, 60);
    }

    #[test]
    fn skill_effect_maps_result() {
        let shown = skill_effect(net::SkillEffect {
            skill_id: 5,
            level: 1,
            src_id: 1,
            target_id: 2,
            result: 1,
        });

        assert_eq!(shown.skill_id, 5);
        assert_eq!(shown.result, 1);
    }

    #[test]
    fn cast_cancel_maps_gid() {
        assert_eq!(cast_cancel(net::CastCancel { gid: 42 }).gid, 42);
    }

    #[test]
    fn skill_cooldown_maps_tick() {
        let cooldown = skill_cooldown(net::SkillCooldown {
            skill_id: 5,
            tick: 3000,
        });

        assert_eq!(cooldown.skill_id, 5);
        assert_eq!(cooldown.tick, 3000);
    }

    #[test]
    fn ground_skill_maps_cell() {
        let placed = ground_skill(net::GroundSkill {
            skill_id: 5,
            src_id: 1,
            level: 10,
            x: 30,
            y: 40,
            server_tick: 888,
        });

        assert_eq!(placed.skill_id, 5);
        assert_eq!(placed.src_id, 1);
        assert_eq!(placed.level, 10);
        assert_eq!(placed.x, 30);
        assert_eq!(placed.y, 40);
        assert_eq!(placed.server_tick, 888);
    }

    #[test]
    fn skill_list_maps_each_skill() {
        let received = skill_list(net::SkillList {
            skills: vec![
                net::SkillInfo {
                    skill_id: 1,
                    r#type: 2,
                    level: 3,
                    sp: 4,
                    range: 5,
                    name: "Bash".into(),
                    upgradable: true,
                },
                net::SkillInfo {
                    skill_id: 6,
                    r#type: 7,
                    level: 8,
                    sp: 9,
                    range: 10,
                    name: "Provoke".into(),
                    upgradable: false,
                },
            ],
        });

        assert_eq!(received.skills.len(), 2);
        assert_eq!(received.skills[0].skill_id, 1);
        assert_eq!(received.skills[0].type_, 2);
        assert_eq!(received.skills[0].name, "Bash");
        assert!(received.skills[0].upgradable);
        assert_eq!(received.skills[1].skill_id, 6);
        assert!(!received.skills[1].upgradable);
    }
}
