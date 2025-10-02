// Public exports for the game engine
pub mod app;
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod plugins;
pub mod presentation;
pub mod utils;

// Re-export commonly used types
pub use app::{AuthenticationPlugin, LifthrasirPlugin};
pub use domain::character::{AssetCatalogPlugin, CharacterDomainPlugin};
pub use domain::entities::character::UnifiedCharacterEntityPlugin;
pub use plugins::{AssetsPlugin, InputPlugin};

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
            LifthrasirPlugin,
            AssetsPlugin,          // ENABLED: Registers ClientConfig asset type
            CharacterDomainPlugin, // ENABLED: Character events and networking (no UI)
            AuthenticationPlugin,  // ENABLED: Reads LoginAttemptEvent and handles auth
        ));

    app
}
