#[cfg(test)]
use bevy::prelude::*;
#[cfg(test)]
use lifthrasir_assets::effects::asset::{EffectBlend, LoadedEffectAsset};

#[cfg(test)]
/// Full-quad UVs: origin `(0, 0)`, width/height `1` in `uv[0..4]`; `uvs_from_uv`
/// ignores `uv[4..8]`.
const FULL_QUAD_UV: [f32; 8] = [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];

#[cfg(test)]
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_assets::effects::authored::*;
    use lifthrasir_runtime::effects::playback::interpolate_layer_frame;

    fn key(at: u32) -> AuthoredKey {
        AuthoredKey {
            at,
            offset: (0.0, 0.0),
            quad: Some((64.0, 128.0)),
            xy: None,
            uv: None,
            color: (255, 255, 255, 255),
            angle: 0.0,
            texture_index: 0,
        }
    }

    fn layer(keys: Vec<AuthoredKey>) -> AuthoredLayer {
        AuthoredLayer {
            textures: vec!["ro://data/texture/effect/icearrow.tga".to_string()],
            blend: AuthoredBlend::Add,
            keys,
        }
    }

    fn convert(effect: &AuthoredEffect) -> Result<LoadedEffectAsset, AuthoredEffectError> {
        convert_authored_effect(effect, |_| Handle::default())
    }

    #[test]
    fn keys_expand_to_paired_frames() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(0), key(12)])],
        };
        let asset = convert(&effect).expect("convert");
        let frames = &asset.layers[0].frames;

        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].frame_index, 0);
        assert_eq!(frames[1].frame_index, 0);
        assert_eq!(frames[2].frame_index, 12);
        assert_eq!(frames[3].frame_index, 12);
    }

    #[test]
    fn dense_map_none_before_first_held_between_last_at_own_slot() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(2), key(4)])],
        };
        let asset = convert(&effect).expect("convert");
        let map = &asset.layers[0].frame_index_map;

        assert_eq!(map.len(), 5);
        assert_eq!(map[0], None);
        assert_eq!(map[1], None);
        assert_eq!(map[2], Some(0));
        assert_eq!(map[3], Some(0));
        assert_eq!(map[4], Some(2));
    }

    #[test]
    fn quad_expands_to_centred_xy() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(0)])],
        };
        let asset = convert(&effect).expect("convert");
        assert_eq!(
            asset.layers[0].frames[0].xy,
            [-32.0, 32.0, 32.0, -32.0, -64.0, -64.0, 64.0, 64.0]
        );
    }

    #[test]
    fn raw_xy_passes_through() {
        let mut k = key(0);
        k.quad = None;
        k.xy = Some([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![k])],
        };
        let asset = convert(&effect).expect("convert");
        assert_eq!(
            asset.layers[0].frames[0].xy,
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
        );
    }

    #[test]
    fn uv_defaults_to_full_quad() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(0)])],
        };
        let asset = convert(&effect).expect("convert");
        assert_eq!(asset.layers[0].frames[0].uv, FULL_QUAD_UV);
    }

    #[test]
    fn blend_maps_directly() {
        assert_eq!(EffectBlend::from(AuthoredBlend::Add), EffectBlend::Add);
        assert_eq!(EffectBlend::from(AuthoredBlend::Blend), EffectBlend::Blend);
        assert_eq!(
            EffectBlend::from(AuthoredBlend::Multiply),
            EffectBlend::Multiply
        );

        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![AuthoredLayer {
                textures: vec!["a".to_string()],
                blend: AuthoredBlend::Multiply,
                keys: vec![key(0)],
            }],
        };
        let asset = convert(&effect).expect("convert");
        assert_eq!(asset.layers[0].blend, EffectBlend::Multiply);
        assert_eq!(asset.layers[0].frames[0].blend, EffectBlend::Multiply);
    }

    #[test]
    fn max_key_is_last_key_plus_one() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(0), key(12)])],
        };
        assert_eq!(convert(&effect).expect("convert").max_key, 13);
    }

    #[test]
    fn max_key_spans_all_layers() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(0), key(5)]), layer(vec![key(0), key(20)])],
        };
        assert_eq!(convert(&effect).expect("convert").max_key, 21);
    }

    #[test]
    fn unsorted_keys_rejected() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(5), key(2)])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::UnsortedKeys { layer: 0, key: 1 }
        );
    }

    #[test]
    fn equal_keys_rejected() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![key(2), key(2)])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::UnsortedKeys { layer: 0, key: 1 }
        );
    }

    #[test]
    fn empty_layers_rejected() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![],
        };
        assert_eq!(convert(&effect).unwrap_err(), AuthoredEffectError::NoLayers);
    }

    #[test]
    fn empty_textures_rejected() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![AuthoredLayer {
                textures: vec![],
                blend: AuthoredBlend::Add,
                keys: vec![key(0)],
            }],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::NoTextures { layer: 0 }
        );
    }

    #[test]
    fn empty_keys_rejected() {
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::NoKeys { layer: 0 }
        );
    }

    #[test]
    fn out_of_range_texture_index_rejected() {
        let mut k = key(0);
        k.texture_index = 3;
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![k])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::TextureIndexOutOfRange {
                layer: 0,
                key: 0,
                index: 3,
                len: 1,
            }
        );
    }

    #[test]
    fn quad_and_xy_both_set_rejected() {
        let mut k = key(0);
        k.xy = Some([0.0; 8]);
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![k])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::QuadXyAmbiguous { layer: 0, key: 0 }
        );
    }

    #[test]
    fn neither_quad_nor_xy_rejected() {
        let mut k = key(0);
        k.quad = None;
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![k])],
        };
        assert_eq!(
            convert(&effect).unwrap_err(),
            AuthoredEffectError::QuadXyAmbiguous { layer: 0, key: 0 }
        );
    }

    #[test]
    fn deserializes_from_ron() {
        let ron = r#"(
            fps: 30,
            layers: [
                (
                    textures: ["ro://data/texture/effect/icearrow.tga"],
                    blend: Add,
                    keys: [
                        ( at: 0,  offset: (0.0, -320.0), quad: (64.0, 128.0), color: (255, 255, 255, 0),   angle: 15.0 ),
                        ( at: 12, offset: (0.0, 0.0),    quad: (64.0, 128.0), color: (255, 255, 255, 255), angle: 15.0 ),
                    ],
                ),
            ],
        )"#;
        let effect: AuthoredEffect = ron_options().from_str(ron).expect("deserialize");
        convert(&effect).expect("convert");
    }

    /// Deserialize an authored effect the way the loader does (implicit_some) and
    /// convert it, asserting both succeed. Textures resolve to default handles.
    fn parse_and_convert(ron: &str) -> LoadedEffectAsset {
        let effect: AuthoredEffect = ron_options().from_str(ron).expect("deserialize");
        convert(&effect).expect("convert")
    }

    #[test]
    fn fire_bolt_asset_parses_and_converts() {
        let ron = include_str!("../../../../../assets/data/effects/fire_bolt.strfx.ron");
        let asset = parse_and_convert(ron);
        assert_eq!(asset.fps, 30);
        assert!(!asset.layers.is_empty());
    }

    #[test]
    fn cold_bolt_asset_parses_and_converts() {
        let ron = include_str!("../../../../../assets/data/effects/cold_bolt.strfx.ron");
        let asset = parse_and_convert(ron);
        assert_eq!(asset.fps, 30);
        assert!(!asset.layers.is_empty());
    }

    #[test]
    fn lightning_bolt_asset_parses_and_converts() {
        let ron = include_str!("../../../../../assets/data/effects/lightning_bolt.strfx.ron");
        let asset = parse_and_convert(ron);
        assert_eq!(asset.fps, 30);
        assert!(!asset.layers.is_empty());
    }

    #[test]
    fn heavens_drive_asset_parses_and_converts() {
        let ron = include_str!("../../../../../assets/data/effects/heavens_drive.strfx.ron");
        let asset = parse_and_convert(ron);
        assert_eq!(asset.fps, 30);
        assert!(!asset.layers.is_empty());
    }

    #[test]
    fn heal_asset_parses_and_converts() {
        let ron = include_str!("../../../../../assets/data/effects/heal.strfx.ron");
        let asset = parse_and_convert(ron);
        assert_eq!(asset.fps, 30);
        assert!(!asset.layers.is_empty());
    }

    /// Walks every checked-in `*.strfx.ron` asset and runs it through the same
    /// parse -> convert pipeline the per-file tests use, so newly authored
    /// effects are covered without a dedicated test. Fails with the offending
    /// filename on any parse or validation error.
    #[test]
    fn all_authored_effects_parse_and_convert() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/effects");
        let mut found = 0;

        for entry in std::fs::read_dir(&dir).expect("read effects dir") {
            let path = entry.expect("dir entry").path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".strfx.ron") {
                continue;
            }
            found += 1;

            let ron = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{name}: failed to read file: {e}"));
            let effect: AuthoredEffect = ron_options()
                .from_str(&ron)
                .unwrap_or_else(|e| panic!("{name}: failed to parse: {e}"));
            convert(&effect).unwrap_or_else(|e| panic!("{name}: failed to convert: {e}"));
        }

        assert!(
            found >= 5,
            "expected at least 5 checked-in strfx.ron assets, found {found}"
        );
    }

    #[test]
    fn round_trip_lerps_at_midpoint() {
        // Two keys: offset (0,-320)->(0,0), alpha 0->255. At key 6 (midpoint of
        // 0..12) the runtime should lerp to offset (0,-160) and alpha 0.5.
        let k0 = AuthoredKey {
            at: 0,
            offset: (0.0, -320.0),
            quad: Some((64.0, 128.0)),
            xy: None,
            uv: None,
            color: (255, 255, 255, 0),
            angle: 0.0,
            texture_index: 0,
        };
        let k1 = AuthoredKey {
            at: 12,
            offset: (0.0, 0.0),
            quad: Some((64.0, 128.0)),
            xy: None,
            uv: None,
            color: (255, 255, 255, 255),
            angle: 0.0,
            texture_index: 0,
        };
        let effect = AuthoredEffect {
            fps: 30,
            layers: vec![layer(vec![k0, k1])],
        };
        let asset = convert(&effect).expect("convert");

        let render = interpolate_layer_frame(&asset.layers[0], 6).expect("active frame");
        assert_eq!(render.offset, Vec2::new(0.0, -160.0));
        assert!((render.color[3] - 0.5).abs() < 1e-6);
    }
}
