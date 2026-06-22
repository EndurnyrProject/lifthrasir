use crate::core::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};
use crate::infrastructure::networking::zone_messages::MapChangeRequested;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Transient marker: present iff the current `Loading` cycle is a warp, not a
/// first-entry. Inserted by `handle_map_change`; consumed and removed in Task 6's
/// `reposition_local_player` on `OnEnter(InGame)`.
#[derive(Resource)]
pub struct Warping;

/// Consume `MapChangeRequested` (server warp) and kick the existing entry cycle.
///
/// Repoints `MapSpawnContext` at the new map/cell (keeping `character_id`), resets
/// the zone handshake to `Entering` so `zone_send_map_loaded`'s guard re-fires on
/// the new `Added<MapData>`, flags the cycle as a warp, and flips to `Loading`.
/// `MapSpawnContext` is guaranteed present in-game (the entry path inserts it), so
/// a missing resource here fails loudly per the critical-systems guideline.
#[auto_add_system(
    plugin = crate::plugins::world_domain_plugin::WorldDomainPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_map_change(
    mut events: MessageReader<MapChangeRequested>,
    mut ctx: ResMut<MapSpawnContext>,
    mut zone_state: ResMut<QuicZoneState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for m in events.read() {
        ctx.map_name = m.map_name.clone();
        ctx.spawn_x = m.x as u16;
        ctx.spawn_y = m.y as u16;
        zone_state.phase = ZonePhase::Entering;
        commands.insert_resource(Warping);
        next_state.set(GameState::Loading);
    }
}

/// Reposition the persisted local player after a warp.
///
/// On a warp the `LocalPlayer` entity (and its sprite hierarchy + `CharacterStatus`)
/// survives map teardown, so the full first-entry spawn is skipped. This only moves
/// the existing player to the destination cell, then clears the `Warping` flag. It is
/// a no-op on first entry (no `Warping` present). `spawn_coords_to_world_position`
/// ignores the map-dim args, so `0, 0` is correct.
#[auto_add_system(
    plugin = crate::plugins::world_domain_plugin::WorldDomainPlugin,
    schedule = OnEnter(GameState::InGame)
)]
pub fn reposition_local_player(
    mut commands: Commands,
    warping: Option<Res<Warping>>,
    ctx: Res<MapSpawnContext>,
    mut players: Query<&mut Transform, With<LocalPlayer>>,
) {
    if warping.is_none() {
        return;
    }

    match players.single_mut() {
        Ok(mut transform) => {
            transform.translation = spawn_coords_to_world_position(ctx.spawn_x, ctx.spawn_y, 0, 0);
        }
        Err(_) => warn!("reposition_local_player: warping but no LocalPlayer entity found"),
    }

    commands.remove_resource::<Warping>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn warp_repoints_context_resets_phase_and_queues_loading() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.insert_resource(MapSpawnContext::new("prontera".into(), 100, 200, 42));
        app.insert_resource(QuicZoneState {
            phase: ZonePhase::Playing,
            ..Default::default()
        });
        app.add_message::<MapChangeRequested>();
        app.add_systems(Update, handle_map_change);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        app.world_mut().write_message(MapChangeRequested {
            map_name: "geffen".into(),
            x: 50,
            y: 60,
        });
        app.update();

        let ctx = app.world().resource::<MapSpawnContext>();
        assert_eq!(ctx.map_name, "geffen");
        assert_eq!(ctx.spawn_x, 50);
        assert_eq!(ctx.spawn_y, 60);
        assert_eq!(ctx.character_id, 42);

        assert_eq!(
            app.world().resource::<QuicZoneState>().phase,
            ZonePhase::Entering
        );
        assert!(app.world().get_resource::<Warping>().is_some());

        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Loading)
        ));
    }

    #[test]
    fn reposition_moves_player_to_new_cell_and_clears_warping() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.insert_resource(MapSpawnContext::new("geffen".into(), 50, 60, 42));
        app.insert_resource(Warping);
        app.add_systems(OnEnter(GameState::InGame), reposition_local_player);

        let player = app
            .world_mut()
            .spawn((LocalPlayer, Transform::from_xyz(1.0, 2.0, 3.0)))
            .id();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        let expected = spawn_coords_to_world_position(50, 60, 0, 0);
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation,
            expected
        );
        assert!(app.world().get_resource::<Warping>().is_none());
    }

    #[test]
    fn reposition_is_noop_without_warping() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.insert_resource(MapSpawnContext::new("geffen".into(), 50, 60, 42));
        app.add_systems(OnEnter(GameState::InGame), reposition_local_player);

        let origin = Transform::from_xyz(1.0, 2.0, 3.0);
        let player = app.world_mut().spawn((LocalPlayer, origin)).id();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation,
            origin.translation
        );
    }
}
