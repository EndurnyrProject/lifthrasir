use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{SnapshotReceived, ZoneSnapshotEntity};

pub fn snapshot(s: net::Snapshot) -> SnapshotReceived {
    SnapshotReceived {
        server_tick: s.server_tick,
        entities: s.entities.into_iter().map(snapshot_entity).collect(),
    }
}

fn snapshot_entity(e: net::SnapshotEntity) -> ZoneSnapshotEntity {
    ZoneSnapshotEntity {
        id: e.id,
        x: e.x,
        y: e.y,
        dir: e.dir,
        move_state: e.move_state,
        hp_pct: e.hp_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_maps_tick_and_entities() {
        let received = snapshot(net::Snapshot {
            server_tick: 123456,
            entities: vec![
                net::SnapshotEntity {
                    id: 150001,
                    x: 100,
                    y: 200,
                    dir: 4,
                    move_state: 1,
                    hp_pct: 80,
                },
                net::SnapshotEntity {
                    id: 150002,
                    x: 50,
                    y: 60,
                    dir: 0,
                    move_state: 0,
                    hp_pct: 100,
                },
            ],
        });

        assert_eq!(received.server_tick, 123456);
        assert_eq!(received.entities.len(), 2);

        let first = received.entities[0];
        assert_eq!(first.id, 150001);
        assert_eq!(first.x, 100);
        assert_eq!(first.y, 200);
        assert_eq!(first.dir, 4);
        assert_eq!(first.move_state, 1);
        assert_eq!(first.hp_pct, 80);

        assert_eq!(received.entities[1].id, 150002);
        assert_eq!(received.entities[1].hp_pct, 100);
    }

    #[test]
    fn snapshot_empty_has_no_entities() {
        let received = snapshot(net::Snapshot {
            server_tick: 1,
            entities: vec![],
        });

        assert_eq!(received.server_tick, 1);
        assert!(received.entities.is_empty());
    }
}
