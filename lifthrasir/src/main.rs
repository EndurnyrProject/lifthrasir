mod assets;

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};

/// Client version, baked in at build time by `build.rs` (release tag in CI,
/// `git describe` for local builds).
pub const VERSION: &str = env!("LIFTHRASIR_VERSION");

fn main() {
    let composite_source = assets::load_composite_source();

    let mut app = App::new();

    assets::register_ro_asset_source(&mut app, composite_source);

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("Lifthrasir {VERSION}"),
            resolution: WindowResolution::new(1280, 720),
            ..default()
        }),
        ..default()
    }));

    info!("Lifthrasir {VERSION}");

    app.add_plugins(bevy_framepace::FramepacePlugin);

    #[cfg(feature = "dev")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy_brp_extras::BrpExtrasPlugin::default(),
    ));

    app.add_plugins(game_engine::MapPlugin);
    app.add_plugins(game_engine::CoreGamePlugins);

    app.add_plugins(lifthrasir_ui::LifthrasirUiPlugin);

    app.run();
}
