use crate::proto::aesir::net;
use net_contract::events::{
    ParamChanged, SelfRespawned, StatRaised, UnitHpChanged, UnitResurrected, UnitSpriteChanged,
};

pub fn param_change(p: net::ParamChange) -> ParamChanged {
    ParamChanged {
        var: p.var_id,
        value: p.value,
    }
}

pub fn unit_hp(u: net::UnitHp) -> UnitHpChanged {
    UnitHpChanged {
        gid: u.id,
        hp: u.hp,
        max_hp: u.max_hp,
    }
}

pub fn stat_up_result(s: net::StatUpResult) -> StatRaised {
    StatRaised {
        stat_id: s.stat_id,
        ok: s.ok,
        value: s.value,
    }
}

pub fn sprite_change(s: net::SpriteChange) -> UnitSpriteChanged {
    UnitSpriteChanged {
        gid: s.gid,
        type_: s.r#type,
        val: s.val,
        val2: s.val2,
    }
}

pub fn resurrect(r: net::Resurrect) -> UnitResurrected {
    UnitResurrected {
        gid: r.gid,
        type_: r.r#type,
    }
}

pub fn respawn(r: net::Respawn) -> SelfRespawned {
    SelfRespawned { type_: r.r#type }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_change_unifies_into_u64_value() {
        let changed = param_change(net::ParamChange {
            var_id: 5,
            value: 4_294_967_296,
        });

        assert_eq!(changed.var, 5);
        assert_eq!(changed.value, 4_294_967_296);
    }

    #[test]
    fn unit_hp_maps_id_to_gid() {
        let hp = unit_hp(net::UnitHp {
            id: 150001,
            hp: 3000,
            max_hp: 4200,
        });

        assert_eq!(hp.gid, 150001);
        assert_eq!(hp.hp, 3000);
        assert_eq!(hp.max_hp, 4200);
    }

    #[test]
    fn stat_up_result_maps_ok_and_value() {
        let raised = stat_up_result(net::StatUpResult {
            stat_id: 13,
            ok: true,
            value: 91,
        });

        assert_eq!(raised.stat_id, 13);
        assert!(raised.ok);
        assert_eq!(raised.value, 91);
    }

    #[test]
    fn sprite_change_maps_type_and_vals() {
        let changed = sprite_change(net::SpriteChange {
            gid: 150001,
            r#type: 2,
            val: 13,
            val2: 0,
        });

        assert_eq!(changed.gid, 150001);
        assert_eq!(changed.type_, 2);
        assert_eq!(changed.val, 13);
        assert_eq!(changed.val2, 0);
    }

    #[test]
    fn resurrect_maps_type() {
        let resurrected = resurrect(net::Resurrect {
            gid: 150001,
            r#type: 1,
        });

        assert_eq!(resurrected.gid, 150001);
        assert_eq!(resurrected.type_, 1);
    }

    #[test]
    fn respawn_maps_type() {
        assert_eq!(respawn(net::Respawn { r#type: 0 }).type_, 0);
    }
}
