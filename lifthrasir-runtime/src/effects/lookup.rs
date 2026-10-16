use bevy::prelude::*;
use lifthrasir_assets::effects::LoadedEffectAsset;
use lifthrasir_data::effects::EffectDescriptor;

/// The descriptor's RGBA tint as a Bevy `Color` (the data crate stays Bevy-free).
pub fn descriptor_tint(descriptor: &EffectDescriptor) -> Color {
    let [r, g, b, a] = descriptor.color;
    Color::srgba(r, g, b, a)
}

/// Resolves a `Str` layer's asset name to its `ro://` asset path: authored
/// `.strfx.ron` effects live under `effects/`, GRF `.str` effects under the
/// archive's `data/texture/effect/`.
fn effect_asset_path(name: &str) -> String {
    if name.ends_with(".strfx.ron") {
        format!("ro://effects/{name}")
    } else {
        format!("ro://data/texture/effect/{name}")
    }
}

/// Load a named STR effect through the registered `.str`/`.strfx.ron` loader.
pub fn load_str_effect(asset_server: &AssetServer, name: &str) -> Handle<LoadedEffectAsset> {
    asset_server.load(effect_asset_path(name))
}

/// Load the descriptor's STR effect. `None` for descriptors with no `Str`
/// layer, which spawn no STR visual.
pub fn load_effect(
    asset_server: &AssetServer,
    descriptor: &EffectDescriptor,
) -> Option<Handle<LoadedEffectAsset>> {
    descriptor
        .str_name()
        .map(|name| load_str_effect(asset_server, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effect_asset_path_resolves_authored_names_to_the_filesystem_source() {
        assert_eq!(
            effect_asset_path("fire_bolt.strfx.ron"),
            "ro://effects/fire_bolt.strfx.ron"
        );
    }

    #[test]
    fn effect_asset_path_resolves_grf_names_to_the_ro_source() {
        assert_eq!(
            effect_asset_path("heal.str"),
            "ro://data/texture/effect/heal.str"
        );
    }
}
