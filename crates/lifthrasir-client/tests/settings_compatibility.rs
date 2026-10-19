use bevy::{
    prelude::*,
    settings::{SettingsGroup, SettingsPlugin},
};
use lifthrasir_assets::settings::Upscaling;
use lifthrasir_client::domain::settings::{
    ActionBinds, Anisotropy, AntiAliasing, AudioConfig, DisplayMode, FpsCap, GraphicsSettings,
    KeyBind, Keybinds, Modifier, UiScaling,
    resources::{DlssMode, Ssao},
};

/// An absolute temporary app name confines the real settings store to this fixture directory.
#[test]
fn pre_workspace_settings_load_with_original_groups_and_values() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("settings.toml"),
        include_str!("fixtures/settings/pre_workspace.toml"),
    )
    .unwrap();
    let mut app = App::new();
    app.register_type::<GraphicsSettings>()
        .register_type::<AudioConfig>()
        .register_type::<Keybinds>()
        .add_plugins(SettingsPlugin::new(temp.path().to_str().unwrap()));
    assert_eq!(GraphicsSettings::settings_group_name(), "graphics");
    assert_eq!(AudioConfig::settings_group_name(), "audio");
    assert_eq!(Keybinds::settings_group_name(), "keybinds");
    assert_eq!(
        *app.world().resource::<GraphicsSettings>(),
        GraphicsSettings {
            display_mode: DisplayMode::Windowed,
            resolution: (1280, 720),
            antialiasing: AntiAliasing::MsaaX4,
            anisotropy: Anisotropy::X16,
            upscaling: Upscaling::X3,
            vsync: false,
            fps_cap: FpsCap::F144,
            ui_scaling: UiScaling::P150,
            bloom: false,
            shadows: false,
            dlss: DlssMode::Off,
            ssao: Ssao::High,
        }
    );
    assert_eq!(
        *app.world().resource::<AudioConfig>(),
        AudioConfig {
            bgm_volume: 0.25,
            bgm_muted: true,
            sfx_volume: 0.5,
            sfx_muted: true,
            ambient_volume: 0.125,
            ambient_muted: true,
        }
    );
    assert_eq!(
        app.world().resource::<Keybinds>().party,
        ActionBinds {
            primary: Some(KeyBind::modified(Modifier::Control, "KeyO")),
            secondary: Some(KeyBind::modified(Modifier::Shift, "KeyZ")),
        }
    );
    assert_eq!(
        app.world().resource::<Keybinds>().hotbar,
        Keybinds::default().hotbar
    );
}
