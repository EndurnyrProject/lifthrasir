use crate::{
    core::{GameSettings, GameState, MapState},
    domain::world::{
        spawn_context::MapSpawnContext,
        systems::{
            cleanup_map_loading_state, detect_asset_load_failures, extract_map_from_unified_assets,
            monitor_game_state, on_enter_loading_state, setup_unified_map_loading,
        },
        terrain::{apply_loaded_terrain_textures, generate_terrain_mesh},
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
            .add_systems(OnEnter(GameState::Loading), on_enter_loading_state)
            .add_systems(
                Update,
                (
                    setup_unified_map_loading,
                    extract_map_from_unified_assets,
                    detect_asset_load_failures,
                    generate_terrain_mesh,
                    apply_loaded_terrain_textures,
                )
                    .run_if(in_state(GameState::Loading)),
            )
            .add_systems(OnExit(GameState::Loading), cleanup_map_loading_state)
            .add_systems(OnExit(GameState::Connecting), cleanup_map_loading_state);
    }
}
