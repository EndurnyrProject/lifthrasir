use crate::core::GameState;
use crate::domain::character::events::MapLoadCompleted;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};
use net_contract::events::StatusEffectChanged;
use net_contract::state::ZoneSession;
use std::collections::HashMap;
use std::time::Duration;

/// The local player's active status effects, keyed by EFST id.
///
/// Filtered to `ZoneSession.char_id` rather than tracked on the `LocalPlayer`
/// entity: statuses can arrive over the network before that entity spawns, and a
/// resource sidesteps the spawn race (same class of bug as HP-param buffering).
#[derive(Resource, Default, Debug)]
#[auto_init_resource(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct LocalStatuses {
    pub active: HashMap<u32, ActiveStatus>,
}

/// One active status. The server is one-shot (timing only at apply); the client
/// computes `expires_at` once and counts down locally against `Time`.
#[derive(Debug, Clone)]
pub struct ActiveStatus {
    pub total_ms: u32,
    pub expires_at: Option<Duration>,
    pub permanent: bool,
}

/// Folds the widened `StatusEffectChanged` events for the local player into
/// [`LocalStatuses`]. A `remain_ms` of `0` means the status never expires locally
/// (permanent); a re-apply replaces the entry, refreshing its expiry.
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn track_local_status_icons(
    mut events: MessageReader<StatusEffectChanged>,
    session: Res<ZoneSession>,
    time: Res<Time>,
    mut local: ResMut<LocalStatuses>,
) {
    for event in events.read() {
        if event.unit_id != session.char_id {
            continue;
        }

        if !event.on {
            local.active.remove(&event.efst);
            continue;
        }

        local.active.insert(
            event.efst,
            ActiveStatus {
                total_ms: event.total_ms,
                permanent: event.remain_ms == 0,
                expires_at: (event.remain_ms != 0)
                    .then(|| time.elapsed() + Duration::from_millis(event.remain_ms as u64)),
            },
        );
    }
}

/// Drops timed statuses whose computed `expires_at` has passed. Permanent
/// statuses (`expires_at == None`) are never removed here.
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update
)]
pub fn expire_local_statuses(time: Res<Time>, mut local: ResMut<LocalStatuses>) {
    let now = time.elapsed();
    local
        .active
        .retain(|_, status| status.expires_at.is_none_or(|at| at > now));
}

/// Clears the bar when the player leaves the world.
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = OnExit(GameState::InGame)
)]
pub fn clear_local_statuses(mut local: ResMut<LocalStatuses>) {
    local.active.clear();
}

/// Clears the bar on a map change. Ordered before [`track_local_status_icons`]
/// so the server's same-frame resync (`StatusChange{on:true}` right after map
/// load) repopulates the bar instead of being swallowed by the clear.
#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(before = track_local_status_icons)
)]
pub fn clear_local_statuses_on_map_change(
    mut events: MessageReader<MapLoadCompleted>,
    mut local: ResMut<LocalStatuses>,
) {
    if events.read().next().is_some() {
        local.active.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAR_ID: u32 = 42;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<StatusEffectChanged>()
            .add_message::<MapLoadCompleted>()
            .init_resource::<Time>()
            .init_resource::<LocalStatuses>()
            .insert_resource(ZoneSession {
                char_id: CHAR_ID,
                ..default()
            })
            .add_systems(
                Update,
                (
                    clear_local_statuses_on_map_change.before(track_local_status_icons),
                    track_local_status_icons,
                    expire_local_statuses,
                ),
            );
        app
    }

    fn emit_status(app: &mut App, unit_id: u32, efst: u32, on: bool, remain_ms: u32) {
        app.world_mut()
            .resource_mut::<Messages<StatusEffectChanged>>()
            .write(StatusEffectChanged {
                unit_id,
                efst,
                on,
                total_ms: remain_ms,
                remain_ms,
            });
    }

    fn active(app: &App) -> &HashMap<u32, ActiveStatus> {
        &app.world().resource::<LocalStatuses>().active
    }

    #[test]
    fn timed_status_gets_finite_expiry() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 5000);
        app.update();

        let status = active(&app).get(&10).expect("status tracked");
        assert!(!status.permanent);
        assert_eq!(status.expires_at, Some(Duration::from_millis(5000)));
    }

    #[test]
    fn zero_remain_is_permanent() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 0);
        app.update();

        let status = active(&app).get(&10).expect("status tracked");
        assert!(status.permanent);
        assert_eq!(status.expires_at, None);
    }

    #[test]
    fn off_removes_status() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 5000);
        app.update();
        emit_status(&mut app, CHAR_ID, 10, false, 0);
        app.update();

        assert!(active(&app).is_empty());
    }

    #[test]
    fn other_units_are_ignored() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID + 1, 10, true, 5000);
        app.update();

        assert!(active(&app).is_empty());
    }

    #[test]
    fn expired_status_is_dropped_permanent_is_kept() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 1000);
        emit_status(&mut app, CHAR_ID, 20, true, 0);
        app.update();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1500));
        app.update();

        assert!(!active(&app).contains_key(&10), "timed status expired");
        assert!(active(&app).contains_key(&20), "permanent status kept");
    }

    #[test]
    fn future_expiry_is_kept() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 5000);
        app.update();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1000));
        app.update();

        assert!(active(&app).contains_key(&10));
    }

    #[test]
    fn clear_empties_then_track_repopulates() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 5000);
        app.update();
        assert!(!active(&app).is_empty());

        app.world_mut().run_system_cached(clear_local_statuses).ok();
        assert!(active(&app).is_empty());

        emit_status(&mut app, CHAR_ID, 11, true, 5000);
        app.update();
        assert!(active(&app).contains_key(&11));
    }

    #[test]
    fn map_change_clear_does_not_eat_same_frame_resync() {
        let mut app = app();
        emit_status(&mut app, CHAR_ID, 10, true, 5000);
        app.update();

        app.world_mut()
            .resource_mut::<Messages<MapLoadCompleted>>()
            .write(MapLoadCompleted {
                map_name: "prontera".to_string(),
            });
        emit_status(&mut app, CHAR_ID, 99, true, 5000);
        app.update();

        assert!(
            !active(&app).contains_key(&10),
            "pre-warp status cleared on map change"
        );
        assert!(
            active(&app).contains_key(&99),
            "same-frame resync survives the clear"
        );
    }
}
