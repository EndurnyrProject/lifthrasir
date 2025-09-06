use crate::utils::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::prelude::*;

mod app;
mod assets;
mod components;
mod ro_formats;
mod systems;
mod utils;

use app::{LifthrasirPlugin, MapPlugin};
use systems::{EnhancedLightingPlugin, extract_map_from_grf, setup_grf_map_loading};

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
        .add_plugins((LifthrasirPlugin, MapPlugin))
        .add_plugins(EnhancedLightingPlugin)
        .add_systems(Startup, setup_grf_map_loading)
        .add_systems(Update, extract_map_from_grf)
        .run();
}
