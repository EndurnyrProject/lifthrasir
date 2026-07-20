use super::events::{MapLoadCompleted, MapLoadingStarted};
use crate::core::state::GameState;
use crate::domain::system_sets::CharacterFlowSystems;
use crate::domain::world::map::MapData;
use crate::domain::world::spawn_context::MapSpawnContext;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(Resource)]
pub struct MapLoadingTimer {
    timer: Timer,
    pub(crate) map_name: String,
}

impl MapLoadingTimer {
    pub(crate) fn new(map_name: String) -> Self {
        Self {
            timer: Timer::from_seconds(30.0, TimerMode::Once),
            map_name,
        }
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadStart)
)]
pub fn start_map_loading_timer(
    mut events: MessageReader<MapLoadingStarted>,
    mut commands: Commands,
) {
    for event in events.read() {
        debug!("Starting map loading timeout timer for: {}", event.map_name);
        commands.insert_resource(MapLoadingTimer::new(event.map_name.clone()));
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadTimeout)
)]
pub fn detect_map_loading_timeout(
    timer: Option<ResMut<MapLoadingTimer>>,
    time: Res<Time>,
    maps: Query<&MapData>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let Some(mut loading) = timer else {
        return;
    };

    if !maps.is_empty() {
        debug!(
            "Map '{}' loaded successfully, removing timeout timer",
            loading.map_name
        );
        commands.remove_resource::<MapLoadingTimer>();
        return;
    }

    loading.timer.tick(time.delta());
    if !loading.timer.just_finished() {
        return;
    }

    error!(
        "Map loading timeout for '{}' - assets failed to load within 30 seconds",
        loading.map_name
    );
    commands.remove_resource::<MapLoadingTimer>();
    game_state.set(GameState::CharacterSelection);
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadDetect)
)]
pub fn detect_map_load_complete(
    maps: Query<Entity, Added<MapData>>,
    spawn_context: Option<Res<MapSpawnContext>>,
    mut events: MessageWriter<MapLoadCompleted>,
) {
    for entity in maps.iter() {
        let Some(context) = spawn_context.as_ref() else {
            warn!(
                "MapData spawned (entity {:?}) without MapSpawnContext - skipping",
                entity
            );
            continue;
        };

        debug!(
            "Map loading completed: {} (entity {:?})",
            context.map_name, entity
        );
        events.write(MapLoadCompleted {
            map_name: context.map_name.clone(),
        });
    }
}

#[auto_add_system(
    plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin,
    schedule = Update,
    config(in_set = CharacterFlowSystems::MapLoadComplete)
)]
pub fn handle_map_load_complete(
    mut events: MessageReader<MapLoadCompleted>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        debug!("Map '{}' loaded; entering the game world", event.map_name);
        game_state.set(GameState::InGame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timeout_uses_bevy_time_and_returns_to_character_selection() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_resource::<Time>();
        app.init_state::<GameState>();
        app.insert_resource(MapLoadingTimer::new("missing_map".into()));
        app.add_systems(Update, detect_map_loading_timeout);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(31));
        app.update();
        app.update();

        assert!(app.world().get_resource::<MapLoadingTimer>().is_none());
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::CharacterSelection
        );
    }
}
