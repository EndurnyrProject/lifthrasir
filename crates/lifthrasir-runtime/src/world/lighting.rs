//! Map-wide lighting helpers shared by every host that loads a `LIF_map` glb.

use bevy::prelude::*;
use lifthrasir_data::lif::LifMap;

/// The [`GlobalAmbientLight`] baked into a map's `LIF_map` extension. The
/// client, the content viewer and the world editor all build it from here so
/// they cannot drift apart.
pub fn map_ambient_light(meta: &LifMap) -> GlobalAmbientLight {
    GlobalAmbientLight {
        color: Color::srgb(
            meta.ambient_color[0],
            meta.ambient_color[1],
            meta.ambient_color[2],
        ),
        brightness: meta.ambient_brightness,
        affects_lightmapped_meshes: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_light_maps_color_and_brightness_verbatim() {
        let meta = LifMap {
            format_version: lifthrasir_data::lif::MAP_FORMAT_VERSION,
            source: None,
            ambient_color: [0.1, 0.2, 0.3],
            no_shade_tint: [1.0; 3],
            indoor: true,
            ambient_brightness: 425.0,
            exposure_ev100: 9.7,
        };

        let light = map_ambient_light(&meta);

        assert_eq!(
            light.color,
            Color::srgb(
                meta.ambient_color[0],
                meta.ambient_color[1],
                meta.ambient_color[2]
            )
        );
        assert_eq!(light.brightness, 425.0);
        assert!(!light.affects_lightmapped_meshes);
    }
}
