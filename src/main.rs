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
use plugins::{AssetsPlugin, InputPlugin}; // WorldPlugin, RenderingPlugin disabled for UI development
use presentation::ui::{LoginPlugin, PopupPlugin, ServerSelectionPlugin};

fn main() {
    App::new()
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
            AssetsPlugin,
            LoginPlugin,
            ServerSelectionPlugin,
            PopupPlugin,
            AuthenticationPlugin, // New authentication plugin
        ))
        .run();
}
