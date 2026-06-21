use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{SelfMoved, UnitMoveStopped};

pub fn self_move(m: net::SelfMove) -> SelfMoved {
    SelfMoved {
        src_x: m.src_x,
        src_y: m.src_y,
        dst_x: m.dst_x,
        dst_y: m.dst_y,
        start_time: m.start_time,
    }
}

pub fn move_stop(m: net::MoveStop) -> UnitMoveStopped {
    UnitMoveStopped {
        gid: m.gid,
        x: m.x,
        y: m.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_move_maps_src_and_dst() {
        let moved = self_move(net::SelfMove {
            src_x: 10,
            src_y: 20,
            dst_x: 30,
            dst_y: 40,
            start_time: 999,
        });

        assert_eq!(moved.src_x, 10);
        assert_eq!(moved.src_y, 20);
        assert_eq!(moved.dst_x, 30);
        assert_eq!(moved.dst_y, 40);
        assert_eq!(moved.start_time, 999);
    }

    #[test]
    fn move_stop_maps_gid_and_cell() {
        let stopped = move_stop(net::MoveStop {
            gid: 150001,
            x: 55,
            y: 66,
        });

        assert_eq!(stopped.gid, 150001);
        assert_eq!(stopped.x, 55);
        assert_eq!(stopped.y, 66);
    }
}
