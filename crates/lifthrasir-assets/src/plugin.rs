use crate::animation::SpriteProcessingPlugin;
use crate::effects::{AuthoredEffectLoader, LoadedEffectAsset, StrEffectLoader};
use crate::loaders::{
    BgmNameTableAsset, BgmNameTableLoader, RoActAsset, RoActLoader, RoAltitudeAsset,
    RoPaletteAsset, RoPaletteLoader, RoSpriteAsset, RoSpriteLoader, bmp_loader::BmpLoader,
    svg_loader::SvgLoader, tga_loader::TgaLoader,
};
use bevy::prelude::*;

/// Register content loaders after Bevy's AssetPlugin, without client policy or catalogs.
pub struct ContentAssetsPlugin;

impl Plugin for ContentAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RoSpriteAsset>()
            .init_asset_loader::<RoSpriteLoader>()
            .init_asset::<RoActAsset>()
            .init_asset_loader::<RoActLoader>()
            .init_asset::<RoAltitudeAsset>()
            .init_asset::<RoPaletteAsset>()
            .init_asset_loader::<RoPaletteLoader>()
            .init_asset::<LoadedEffectAsset>()
            .init_asset_loader::<StrEffectLoader>()
            .init_asset_loader::<AuthoredEffectLoader>()
            .init_asset::<BgmNameTableAsset>()
            .init_asset_loader::<BgmNameTableLoader>()
            .init_asset_loader::<BmpLoader>()
            .init_asset_loader::<TgaLoader>()
            .init_asset_loader::<SvgLoader>()
            .add_plugins(SpriteProcessingPlugin);
    }
}
