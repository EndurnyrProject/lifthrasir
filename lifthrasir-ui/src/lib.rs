use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;

pub mod screens;
pub mod theme;

pub struct LifthrasirUiPlugin;

impl Plugin for LifthrasirUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera);
        app.add_plugins((
            screens::fade::FadeTransitionPlugin,
            screens::loading::LoadingScreenPlugin,
        ));
    }
}

/// Dedicated 2D camera that hosts all screen-space UI. It renders above the 3D
/// world camera and never clears it, so menus and in-game overlays share the window.
///
/// Note: this camera has no `RenderLayers`, so it renders layer 0. `bevy_extended_ui`'s
/// `render_layers` is pinned to `[0]` in `configure_extended_ui` to match — if this camera
/// ever gains an explicit `RenderLayers`, update that config or the extended_ui screens vanish.
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
        Name::new("UiCamera"),
    ));
}
