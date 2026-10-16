use bevy::prelude::*;
use lifthrasir_assets::{ContentAssetsPlugin, effects::LoadedEffectAsset, loaders::*};

#[test]
fn magenta_transparency_keeps_its_tolerance_and_clears_rgb() {
    let mut pixels = [240, 15, 240, 255, 239, 15, 240, 200];
    lifthrasir_assets::loaders::pixels::apply_magenta_transparency(&mut pixels);
    assert_eq!(pixels, [0, 0, 0, 0, 239, 15, 240, 200]);
}

#[test]
fn loader_only_app_initializes_and_updates_without_client_resources() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .add_plugins(ContentAssetsPlugin);
    app.finish();
    app.cleanup();
    app.update();
    assert!(app.world().contains_resource::<Assets<RoSpriteAsset>>());
    assert!(app.world().contains_resource::<Assets<RoActAsset>>());
    assert!(app.world().contains_resource::<Assets<RoPaletteAsset>>());
    assert!(app.world().contains_resource::<Assets<RoAltitudeAsset>>());
    assert!(app.world().contains_resource::<Assets<LoadedEffectAsset>>());
    assert!(app.world().contains_resource::<Assets<BgmNameTableAsset>>());
}
