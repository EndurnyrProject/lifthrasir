mod assets;

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};

const FRAMERATE_LIMIT: f64 = 60.0;

fn main() {
    let composite_source = assets::load_composite_source();

    let mut app = App::new();

    assets::register_ro_asset_source(&mut app, composite_source);

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Lifthrasir".into(),
            resolution: WindowResolution::new(1280, 720),
            ..default()
        }),
        ..default()
    }));

    app.add_plugins(bevy_framepace::FramepacePlugin);
    app.world_mut()
        .resource_mut::<bevy_framepace::FramepaceSettings>()
        .limiter = bevy_framepace::Limiter::from_framerate(FRAMERATE_LIMIT);

    app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
    app.add_plugins(bevy_brp_extras::BrpExtrasPlugin::default());

    app.add_plugins(game_engine::MapPlugin);
    app.add_plugins(game_engine::CoreGamePlugins);

    app.add_plugins(lifthrasir_ui::LifthrasirUiPlugin);

    app.run();
}
