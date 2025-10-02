// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod commands;
mod plugin;

use bevy::prelude::*;
use plugin::TauriIntegrationPlugin;

fn main() {
    let mut app = App::new();

    // CRITICAL: TauriIntegrationPlugin must be added FIRST
    // It sets up all core Bevy plugins including StatesPlugin
    // before game engine plugins that use states are added
    app.add_plugins(TauriIntegrationPlugin);

    // Run the app - TauriIntegrationPlugin provides custom runner
    app.run();
}
