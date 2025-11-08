use bevy_auto_plugin::prelude::*;

/// World Domain Plugin
///
/// Handles world loading, terrain generation, and map state management.
///
/// **Registered resource:**
/// - GameSettings
///
/// **Registered systems:**
/// - `monitor_game_state` (Update)
/// - `on_enter_loading_state` (OnEnter(GameState::Loading))
/// - `setup_unified_map_loading` (Update, conditional: in_state(GameState::Loading))
/// - `extract_map_from_unified_assets` (Update, conditional: in_state(GameState::Loading))
/// - `detect_asset_load_failures` (Update, conditional: in_state(GameState::Loading))
/// - `generate_terrain_mesh` (Update, conditional: in_state(GameState::Loading))
/// - `apply_loaded_terrain_textures` (Update, conditional: in_state(GameState::Loading))
/// - `cleanup_map_loading_state` (OnExit(GameState::Loading), OnExit(GameState::Connecting))
///
/// **System ordering:**
/// ```text
/// OnEnter(GameState::Loading):
///   - on_enter_loading_state
///
/// Update (unconditional):
///   - monitor_game_state
///
/// Update (conditional - in_state(GameState::Loading)):
///   - setup_unified_map_loading
///   - extract_map_from_unified_assets
///   - detect_asset_load_failures
///   - generate_terrain_mesh
///   - apply_loaded_terrain_textures
///
/// OnExit(GameState::Loading):
///   - cleanup_map_loading_state
///
/// OnExit(GameState::Connecting):
///   - cleanup_map_loading_state
/// ```
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct WorldDomainPlugin;
