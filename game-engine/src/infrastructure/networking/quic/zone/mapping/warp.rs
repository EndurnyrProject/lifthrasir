use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::MapChangeRequested;

pub fn map_move(m: net::MapMove) -> MapChangeRequested {
    MapChangeRequested {
        map_name: m.map_name,
        x: m.x,
        y: m.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_move_maps_name_and_cell() {
        let requested = map_move(net::MapMove {
            map_name: "prontera".into(),
            x: 150,
            y: 100,
        });

        assert_eq!(requested.map_name, "prontera");
        assert_eq!(requested.x, 150);
        assert_eq!(requested.y, 100);
    }
}
