use crate::{
    core::{GameSettings, GameState, MapState},
    domain::world::{
        spawn_context::MapSpawnContext,
        systems::{
            cleanup_map_loading_state, detect_asset_load_failures,
            extract_map_from_unified_assets, monitor_game_state, on_enter_loading_state,
            setup_unified_map_loading,
        },
        terrain::{generate_terrain_mesh, generate_terrain_when_textures_ready, setup_terrain_camera},
    },
};
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MapState>()
            .init_resource::<GameSettings>()
            .register_type::<MapSpawnContext>()
            .add_systems(Update, monitor_game_state)
            .add_systems(
                OnEnter(GameState::Loading),
                on_enter_loading_state,
            )
            .add_systems(
                Update,
                (
                    setup_unified_map_loading,
                    extract_map_from_unified_assets,
                    detect_asset_load_failures,
                    generate_terrain_mesh,
                    generate_terrain_when_textures_ready,
                )
                    .run_if(in_state(GameState::Loading)),
            )
            .add_systems(
                OnEnter(GameState::InGame),
                setup_terrain_camera,
            )
            .add_systems(
                OnExit(GameState::Loading),
                cleanup_map_loading_state,
            )
            .add_systems(
                OnExit(GameState::Connecting),
                cleanup_map_loading_state,
            );
    }
}
