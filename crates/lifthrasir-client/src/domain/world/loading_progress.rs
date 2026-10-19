//! Map-load progress tracking via `iyes_progress`.
//!
//! The converted glb reports three coarse steps: document parsed, dependencies
//! loaded, and scene data adopted into `MapData`.

use crate::core::state::GameState;
use crate::domain::world::map::MapData;
use bevy::prelude::*;
use iyes_progress::prelude::*;

use crate::presentation::world::SelectedMapScene;
use lifthrasir_runtime::world::{MapSceneHandle, MapSceneStatus};

pub struct MapLoadProgressPlugin;

impl Plugin for MapLoadProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::Loading, GameState::InGame),
        )
        .add_systems(
            Update,
            track_map_load_progress
                .track_progress::<GameState>()
                .after(crate::presentation::world::activate_selected_scene)
                .run_if(in_state(GameState::Loading)),
        );
    }
}

pub(crate) const GLTF_MAP_STEPS: u32 = 3;

pub(crate) fn track_map_load_progress(
    asset_server: Res<AssetServer>,
    selected: Option<Res<SelectedMapScene>>,
    roots: Query<(
        Option<&MapSceneHandle>,
        Option<&MapSceneStatus>,
        Has<MapData>,
    )>,
) -> Progress {
    let done = selected
        .and_then(|selected| roots.get(selected.0).ok())
        .map_or(0, |(handle, status, adopted)| {
            let loaded = handle.is_some_and(|handle| asset_server.is_loaded(&handle.scene));
            let dependencies = handle
                .is_some_and(|handle| asset_server.is_loaded_with_dependencies(&handle.scene));
            u32::from(loaded)
                + u32::from(dependencies)
                + u32::from(adopted && matches!(status, Some(MapSceneStatus::Ready)))
        });
    Progress {
        done,
        total: GLTF_MAP_STEPS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn unrelated_map_data_cannot_complete_the_selected_roots_progress() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        let selected = app.world_mut().spawn(MapSceneStatus::Loading).id();
        app.insert_resource(SelectedMapScene(selected));
        app.world_mut().spawn(MapData {
            name: "unrelated".into(),
            width: 4,
            height: 4,
        });
        let progress = app
            .world_mut()
            .run_system_once(track_map_load_progress)
            .unwrap();
        assert_eq!(progress.done, 0);
        assert_eq!(progress.total, GLTF_MAP_STEPS);
    }
}
