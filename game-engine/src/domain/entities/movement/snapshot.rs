//! Snapshot ingest for remote-entity interpolation.
//!
//! aesir broadcasts per-map DELTA snapshots over the unreliable `:snapshots`
//! channel instead of per-step move packets. This module is the ingest half:
//! it estimates server time on the client ([`ServerClock`]) and fills a small
//! per-entity sample buffer ([`SnapshotBuffer`]) from [`SnapshotReceived`].
//!
//! It does NOT move entities — interpolation reads these buffers in a later step.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::core::state::GameState;
use crate::domain::entities::registry::EntityRegistry;
use net_contract::events::SnapshotReceived;

/// Max samples retained per entity. ~6 covers a couple of snapshot intervals
/// plus interpolation delay; older samples are useless once we've moved past them.
const BUFFER_CAPACITY: usize = 6;

/// Client-side estimate of server time. `server_tick` is server wall-clock
/// milliseconds (`System.system_time(:millisecond)`), so the offset against the
/// client's monotonic [`Time<Real>`] clock lets us place samples on a shared timeline.
///
/// Latest snapshot wins — no smoothing for v1.
#[derive(Resource, Default, Debug)]
#[auto_init_resource(plugin = crate::app::movement_plugin::MovementDomainPlugin)]
pub struct ServerClock {
    /// `server_tick - client_now_ms` from the most recent snapshot.
    pub offset_ms: i64,
}

impl ServerClock {
    /// Estimated server time for a given client `Time<Real>` reading.
    pub fn server_now_ms(&self, client_now_ms: i64) -> i64 {
        client_now_ms + self.offset_ms
    }
}

/// One position/state sample for a remote entity, stamped with server time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotSample {
    pub server_tick: u64,
    pub x: u16,
    pub y: u16,
    pub dir: u8,
    pub move_state: u8,
}

/// Ring of recent [`SnapshotSample`]s for a remote entity, ordered by `server_tick`.
#[derive(Component, Default, Debug)]
pub struct SnapshotBuffer {
    samples: VecDeque<SnapshotSample>,
}

impl SnapshotBuffer {
    /// Create a buffer seeded with a first sample.
    pub fn with_sample(sample: SnapshotSample) -> Self {
        let mut buffer = Self::default();
        buffer.push(sample);
        buffer
    }

    /// Append a sample, dropping out-of-order/duplicate ticks and trimming to capacity.
    pub fn push(&mut self, sample: SnapshotSample) {
        if let Some(last) = self.samples.back() {
            if sample.server_tick <= last.server_tick {
                return;
            }
        }
        self.samples.push_back(sample);
        if self.samples.len() > BUFFER_CAPACITY {
            self.samples.pop_front();
        }
    }

    /// Samples oldest-first.
    pub fn samples(&self) -> &VecDeque<SnapshotSample> {
        &self.samples
    }
}

/// Fills per-entity [`SnapshotBuffer`]s from incoming snapshots and re-estimates
/// the [`ServerClock`] offset. Skips the local player and not-yet-spawned entities.
/// Does not touch `Transform`.
#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn ingest_snapshots_system(
    mut commands: Commands,
    mut snapshots: MessageReader<SnapshotReceived>,
    time: Res<Time<Real>>,
    registry: Res<EntityRegistry>,
    mut buffers: Query<&mut SnapshotBuffer>,
) {
    let client_now_ms = time.elapsed().as_millis() as i64;

    for snapshot in snapshots.read() {
        let offset_ms = snapshot.server_tick as i64 - client_now_ms;
        commands.insert_resource(ServerClock { offset_ms });

        let mut buffered = 0;
        let mut skipped_unknown = 0;
        let mut skipped_local = 0;

        for entity_snapshot in &snapshot.entities {
            let Some(entity) = registry.get_entity(entity_snapshot.id) else {
                skipped_unknown += 1;
                continue;
            };
            if registry.is_local_player(entity) {
                skipped_local += 1;
                continue;
            }

            let sample = SnapshotSample {
                server_tick: snapshot.server_tick,
                x: entity_snapshot.x as u16,
                y: entity_snapshot.y as u16,
                dir: entity_snapshot.dir as u8,
                move_state: entity_snapshot.move_state as u8,
            };

            if let Ok(mut buffer) = buffers.get_mut(entity) {
                buffer.push(sample);
            } else {
                commands
                    .entity(entity)
                    .insert(SnapshotBuffer::with_sample(sample));
            }
            buffered += 1;
        }

        debug!(
            "[snapshot] ingest tick={} offset_ms={} buffered={} skipped_local={} skipped_unknown={}",
            snapshot.server_tick, offset_ms, buffered, skipped_local, skipped_unknown
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::ZoneSnapshotEntity;

    fn sample(tick: u64) -> SnapshotSample {
        SnapshotSample {
            server_tick: tick,
            x: 1,
            y: 2,
            dir: 3,
            move_state: 1,
        }
    }

    #[test]
    fn push_accepts_increasing_ticks() {
        let mut buffer = SnapshotBuffer::default();
        buffer.push(sample(10));
        buffer.push(sample(20));
        buffer.push(sample(30));

        let ticks: Vec<u64> = buffer.samples().iter().map(|s| s.server_tick).collect();
        assert_eq!(ticks, vec![10, 20, 30]);
    }

    #[test]
    fn push_ignores_non_increasing_ticks() {
        let mut buffer = SnapshotBuffer::default();
        buffer.push(sample(20));
        buffer.push(sample(20)); // duplicate
        buffer.push(sample(10)); // out of order

        let ticks: Vec<u64> = buffer.samples().iter().map(|s| s.server_tick).collect();
        assert_eq!(ticks, vec![20]);
    }

    #[test]
    fn push_caps_length_dropping_oldest() {
        let mut buffer = SnapshotBuffer::default();
        for tick in 1..=(BUFFER_CAPACITY as u64 + 3) {
            buffer.push(sample(tick * 10));
        }

        assert_eq!(buffer.samples().len(), BUFFER_CAPACITY);
        let oldest = buffer.samples().front().map(|s| s.server_tick);
        let newest = buffer.samples().back().map(|s| s.server_tick);
        assert_eq!(oldest, Some(40)); // first 3 (10,20,30) dropped
        assert_eq!(newest, Some((BUFFER_CAPACITY as u64 + 3) * 10));
    }

    #[test]
    fn server_clock_offset_and_now() {
        let clock = ServerClock {
            offset_ms: 5_000 - 1_200,
        };
        assert_eq!(clock.offset_ms, 3_800);
        assert_eq!(clock.server_now_ms(1_200), 5_000);
        assert_eq!(clock.server_now_ms(2_000), 5_800);
    }

    #[test]
    fn ingest_buffers_remote_skips_local_and_unknown() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<GameState>()
            .add_message::<SnapshotReceived>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, ingest_snapshots_system);

        let remote = app.world_mut().spawn_empty().id();
        let local = app.world_mut().spawn_empty().id();
        {
            let mut registry = app.world_mut().resource_mut::<EntityRegistry>();
            registry.register_entity(1001, remote);
            registry.set_local_player(local, 2002);
        }

        // System is gated on GameState::InGame.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        let snapshot = SnapshotReceived {
            server_tick: 7_777,
            entities: vec![
                ZoneSnapshotEntity {
                    id: 1001,
                    x: 50,
                    y: 60,
                    dir: 2,
                    move_state: 1,
                    hp_pct: 100,
                },
                ZoneSnapshotEntity {
                    id: 2002, // local player, must be skipped
                    x: 10,
                    y: 10,
                    dir: 0,
                    move_state: 0,
                    hp_pct: 100,
                },
                ZoneSnapshotEntity {
                    id: 9999, // unregistered, must be skipped
                    x: 1,
                    y: 1,
                    dir: 0,
                    move_state: 0,
                    hp_pct: 100,
                },
            ],
        };
        app.world_mut()
            .resource_mut::<Messages<SnapshotReceived>>()
            .write(snapshot);
        app.update();

        let buffer = app.world().get::<SnapshotBuffer>(remote);
        assert!(buffer.is_some(), "remote entity should get a buffer");
        let buffer = buffer.expect("checked above");
        assert_eq!(buffer.samples().len(), 1);
        let first = buffer.samples().front().expect("one sample");
        assert_eq!(first.x, 50);
        assert_eq!(first.y, 60);
        assert_eq!(first.server_tick, 7_777);

        assert!(
            app.world().get::<SnapshotBuffer>(local).is_none(),
            "local player must not get a buffer"
        );

        assert_ne!(app.world().resource::<ServerClock>().offset_ms, 0);
    }
}
