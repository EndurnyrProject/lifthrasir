use crate::proto::aesir::net;
use net_contract::events::{StatusEffectChanged, UnitStateChanged};

pub fn status_change(s: net::StatusChange) -> StatusEffectChanged {
    StatusEffectChanged {
        unit_id: s.unit_id,
        efst: s.efst,
        on: s.on,
        total_ms: s.total_ms,
        remain_ms: s.remain_ms,
    }
}

pub fn unit_state_change(s: net::UnitStateChange) -> UnitStateChanged {
    UnitStateChanged {
        unit_id: s.unit_id,
        body_state: s.body_state,
        health_state: s.health_state,
        effect_state: s.effect_state,
        virtue: s.virtue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_change_maps_efst_and_toggle() {
        let applied = status_change(net::StatusChange {
            unit_id: 150001,
            efst: 29,
            on: true,
            total_ms: 0,
            remain_ms: 0,
            val1: 0,
            val2: 0,
            val3: 0,
        });

        assert_eq!(applied.unit_id, 150001);
        assert_eq!(applied.efst, 29);
        assert!(applied.on);

        let removed = status_change(net::StatusChange {
            unit_id: 150001,
            efst: 29,
            on: false,
            ..Default::default()
        });

        assert!(!removed.on);
    }

    #[test]
    fn status_change_maps_duration_timing() {
        let timed = status_change(net::StatusChange {
            unit_id: 150001,
            efst: 10,
            on: true,
            total_ms: 60_000,
            remain_ms: 45_000,
            val1: 0,
            val2: 0,
            val3: 0,
        });

        assert_eq!(timed.total_ms, 60_000);
        assert_eq!(timed.remain_ms, 45_000);
    }

    #[test]
    fn unit_state_change_maps_all_state_fields() {
        let applied = unit_state_change(net::UnitStateChange {
            unit_id: 150001,
            body_state: 1,
            health_state: 2,
            effect_state: 4,
            virtue: 8,
        });

        assert_eq!(applied.unit_id, 150001);
        assert_eq!(applied.body_state, 1);
        assert_eq!(applied.health_state, 2);
        assert_eq!(applied.effect_state, 4);
        assert_eq!(applied.virtue, 8);
    }
}
