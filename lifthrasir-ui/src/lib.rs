use bevy::camera::ClearColorConfig;
use bevy::camera::Hdr;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;

pub mod cursor;
pub mod focus;
pub mod rich_text;
pub mod screens;
pub mod theme;
pub mod widgets;
pub mod worldspace;

pub struct LifthrasirUiPlugin;

impl Plugin for LifthrasirUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera);
        app.add_plugins((
            cursor::NativeCursorPlugin,
            focus::UiFocusMirrorPlugin,
            widgets::placeholder::PlaceholderPlugin,
            screens::fade::FadeTransitionPlugin,
            screens::menu_background::MenuBackgroundPlugin,
            screens::loading::LoadingScreenPlugin,
            screens::login::LoginScreenPlugin,
            screens::server_select::ServerSelectScreenPlugin,
            screens::character_select::CharacterSelectScreenPlugin,
            screens::character_create::CharacterCreateScreenPlugin,
            screens::character_preview::CharacterPreviewPlugin,
            widgets::InGameHudPlugin,
        ));
        app.add_plugins((
            widgets::death_dialog::DeathDialogPlugin,
            widgets::info_modal::InfoModalPlugin,
            widgets::system_dialog::SystemDialogPlugin,
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
        // Cameras sharing the window target must agree on HDR, otherwise the 3D
        // pass blows out to white. This is only the pre-settings value: the
        // settings apply system syncs it with the world camera on the first
        // Update and on every settings change.
        Hdr,
        IsDefaultUiCamera,
        Name::new("UiCamera"),
    ));
}
