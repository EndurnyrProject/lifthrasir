use bevy::camera::ClearColorConfig;
use bevy::camera::Hdr;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;

pub mod cursor;
pub mod focus;
pub mod screens;
pub mod widgets;
pub mod worldspace;

pub struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            app.is_plugin_added::<lifthrasir_ui::UiPrimitivesPlugin>(),
            "ClientUiPlugin requires UiPrimitivesPlugin first"
        );
        app.add_systems(Startup, spawn_ui_camera);
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
        ));
        app.add_plugins((
            widgets::death_dialog::DeathDialogPlugin,
            widgets::escape_menu::EscapeMenuPlugin,
            widgets::info_modal::InfoModalPlugin,
            widgets::system_dialog::SystemDialogPlugin,
            worldspace::WorldspaceUiPlugin,
        ));
    }
}

#[cfg(test)]
pub(crate) fn install_test_ui_primitives(app: &mut App) {
    if !app.is_plugin_added::<bevy::input::InputPlugin>() {
        app.add_plugins(bevy::input::InputPlugin);
    }
    if !app.world().contains_resource::<Assets<Image>>() {
        app.init_asset::<Image>();
    }
    if !app.world().contains_resource::<Assets<Font>>() {
        app.init_asset::<Font>();
    }
    if !app
        .world()
        .contains_resource::<Assets<bevy::shader::Shader>>()
    {
        app.init_asset::<bevy::shader::Shader>();
    }
    app.insert_resource(lifthrasir_ui::theme::feathers_theme::norse_theme())
        .add_plugins(lifthrasir_ui::UiPrimitivesPlugin);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn client_root_preserves_host_theme_and_keeps_its_hdr_camera() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        install_test_ui_primitives(&mut app);
        let token = lifthrasir_ui::theme::feathers_theme::TOKEN_ACCENT;
        app.world_mut()
            .resource_mut::<bevy_feathers::theme::UiTheme>()
            .0
            .color
            .insert(token.clone(), Color::WHITE);
        app.init_resource::<lifthrasir_client::domain::settings::GraphicsSettings>()
            .init_resource::<lifthrasir_client::domain::settings::AudioConfig>()
            .init_resource::<lifthrasir_client::domain::settings::Keybinds>()
            .add_plugins(ClientUiPlugin);
        assert_eq!(
            app.world()
                .resource::<bevy_feathers::theme::UiTheme>()
                .color(&token),
            Color::WHITE
        );
        app.world_mut().run_system_once(spawn_ui_camera).unwrap();
        let world = app.world_mut();
        let (camera, _) = world
            .query_filtered::<(&Camera, &Hdr), With<IsDefaultUiCamera>>()
            .single(world)
            .unwrap();
        assert_eq!(camera.order, 1);
        assert!(matches!(camera.clear_color, ClearColorConfig::None));
    }
}
