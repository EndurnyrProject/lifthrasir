use crate::infrastructure::accessory::AccessoryDataAsset;
use crate::infrastructure::assets::{bmp_loader::BmpLoader, svg_loader::SvgLoader, *};
use crate::infrastructure::config::ClientConfig;
use crate::infrastructure::effect::{LoadedEffectAsset, SkillEffectDataAsset, StrEffectLoader};
use crate::infrastructure::item::ItemDataAsset;
use crate::infrastructure::job::JobDataAsset;
use crate::infrastructure::skill::SkillDataAsset;
use crate::infrastructure::weapon::WeaponDataAsset;
use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_common_assets::toml::TomlAssetPlugin;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
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
            .init_asset::<LoadedEffectAsset>()
            .init_asset_loader::<StrEffectLoader>()
            .init_asset::<BgmNameTableAsset>()
            .init_asset_loader::<BgmNameTableLoader>()
            .init_asset::<IndoorMapTableAsset>()
            .init_asset_loader::<IndoorMapTableLoader>()
            .init_asset_loader::<BmpLoader>()
            .init_asset_loader::<SvgLoader>()
            .add_plugins((
                TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
                TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
                RonAssetPlugin::<JobDataAsset>::new(&["ron"]),
                RonAssetPlugin::<ItemDataAsset>::new(&["ron"]),
                RonAssetPlugin::<SkillDataAsset>::new(&["ron"]),
                RonAssetPlugin::<SkillEffectDataAsset>::new(&["ron"]),
                RonAssetPlugin::<AccessoryDataAsset>::new(&["ron"]),
                RonAssetPlugin::<WeaponDataAsset>::new(&["ron"]),
                AnimationProcessingPlugin,
            ));
    }
}
