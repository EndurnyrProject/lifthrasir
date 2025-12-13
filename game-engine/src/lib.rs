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
pub use domain::camera::CameraPlugin;
pub use domain::character::{AssetCatalogPlugin, CharacterDomainPlugin};
pub use domain::entities::billboard::BillboardPlugin;
pub use domain::entities::character::UnifiedCharacterEntityPlugin;
pub use domain::entities::hover_plugin::EntityHoverPlugin;
pub use domain::entities::movement::MovementPlugin;
pub use domain::entities::spawning::EntitySpawningPlugin;
pub use infrastructure::diagnostics::RoDiagnosticsPlugin;
pub use infrastructure::lua_scripts::job::JobSystemPlugin;
pub use plugins::{AssetsPlugin, AudioPlugin, InputPlugin, WorldPlugin};
pub use presentation::ui::fps_counter::FpsCounterPlugin;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_tokio_tasks::TokioTasksPlugin;

pub struct CoreGamePlugins;

impl PluginGroup for CoreGamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(TokioTasksPlugin::default())
            .add(RoDiagnosticsPlugin)
            .add(LifthrasirPlugin)
            .add(CameraPlugin)
            .add(AssetsPlugin)
            .add(JobSystemPlugin)
            .add(AudioPlugin)
            .add(AssetCatalogPlugin)
            .add(EntitySpawningPlugin)
            .add(CharacterDomainPlugin)
            .add(AuthenticationPlugin)
            .add(WorldPlugin)
            .add(BillboardPlugin)
            .add(MovementPlugin)
            .add(EntityHoverPlugin)
            .add(InputPlugin)
            .add(FpsCounterPlugin)
    }
}
