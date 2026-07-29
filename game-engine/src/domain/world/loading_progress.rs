//! Map-load progress tracking via `iyes_progress`.
//!
//! The converted glb reports three coarse steps: document parsed, dependencies
//! loaded, and scene data adopted into `MapData`.

use crate::core::state::GameState;
use crate::domain::world::map::MapData;
use bevy::prelude::*;
use iyes_progress::prelude::*;

use crate::domain::world::gltf_map::GltfMapLoader;
use bevy::world_serialization::WorldAssetRoot;

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
                .run_if(in_state(GameState::Loading)),
        );
    }
}

pub(crate) const GLTF_MAP_STEPS: u32 = 3;

pub(crate) fn track_map_load_progress(
    asset_server: Res<AssetServer>,
    maps: Query<(), With<MapData>>,
    gltf_roots: Query<&WorldAssetRoot, With<GltfMapLoader>>,
) -> Progress {
    if let Some(root) = gltf_roots.iter().next() {
        let scene = root.0.id();
        let done = u32::from(asset_server.is_loaded(scene))
            + u32::from(asset_server.is_loaded_with_dependencies(scene))
            + u32::from(!maps.is_empty());
        return Progress {
            done,
            total: GLTF_MAP_STEPS,
        };
    }

    Progress {
        done: u32::from(!maps.is_empty()),
        total: 1,
    }
}
