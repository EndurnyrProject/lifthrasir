use crate::infrastructure::accessory::AccessoryDataAsset;
use crate::infrastructure::assets::{
    bmp_loader::BmpLoader, svg_loader::SvgLoader, tga_loader::TgaLoader, *,
};
use crate::infrastructure::config::ClientConfig;
use crate::infrastructure::effect::{
    AuthoredEffectLoader, EffectDataAsset, LoadedEffectAsset, StrEffectLoader,
};
use crate::infrastructure::item::ItemDataAsset;
use crate::infrastructure::job::JobDataAsset;
use crate::infrastructure::skill::SkillDataAsset;
use crate::infrastructure::status::StatusIconDataAsset;
use crate::infrastructure::weapon::WeaponDataAsset;
use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_common_assets::toml::TomlAssetPlugin;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ClientConfig>()
            .init_asset::<RoSpriteAsset>()
            .init_asset_loader::<RoSpriteLoader>()
            .init_asset::<RoActAsset>()
            .init_asset_loader::<RoActLoader>()
            .init_asset::<RoAnimationAsset>()
            .init_asset::<RoAltitudeAsset>()
            .init_asset::<RoPaletteAsset>()
            .init_asset_loader::<RoPaletteLoader>()
            .init_asset::<LoadedEffectAsset>()
            .init_asset_loader::<StrEffectLoader>()
            .init_asset_loader::<AuthoredEffectLoader>()
            .init_asset::<BgmNameTableAsset>()
            .init_asset_loader::<BgmNameTableLoader>()
            .init_asset::<IndoorMapTableAsset>()
            .init_asset_loader::<IndoorMapTableLoader>()
            .init_asset_loader::<BmpLoader>()
            .init_asset_loader::<TgaLoader>()
            .init_asset_loader::<SvgLoader>()
            .add_plugins((
                TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
                RonAssetPlugin::<JobDataAsset>::new(&["ron"]),
                RonAssetPlugin::<ItemDataAsset>::new(&["ron"]),
                RonAssetPlugin::<SkillDataAsset>::new(&["ron"]),
                RonAssetPlugin::<EffectDataAsset>::new(&["ron"]),
                RonAssetPlugin::<AccessoryDataAsset>::new(&["ron"]),
                RonAssetPlugin::<WeaponDataAsset>::new(&["ron"]),
                RonAssetPlugin::<StatusIconDataAsset>::new(&["ron"]),
                AnimationProcessingPlugin,
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetLoader;

    #[test]
    fn authored_effect_loader_extension_is_strfx_ron() {
        assert_eq!(AuthoredEffectLoader.extensions(), ["strfx.ron"]);
    }
}
