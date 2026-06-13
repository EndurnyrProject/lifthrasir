mod assets;

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};
use bevy_extended_ui::{ExtendedCam, ExtendedUiConfiguration, ExtendedUiPlugin};

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

    app.add_plugins(ExtendedUiPlugin);
    app.add_systems(Startup, configure_extended_ui);

    #[cfg(debug_assertions)]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy_brp_extras::BrpExtrasPlugin::default(),
    ));

    app.add_plugins(game_engine::MapPlugin);
    app.add_plugins(game_engine::CoreGamePlugins);

    app.add_plugins(lifthrasir_ui::LifthrasirUiPlugin);

    app.run();
}

/// Reconciles bevy_extended_ui with the UI camera owned by `LifthrasirUiPlugin`.
///
/// `ExtendedCam::None` stops the crate from spawning its own UI camera, reusing the
/// existing `Camera2d`/`IsDefaultUiCamera`. The crate spawns its widget tree on
/// `render_layers[0]`; that camera has no `RenderLayers` (so it renders layer 0), so
/// the render layer is pinned to 0 to keep the UI visible on it.
fn configure_extended_ui(mut config: ResMut<ExtendedUiConfiguration>) {
    config.camera = ExtendedCam::None;
    config.render_layers = vec![0];
    config.assets_path = "assets/ui/".into();
}
