mod assets;

use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Subscriber};
use bevy::log::tracing_subscriber::layer::Context;
use bevy::log::tracing_subscriber::registry::LookupSpan;
use bevy::log::tracing_subscriber::Layer;
use bevy::log::{BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};
use bevy_extended_ui::{ExtendedCam, ExtendedUiConfiguration, ExtendedUiPlugin};

const FRAMERATE_LIMIT: f64 = 60.0;

/// bevy_extended_ui 1.6's `propagate_style_inheritance` mangles an inherited `.ttf`
/// `font-family` into a folder path (`fonts/x.ttf/x.ttf-Regular.ttf`) and loads it
/// for every font-less widget sub-entity (input selection/suffix spans), which have
/// no `CssSource` so no stylesheet rule can give them a local font. The load always
/// fails with "Not a directory (os error 20)", spamming the log every frame. The
/// failed font is cosmetic (those spans render no text), so drop only this exact
/// message — every other `bevy_asset` error still prints.
const SUPPRESSED_ASSET_ERROR: &str = "Not a directory (os error 20)";

struct SuppressFontInheritanceSpam;

#[derive(Default)]
struct AssetErrorMessage(bool);

impl Visit for AssetErrorMessage {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" && format!("{value:?}").contains(SUPPRESSED_ASSET_ERROR) {
            self.0 = true;
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SuppressFontInheritanceSpam {
    fn event_enabled(&self, event: &Event<'_>, _ctx: Context<'_, S>) -> bool {
        if event.metadata().target() != "bevy_asset::server" {
            return true;
        }
        let mut matched = AssetErrorMessage::default();
        event.record(&mut matched);
        !matched.0
    }
}

fn main() {
    let composite_source = assets::load_composite_source();

    let mut app = App::new();

    assets::register_ro_asset_source(&mut app, composite_source);

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Lifthrasir".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                custom_layer: |_| Some(Box::new(SuppressFontInheritanceSpam) as BoxedLayer),
                ..default()
            }),
    );

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
