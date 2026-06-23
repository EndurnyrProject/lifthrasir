use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::ui::IsDefaultUiCamera;
use bevy_ui_text_input::TextInputPlugin;

pub mod cursor;
pub mod focus;
pub mod screens;
pub mod theme;
pub mod widgets;
pub mod worldspace;

pub struct LifthrasirUiPlugin;

impl Plugin for LifthrasirUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera);
        app.add_plugins(TextInputPlugin);
        app.add_plugins((
            cursor::NativeCursorPlugin,
            focus::UiFocusMirrorPlugin,
            screens::fade::FadeTransitionPlugin,
            screens::menu_background::MenuBackgroundPlugin,
            screens::loading::LoadingScreenPlugin,
            screens::login::LoginScreenPlugin,
            screens::server_select::ServerSelectScreenPlugin,
            screens::character_select::CharacterSelectScreenPlugin,
            screens::character_create::CharacterCreateScreenPlugin,
            screens::character_preview::CharacterPreviewPlugin,
            widgets::InGameHudPlugin,
            worldspace::WorldspaceUiPlugin,
        ));
    }
}

/// Dedicated 2D camera that hosts all screen-space UI. It renders above the 3D
/// world camera and never clears it, so menus and in-game overlays share the window.
fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // Must match the 3D world camera's HDR setting: cameras sharing the window
        // target have to agree on HDR, otherwise the 3D pass blows out to white.
        Hdr,
        IsDefaultUiCamera,
        Name::new("UiCamera"),
    ));
}
