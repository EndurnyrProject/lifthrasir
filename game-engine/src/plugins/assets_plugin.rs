use crate::infrastructure::assets::{bmp_loader::BmpLoader, *};
use crate::infrastructure::config::ClientConfig;
use bevy::prelude::*;
use bevy_common_assets::toml::TomlAssetPlugin;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        // Register all RO asset types and loaders (AssetServer is now available)
        app.init_asset::<ClientConfig>()
            .init_asset::<AssetConfig>()
            .init_asset::<RoSpriteAsset>()
            .init_asset_loader::<RoSpriteLoader>()
            .init_asset::<RoActAsset>()
            .init_asset_loader::<RoActLoader>()
            .init_asset::<RoAnimationAsset>()
            .init_asset::<RoWorldAsset>()
            .init_asset_loader::<RoWorldLoader>()
            .init_asset::<RoGroundAsset>()
            .init_asset_loader::<RoGroundLoader>()
            .init_asset::<RoAltitudeAsset>()
            .init_asset_loader::<RoAltitudeLoader>()
            .init_asset::<RsmAsset>()
            .init_asset_loader::<RsmLoader>()
            .init_asset::<GrfAsset>()
            .init_asset_loader::<GrfLoader>()
            .init_asset::<RoPaletteAsset>()
            .init_asset_loader::<RoPaletteLoader>()
            .init_asset::<BgmNameTableAsset>()
            .init_asset_loader::<BgmNameTableLoader>()
            .init_asset_loader::<BmpLoader>()
            .add_plugins((
                TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
                TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
            ));
    }
}
