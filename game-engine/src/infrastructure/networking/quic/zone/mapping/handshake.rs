use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::ZoneEntered;

pub fn enter_ack(e: net::EnterAck) -> ZoneEntered {
    ZoneEntered {
        account_id: e.account_id,
        x: e.x,
        y: e.y,
        dir: e.dir,
        start_time: e.start_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_ack_maps_field_for_field() {
        let entered = enter_ack(net::EnterAck {
            account_id: 2000001,
            x: 150,
            y: 99,
            dir: 4,
            start_time: 123456789,
            font: 0,
        });

        assert_eq!(entered.account_id, 2000001);
        assert_eq!(entered.x, 150);
        assert_eq!(entered.y, 99);
        assert_eq!(entered.dir, 4);
        assert_eq!(entered.start_time, 123456789);
    }
}
