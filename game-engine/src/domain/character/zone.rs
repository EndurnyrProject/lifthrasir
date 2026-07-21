use super::events::MapLoadingStarted;
use super::map_loading::MapLoadingTimer;
use crate::core::state::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::system_sets::CharacterFlowSystems;
use crate::domain::world::MapScoped;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::domain::world::warp::Warping;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::commands::{ConnectZone, LeaveZone};
use net_contract::events::{ZoneEntered, ZoneServerInfoReceived};
use net_contract::state::{UserSession, ZoneSession};

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneServerInfo)
)]
pub fn handle_zone_server_info(
    mut events: MessageReader<ZoneServerInfoReceived>,
    user_session: Option<Res<UserSession>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut connect_zone: MessageWriter<ConnectZone>,
) {
    for event in events.read() {
        let Some(session) = user_session.as_ref() else {
            error!("ZoneServerInfo received without a UserSession");
            continue;
        };

        let zone = &event.zone_server_info;
        info!("Connecting to zone server for map: {}", zone.map_name);
        connect_zone.write(ConnectZone {
            address: format!("{}:{}", zone.ip_string(), zone.port),
            account_id: session.tokens.account_id,
            login_id1: session.tokens.login_id1,
            login_id2: session.tokens.login_id2,
            sex: session.sex as u32,
            char_id: zone.char_id,
            zone_auth_token: zone.auth_token.clone(),
            map_name: zone.map_name.clone(),
        });
        game_state.set(GameState::Connecting);
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::ZoneConnection)
)]
pub fn handle_zone_entered(
    mut events: MessageReader<ZoneEntered>,
    session: Res<ZoneSession>,
    mut commands: Commands,
    mut map_loading_events: MessageWriter<MapLoadingStarted>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        info!(
            "Zone server accepted entry! Spawning at ({}, {}) facing {}",
            event.x, event.y, event.dir
        );

        commands.insert_resource(MapSpawnContext::new(
            session.map_name.clone(),
            event.x as u16,
            event.y as u16,
            session.char_id,
        ));
        map_loading_events.write(MapLoadingStarted {
            map_name: session.map_name.clone(),
        });
        game_state.set(GameState::Loading);
    }
}

type ZoneSessionEntities = Or<(With<LocalPlayer>, With<MapScoped>)>;

/// Clears all client-side zone state when returning to login.
#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = OnEnter(GameState::Login)
)]
pub fn teardown_zone_session_on_login(
    mut commands: Commands,
    mut leave_zone: MessageWriter<LeaveZone>,
    mut registry: ResMut<EntityRegistry>,
    world_entities: Query<Entity, ZoneSessionEntities>,
) {
    leave_zone.write(LeaveZone);
    commands.remove_resource::<MapSpawnContext>();
    commands.remove_resource::<MapLoadingTimer>();
    commands.remove_resource::<Warping>();

    for entity in world_entities.iter() {
        commands.entity(entity).despawn();
    }
    registry.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teardown_on_login_clears_session_and_world_entities() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<LeaveZone>();
        app.init_resource::<EntityRegistry>();
        app.insert_resource(MapSpawnContext::new("prontera".into(), 100, 100, 42));
        app.insert_resource(MapLoadingTimer::new("prontera".into()));
        app.add_systems(OnEnter(GameState::Login), teardown_zone_session_on_login);

        let player = app.world_mut().spawn(LocalPlayer).id();
        let terrain = app.world_mut().spawn(MapScoped).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .set_local_player(player, 42);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Login);
        app.update();

        assert!(app.world().get_resource::<MapSpawnContext>().is_none());
        assert!(app.world().get_resource::<MapLoadingTimer>().is_none());
        assert!(app.world().get_entity(player).is_err());
        assert!(app.world().get_entity(terrain).is_err());
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<LeaveZone>>()
                .drain()
                .count(),
            1
        );
        assert!(
            app.world()
                .resource::<EntityRegistry>()
                .local_player_entity()
                .is_none()
        );
    }
}
