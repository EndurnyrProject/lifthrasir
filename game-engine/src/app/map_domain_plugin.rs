use bevy_auto_plugin::prelude::*;

/// Map Domain Plugin
///
/// Handles map model rendering, RSM assets, and water systems.
///
/// **Registered systems:**
/// - `log_loaded_world_data` (Update)
/// - `spawn_map_models` (Update)
/// - `load_rsm_assets` (Update)
/// - `update_model_meshes` (Update, after load_rsm_assets)
/// - `create_model_materials_when_textures_ready` (Update, after load_rsm_assets)
/// - `update_rsm_animations` (Update)
/// - `load_water_system` (Update)
/// - `finalize_water_loading_system` (Update, after load_water_system)
/// - `animate_water_system` (Update, after finalize_water_loading_system)
///
/// **System ordering:**
/// ```text
/// Load Phase (parallel):
///   - log_loaded_world_data
///   - spawn_map_models
///   - load_rsm_assets
///
/// Process Phase (sequential):
///   - update_model_meshes (after load_rsm_assets)
///   - create_model_materials_when_textures_ready (after load_rsm_assets)
///   - update_rsm_animations (independent)
///
/// Water Phase (sequential):
///   - load_water_system
///   - finalize_water_loading_system (after load_water_system)
///   - animate_water_system (after finalize_water_loading_system)
/// ```
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct MapDomainPlugin;
