use crate::utils::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::prelude::*;

mod app;
mod core;
mod domain;
mod infrastructure;
mod plugins;
mod presentation;
mod utils;

use app::{LifthrasirPlugin, MapPlugin};
use plugins::*;

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
            LifthrasirPlugin,
            MapPlugin,
            WorldPlugin,
            RenderingPlugin,
            InputPlugin,
            AssetsPlugin,
        ))
        .run();
}
