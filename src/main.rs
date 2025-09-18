use crate::utils::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::prelude::*;
use bevy_tokio_tasks::TokioTasksPlugin;

mod app;
mod core;
mod domain;
mod infrastructure;
mod plugins;
mod presentation;
mod utils;

use app::{AuthenticationPlugin, LifthrasirPlugin}; // MapPlugin disabled for UI development
use domain::character::AssetCatalogPlugin;
use domain::entities::character::UnifiedCharacterEntityPlugin;
use plugins::{AssetsPlugin, InputPlugin}; // WorldPlugin, RenderingPlugin disabled for UI development
use presentation::ui::{
    CharacterSelectionPlugin, LoginPlugin, PopupPlugin, ScrollPlugin, ServerSelectionPlugin,
};

fn main() {
    // Create the asset registration plugin separately from systems
    let ro_asset_source_plugin =
        crate::infrastructure::assets::ro_assets_plugin::RoAssetsPlugin::with_unified_source();

    App::new()
        // CRITICAL: Asset sources must be registered BEFORE DefaultPlugins (which contains AssetPlugin)
        .add_plugins(ro_asset_source_plugin)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Lifthrasir - Ragnarok Online Client".into(),
                resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            TokioTasksPlugin::default(), // Add Tokio runtime integration
            LifthrasirPlugin,
            // MapPlugin,              // Disabled for UI development
            // WorldPlugin,            // Disabled for UI development
            // RenderingPlugin,        // Disabled for UI development
            InputPlugin,
            AssetsPlugin, // Contains SpriteAssetCoordinatorPlugin (after AssetServer is available)
            AssetCatalogPlugin, // Builds catalogs of available assets
            UnifiedCharacterEntityPlugin, // Unified character entity system
            ScrollPlugin,             // Scrollable panel support
            LoginPlugin,
            ServerSelectionPlugin,
            CharacterSelectionPlugin,
            PopupPlugin,
            AuthenticationPlugin, // New authentication plugin
        ))
        .run();
}
