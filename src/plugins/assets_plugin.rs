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
            .init_asset_loader::<BmpLoader>()
            .add_plugins((
                TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
                TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
            ))
            .add_systems(Startup, setup_debug_systems)
            .add_systems(Update, debug_asset_sources);
    }
}

fn setup_debug_systems() {
    info!("Unified asset system initialized");
    info!("Press F1 to display asset loading debug information");
}

fn debug_asset_sources(input: Res<ButtonInput<KeyCode>>, asset_server: Option<Res<AssetServer>>) {
    if input.just_pressed(KeyCode::F1) {
        info!("=== Unified Asset System Debug Info ===");
        info!("Asset server using unified 'ro://' source for RO assets");
        info!("Standard Bevy AssetServer handles all asset loading");

        if asset_server.is_some() {
            info!("AssetServer is ready and operational");
        } else {
            info!("AssetServer not yet available");
        }
    }

    if input.just_pressed(KeyCode::F2) {
        info!("=== Asset Loading Information ===");
        info!("All RO assets are loaded through 'ro://' prefix");
        info!("Example: 'ro://sprite/body/male/01_m.spr'");
        info!("Assets are sourced from GRF files and data folder as configured");
    }
}
