use crate::infrastructure::config::ClientConfig;
#[cfg(test)]
use crate::infrastructure::effect::AuthoredEffectLoader;
use bevy::prelude::*;
use bevy_common_assets::toml::TomlAssetPlugin;
use lifthrasir_assets::ContentAssetsPlugin;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ContentAssetsPlugin)
            .init_asset::<ClientConfig>()
            .add_plugins(TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]));
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
