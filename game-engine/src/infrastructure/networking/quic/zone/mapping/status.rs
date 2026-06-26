use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::StatusEffectChanged;

pub fn status_change(s: net::StatusChange) -> StatusEffectChanged {
    StatusEffectChanged {
        unit_id: s.unit_id,
        efst: s.efst,
        on: s.on,
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
}
