use bevy::prelude::*;
use crate::utils::{WINDOW_WIDTH, WINDOW_HEIGHT};

mod ro_formats;
mod assets;
mod components;
mod systems;
mod app;
mod utils;

use app::LifthrasirPlugin;

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
        .add_plugins(LifthrasirPlugin)
        .run();
}
