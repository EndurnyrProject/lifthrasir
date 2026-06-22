use crate::core::GameState;
use crate::domain::entities::registry::EntityRegistry;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Marker for entities whose lifetime is the current map.
///
/// Every map-content entity (terrain, models, lights, water, the map loader,
/// and remote network entities) carries this so a single despawn system can
/// tear them all down on map exit. The local player is NOT map-scoped.
#[derive(Component, Debug)]
pub struct MapScoped;

/// Tear down every map-scoped entity on map exit and drop stale remote registry
/// entries, keeping the local player.
///
/// Runs on `OnExit(GameState::InGame)`, which fires both on session leave and on
/// every warp (warps cycle `InGame -> Loading -> InGame`). Despawn is recursive in
/// Bevy 0.18, so tagging the hierarchy roots is enough to reap their descendants
/// (model nodes, meshes, etc.). Map sounds keep their own teardown
/// (`teardown_map_sounds`) and are intentionally not `MapScoped`.
#[auto_add_system(
    plugin = crate::plugins::world_domain_plugin::WorldDomainPlugin,
    schedule = OnExit(GameState::InGame)
)]
pub fn despawn_map_scoped(
    mut commands: Commands,
    query: Query<Entity, With<MapScoped>>,
    mut registry: ResMut<EntityRegistry>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    registry.clear_non_local();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn despawns_map_scoped_and_children_on_ingame_exit() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_resource::<EntityRegistry>();
        app.add_systems(OnExit(GameState::InGame), despawn_map_scoped);

        let parent = app.world_mut().spawn(MapScoped).id();
        let child = app.world_mut().spawn(ChildOf(parent)).id();
        let lone = app.world_mut().spawn(MapScoped).id();
        let untagged = app.world_mut().spawn_empty().id();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();

        assert!(app.world().get_entity(parent).is_err());
        assert!(app.world().get_entity(child).is_err());
        assert!(app.world().get_entity(lone).is_err());
        assert!(app.world().get_entity(untagged).is_ok());
    }
}
