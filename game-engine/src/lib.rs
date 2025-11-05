// Public exports for the game engine
pub mod app;
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod plugins;
pub mod presentation;
pub mod utils;

// Re-export commonly used types
pub use app::{AuthenticationPlugin, LifthrasirPlugin, MapPlugin};
pub use domain::character::{AssetCatalogPlugin, CharacterDomainPlugin};
pub use domain::entities::billboard::BillboardPlugin;
pub use domain::entities::character::UnifiedCharacterEntityPlugin;
pub use domain::entities::hover_plugin::{EntityHoverPlugin, EntityHoverSystems};
pub use domain::entities::movement::MovementPlugin;
pub use domain::entities::spawning::EntitySpawningPlugin;
pub use infrastructure::diagnostics::RoDiagnosticsPlugin;
pub use plugins::{AssetsPlugin, AudioPlugin, InputPlugin, WorldPlugin};
pub use presentation::ui::fps_counter::FpsCounterPlugin;

use bevy::prelude::*;
use bevy_tokio_tasks::TokioTasksPlugin;

/// Initialize and return a configured Bevy App (without running it)
/// This allows the Tauri host to control the app lifecycle
pub fn create_app() -> App {
    let ro_asset_source_plugin =
        crate::infrastructure::assets::ro_assets_plugin::RoAssetsPlugin::with_unified_source();

    let mut app = App::new();

    app
        // CRITICAL: Asset sources must be registered BEFORE AssetPlugin
        .add_plugins(ro_asset_source_plugin)
        .add_plugins((
            TokioTasksPlugin::default(), // ENABLED: Required for async networking
            RoDiagnosticsPlugin,         // ENABLED: Performance diagnostics and profiling
            LifthrasirPlugin,
            AssetsPlugin,                 // ENABLED: Registers ClientConfig asset type
            AudioPlugin,                  // ENABLED: Audio system with BGM support and crossfading
            EntitySpawningPlugin, // ENABLED: Entity spawning/despawning events for network entities (must be before CharacterDomainPlugin)
            CharacterDomainPlugin, // ENABLED: Character events and networking (no UI)
            AuthenticationPlugin, // ENABLED: Reads LoginAttemptEvent and handles auth
            WorldPlugin,          // ENABLED: Map loading and world systems
            BillboardPlugin,      // ENABLED: 3D billboard rendering infrastructure
            MovementPlugin,       // ENABLED: Generic entity movement system
        ))
        .add_plugins((
            EntityHoverPlugin,    // ENABLED: Entity hover detection and name request system
            UnifiedCharacterEntityPlugin, // ENABLED: Unified character system with 3D billboard sprite hierarchy
        ));

    app
}
