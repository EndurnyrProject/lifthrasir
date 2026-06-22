//! Entity interpolation for remote units.
//!
//! aesir broadcasts remote movement only as delta snapshots (see [`super::snapshot`]),
//! so remote entities have no per-step move target to interpolate. This module renders
//! them "in the past": it places each remote entity at where the server says it was
//! [`INTERP_DELAY_MS`] ago, lerping between the two bracketing [`SnapshotSample`]s.
//!
//! This is entity interpolation, NOT prediction — the local player is untouched (it uses
//! `SelfMove` via [`super::systems::interpolate_movement_system`]).

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use moonshine_behavior::prelude::*;

use super::components::MovementState;
use super::snapshot::{ServerClock, SnapshotBuffer, SnapshotSample};
use crate::core::state::GameState;
use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
use crate::domain::entities::character::states::AnimationState;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::system_sets::MovementSystems;

/// How far in the past we render remote entities, in milliseconds. Larger = smoother
/// under jitter/packet loss but more visibly behind; one snapshot interval is typical.
const INTERP_DELAY_MS: i64 = 100;

/// Resolved interpolation result for one entity at a given render time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpOutput {
    /// Interpolated cell position (fractional, in RO cell space).
    pub x: f32,
    pub y: f32,
    /// Facing taken from the sample being moved toward (server `dir`, RO sprite convention).
    pub dir: u8,
    /// Whether the entity is in motion at this render time.
    pub moving: bool,
}

/// Pure bracket-and-lerp: given samples ordered oldest-first by `server_tick`, return the
/// interpolated state at `render_ms`. `None` only for an empty buffer.
///
/// Edge cases:
/// - `render_ms` before the oldest sample → snap to the oldest cell (not moving).
/// - `render_ms` after the newest sample, or a single sample → hold at the newest cell,
///   `moving` from the newest sample's `move_state`.
pub fn sample_at(samples: &VecDeque<SnapshotSample>, render_ms: i64) -> Option<InterpOutput> {
    let oldest = samples.front()?;
    let newest = samples.back().expect("non-empty: front() succeeded");

    if render_ms <= oldest.server_tick as i64 {
        return Some(InterpOutput {
            x: oldest.x as f32,
            y: oldest.y as f32,
            dir: oldest.dir,
            moving: false,
        });
    }

    if render_ms >= newest.server_tick as i64 {
        return Some(InterpOutput {
            x: newest.x as f32,
            y: newest.y as f32,
            dir: newest.dir,
            moving: newest.move_state != 0,
        });
    }

    // render_ms is strictly inside the buffer's span: find the bracketing pair.
    let pair = samples.iter().zip(samples.iter().skip(1)).find(|(s0, s1)| {
        (s0.server_tick as i64) <= render_ms && render_ms <= (s1.server_tick as i64)
    });
    let (s0, s1) = pair.expect("render_ms within span must bracket a pair");

    let span = (s1.server_tick - s0.server_tick) as f32;
    let t = if span > 0.0 {
        ((render_ms - s0.server_tick as i64) as f32 / span).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let moving = s0.x != s1.x || s0.y != s1.y || s1.move_state != 0;

    Some(InterpOutput {
        x: s0.x as f32 + (s1.x as f32 - s0.x as f32) * t,
        y: s0.y as f32 + (s1.y as f32 - s0.y as f32) * t,
        dir: s1.dir,
        moving,
    })
}

/// Drives remote entities by interpolating their [`SnapshotBuffer`] at `now - INTERP_DELAY_MS`.
/// Writes `Transform.translation.x/.z` only (terrain owns `.y`, like
/// [`super::systems::interpolate_movement_system`]). Skips the local player.
#[auto_add_system(
    plugin = crate::app::movement_plugin::MovementDomainPlugin,
    schedule = Update,
    config(
        in_set = MovementSystems::Interpolate,
        after = super::snapshot::ingest_snapshots_system,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn interpolate_remote_entities_system(
    clock: Res<ServerClock>,
    time: Res<Time<Real>>,
    registry: Res<EntityRegistry>,
    mut behaviors: Query<BehaviorMut<AnimationState>>,
    mut query: Query<(
        Entity,
        &SnapshotBuffer,
        &mut Transform,
        &mut CharacterDirection,
        &mut MovementState,
    )>,
) {
    let client_now_ms = time.elapsed().as_millis() as i64;
    let render_ms = clock.server_now_ms(client_now_ms) - INTERP_DELAY_MS;

    let mut with_buffer = 0;
    let mut moving = 0;

    for (entity, buffer, mut transform, mut direction, mut state) in query.iter_mut() {
        if registry.is_local_player(entity) {
            continue;
        }
        with_buffer += 1;

        let Some(output) = sample_at(buffer.samples(), render_ms) else {
            continue;
        };
        if output.moving {
            moving += 1;
        }

        // spawn_coords_to_world_position is linear in (x, y); replicate its mapping for the
        // fractional cell so motion is smooth between integer cells.
        let world = world_from_cell(output.x, output.y);
        transform.translation.x = world.x;
        transform.translation.z = world.z;

        let facing = Direction::from_u8(output.dir);
        if direction.facing != facing {
            direction.facing = facing;
        }

        let next = if output.moving {
            MovementState::Moving
        } else {
            MovementState::Idle
        };
        if *state != next {
            debug!(
                "[snapshot] entity {:?} {:?} -> {:?} render_ms={} cell=({:.1},{:.1}) dir={} samples={}",
                entity,
                *state,
                next,
                render_ms,
                output.x,
                output.y,
                output.dir,
                buffer.samples().len()
            );
            *state = next;
            drive_walk_animation(&mut behaviors, entity, next);
        }
    }

    if with_buffer > 0 {
        debug!(
            "[snapshot] interpolate render_ms={} with_buffer={} moving={}",
            render_ms, with_buffer, moving
        );
    }
}

/// World position for a fractional cell. `spawn_coords_to_world_position` only takes
/// integer cells, so we replicate its linear `cell * 5.0` mapping for the fractional case.
fn world_from_cell(x: f32, y: f32) -> Vec3 {
    const RO_UNITS_PER_CELL: f32 = 5.0;
    Vec3::new(x * RO_UNITS_PER_CELL, 0.0, y * RO_UNITS_PER_CELL)
}

/// Mirrors the local player's walk-animation transitions (see
/// [`super::systems::handle_movement_confirmed_system`]): start `Walking` when motion begins
/// and return to `Idle` when it stops, without clobbering combat states (Attacking/Hit/Dead)
/// that own the FSM. `AnimationState` is a moonshine behavior; it must change via
/// [`BehaviorMut`], never a direct insert.
fn drive_walk_animation(
    behaviors: &mut Query<BehaviorMut<AnimationState>>,
    entity: Entity,
    next: MovementState,
) {
    let Ok(mut behavior) = behaviors.get_mut(entity) else {
        return;
    };
    match next {
        MovementState::Moving if *behavior.current() == AnimationState::Idle => {
            behavior.start(AnimationState::Walking);
        }
        MovementState::Idle if *behavior.current() == AnimationState::Walking => {
            behavior.start(AnimationState::Idle);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(tick: u64, x: u16, y: u16, dir: u8, move_state: u8) -> SnapshotSample {
        SnapshotSample {
            server_tick: tick,
            x,
            y,
            dir,
            move_state,
        }
    }

    #[test]
    fn empty_buffer_is_none() {
        let samples = VecDeque::new();
        assert_eq!(sample_at(&samples, 100), None);
    }

    #[test]
    fn midpoint_interpolates() {
        let mut samples = VecDeque::new();
        samples.push_back(s(100, 10, 20, 6, 1));
        samples.push_back(s(200, 20, 40, 6, 1));

        let out = sample_at(&samples, 150).expect("bracketed");
        assert!((out.x - 15.0).abs() < 1e-3, "x = {}", out.x);
        assert!((out.y - 30.0).abs() < 1e-3, "y = {}", out.y);
        assert_eq!(out.dir, 6, "faces the sample moved toward");
        assert!(out.moving);
    }

    #[test]
    fn before_oldest_snaps_to_oldest() {
        let mut samples = VecDeque::new();
        samples.push_back(s(100, 10, 20, 4, 1));
        samples.push_back(s(200, 20, 40, 6, 1));

        let out = sample_at(&samples, 50).expect("snap");
        assert_eq!((out.x, out.y), (10.0, 20.0));
        assert_eq!(out.dir, 4);
        assert!(!out.moving, "snapped to oldest, treated as settled");
    }

    #[test]
    fn after_newest_holds_at_newest() {
        let mut samples = VecDeque::new();
        samples.push_back(s(100, 10, 20, 4, 1));
        samples.push_back(s(200, 20, 40, 6, 0)); // newest standing

        let out = sample_at(&samples, 999).expect("hold");
        assert_eq!((out.x, out.y), (20.0, 40.0));
        assert_eq!(out.dir, 6);
        assert!(!out.moving, "newest move_state=0 => idle");
    }

    #[test]
    fn after_newest_still_moving_when_move_state_set() {
        let mut samples = VecDeque::new();
        samples.push_back(s(200, 20, 40, 6, 1)); // single, still moving

        let out = sample_at(&samples, 999).expect("hold single");
        assert_eq!((out.x, out.y), (20.0, 40.0));
        assert!(
            out.moving,
            "single sample with move_state=1 holds as moving"
        );
    }

    #[test]
    fn single_sample_holds() {
        let mut samples = VecDeque::new();
        samples.push_back(s(200, 20, 40, 6, 0));

        // before, at, and after the only tick all resolve to that cell.
        for render_ms in [0, 200, 5_000] {
            let out = sample_at(&samples, render_ms).expect("single");
            assert_eq!((out.x, out.y), (20.0, 40.0));
            assert_eq!(out.dir, 6);
        }
    }
}
