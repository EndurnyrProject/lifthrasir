use crate::infrastructure::config::ClientConfig;
use bevy::prelude::*;
use bevy_common_assets::toml::TomlAssetPlugin;
#[cfg(test)]
use lifthrasir_assets::effects::authored::AuthoredEffectLoader;
/// Client-specific configuration decoding; shared content loaders belong to the host.
pub struct ClientConfigPlugin;

impl Plugin for ClientConfigPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ClientConfig>()
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
