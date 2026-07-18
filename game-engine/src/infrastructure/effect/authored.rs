use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
    reflect::TypePath,
};
use serde::Deserialize;
use thiserror::Error;

use super::asset::{EffectBlend, LoadedEffectAsset, LoadedFrame, LoadedLayer};

/// Hand-authored effect, deserialized from a `*.strfx.ron` file. Converts into
/// the exact `LoadedEffectAsset` the GRF `.str` loader produces, so authored
/// effects flow through the unchanged STR playback runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthoredEffect {
    pub fps: u32,
    pub layers: Vec<AuthoredLayer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthoredLayer {
    /// Full asset paths (e.g. `ro://data/texture/effect/icearrow.tga`), resolved
    /// via `load_context.load()`. Per-key `texture_index` selects into this list.
    pub textures: Vec<String>,
    pub blend: AuthoredBlend,
    pub keys: Vec<AuthoredKey>,
}

/// Named blend mode, mapped directly onto `EffectBlend` (no D3D ints).
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum AuthoredBlend {
    Add,
    Blend,
    Multiply,
}

/// A single authored keyframe. `at` is the key index (STR frame index). Exactly
/// one of `quad` or `xy` must be set; `offset`, `uv`, `angle` and
/// `texture_index` default.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthoredKey {
    pub at: u32,
    #[serde(default)]
    pub offset: (f32, f32),
    /// `(w, h)` sugar centred on the origin. Mutually exclusive with `xy`.
    #[serde(default)]
    pub quad: Option<(f32, f32)>,
    /// Raw STR corner array escape hatch. Mutually exclusive with `quad`.
    #[serde(default)]
    pub xy: Option<[f32; 8]>,
    /// STR `uv[8]`; defaults to the full quad.
    #[serde(default)]
    pub uv: Option<[f32; 8]>,
    /// RGBA in 0-255 (divided by 255 at interpolation time).
    pub color: (u8, u8, u8, u8),
    /// Raw STR angle units.
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub texture_index: usize,
}

/// Full-quad UVs: origin `(0, 0)`, width/height `1` in `uv[0..4]`; `uvs_from_uv`
/// ignores `uv[4..8]`.
const FULL_QUAD_UV: [f32; 8] = [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];

impl From<AuthoredBlend> for EffectBlend {
    fn from(blend: AuthoredBlend) -> Self {
        match blend {
            AuthoredBlend::Add => EffectBlend::Add,
            AuthoredBlend::Blend => EffectBlend::Blend,
            AuthoredBlend::Multiply => EffectBlend::Multiply,
        }
    }
}

/// Validation failures for an authored effect. Loud, per-layer/per-key.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthoredEffectError {
    #[error("authored effect has no layers")]
    NoLayers,
    #[error("layer {layer} has no textures")]
    NoTextures { layer: usize },
    #[error("layer {layer} has no keys")]
    NoKeys { layer: usize },
    #[error("layer {layer} keys are not strictly ascending by `at` (at key index {key})")]
    UnsortedKeys { layer: usize, key: usize },
    #[error(
        "layer {layer} key {key} texture_index {index} out of range (layer has {len} textures)"
    )]
    TextureIndexOutOfRange {
        layer: usize,
        key: usize,
        index: usize,
        len: usize,
    },
    #[error("layer {layer} key {key} must set exactly one of `quad` or `xy`")]
    QuadXyAmbiguous { layer: usize, key: usize },
}

/// Expand `quad: (w, h)` into the centred `xy[8]` corner array consumed by
/// `corners_from_xy`: `xy[0..4]` are the four corner x's, `xy[4..8]` the y's.
fn quad_to_xy(w: f32, h: f32) -> [f32; 8] {
    let (hw, hh) = (w / 2.0, h / 2.0);
    [-hw, hw, hw, -hw, -hh, -hh, hh, hh]
}

/// Convert an authored key into the raw `xy[8]`, validating quad/xy exclusivity.
fn key_xy(
    key: &AuthoredKey,
    layer: usize,
    key_index: usize,
) -> Result<[f32; 8], AuthoredEffectError> {
    match (key.quad, key.xy) {
        (Some((w, h)), None) => Ok(quad_to_xy(w, h)),
        (None, Some(xy)) => Ok(xy),
        _ => Err(AuthoredEffectError::QuadXyAmbiguous {
            layer,
            key: key_index,
        }),
    }
}

/// Build the dense `max_key`-length frame index map for a layer: each key's even
/// (base) slot `2 * i` is active from its `at` until the next key's `at`; the
/// last key holds its own slot; entries before the first key are `None`.
fn build_authored_map(keys: &[AuthoredKey], layer_max_key: usize) -> Vec<Option<usize>> {
    let mut map = vec![None; layer_max_key];

    for (i, key) in keys.iter().enumerate() {
        let start = key.at as usize;
        let end = keys
            .get(i + 1)
            .map(|next| next.at as usize)
            .unwrap_or(layer_max_key);

        map[start..end].fill(Some(2 * i));
    }

    map
}

/// Convert one authored layer into a `LoadedLayer`. Textures are resolved by the
/// caller-supplied `load_texture` so this stays asset-server-free and testable.
fn convert_layer(
    layer: &AuthoredLayer,
    layer_index: usize,
    load_texture: &mut impl FnMut(&str) -> Handle<Image>,
) -> Result<LoadedLayer, AuthoredEffectError> {
    if layer.textures.is_empty() {
        return Err(AuthoredEffectError::NoTextures { layer: layer_index });
    }
    if layer.keys.is_empty() {
        return Err(AuthoredEffectError::NoKeys { layer: layer_index });
    }

    let blend: EffectBlend = layer.blend.into();
    let textures = layer.textures.iter().map(|p| load_texture(p)).collect();

    let mut frames = Vec::with_capacity(layer.keys.len() * 2);
    let mut previous_at: Option<u32> = None;

    for (key_index, key) in layer.keys.iter().enumerate() {
        if previous_at.is_some_and(|prev| key.at <= prev) {
            return Err(AuthoredEffectError::UnsortedKeys {
                layer: layer_index,
                key: key_index,
            });
        }
        previous_at = Some(key.at);

        if key.texture_index >= layer.textures.len() {
            return Err(AuthoredEffectError::TextureIndexOutOfRange {
                layer: layer_index,
                key: key_index,
                index: key.texture_index,
                len: layer.textures.len(),
            });
        }

        let frame = LoadedFrame {
            frame_index: key.at as usize,
            offset: Vec2::new(key.offset.0, key.offset.1),
            xy: key_xy(key, layer_index, key_index)?,
            uv: key.uv.unwrap_or(FULL_QUAD_UV),
            texture_index: key.texture_index,
            color: [
                key.color.0 as f32,
                key.color.1 as f32,
                key.color.2 as f32,
                key.color.3 as f32,
            ],
            angle: key.angle,
            blend,
        };

        // Paired layout: each key emits its base slot and a duplicate, so key i's
        // even slot `2*i` finds its interpolation target at `frames[2*i + 2]` (the
        // next key's base), matching `interpolate_layer_frame`'s `slot + 2` rule.
        frames.push(frame.clone());
        frames.push(frame);
    }

    let last_key = layer.keys.last().expect("keys validated non-empty above");
    let layer_max_key = last_key.at as usize + 1;
    let frame_index_map = build_authored_map(&layer.keys, layer_max_key);

    Ok(LoadedLayer {
        textures,
        frame_index_map,
        frames,
        blend,
    })
}

/// Pure conversion of an `AuthoredEffect` into a `LoadedEffectAsset`. Validates
/// loudly; textures are resolved via the supplied closure (the loader passes
/// `load_context.load`, tests pass `Handle::default`). Effect-level `max_key` is
/// the max over layers of `last_key.at + 1`.
pub fn convert_authored_effect(
    effect: &AuthoredEffect,
    mut load_texture: impl FnMut(&str) -> Handle<Image>,
) -> Result<LoadedEffectAsset, AuthoredEffectError> {
    if effect.layers.is_empty() {
        return Err(AuthoredEffectError::NoLayers);
    }

    let layers = effect
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| convert_layer(layer, index, &mut load_texture))
        .collect::<Result<Vec<_>, _>>()?;

    // Each converted layer's dense map is `last_key.at + 1` long; the effect's
    // `max_key` is the widest. Both the layer list and each layer's keys are
    // validated non-empty above, so `max` is guaranteed present.
    let max_key = layers
        .iter()
        .map(|layer| layer.frame_index_map.len() as u32)
        .max()
        .expect("layers validated non-empty above");

    Ok(LoadedEffectAsset {
        fps: effect.fps,
        max_key,
        layers,
    })
}

/// RON options with `implicit_some` so authors write `quad: (w, h)` and
/// `offset: (x, y)` bare, not wrapped in `Some(...)` (matches the authoring
/// examples in the asset files).
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

#[derive(Default, TypePath)]
pub struct AuthoredEffectLoader;

#[derive(Debug, Error)]
pub enum AuthoredEffectLoaderError {
    #[error("Could not load authored effect: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse authored effect RON: {0}")]
    Parse(#[from] ron::error::SpannedError),
    #[error(transparent)]
    Convert(#[from] AuthoredEffectError),
}

impl AssetLoader for AuthoredEffectLoader {
    type Asset = LoadedEffectAsset;
    type Settings = ();
    type Error = AuthoredEffectLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let effect: AuthoredEffect = ron_options().from_bytes(&bytes)?;
        let asset = convert_authored_effect(&effect, |path| load_context.load(path.to_string()))?;
        Ok(asset)
    }

    fn extensions(&self) -> &[&str] {
        &["strfx.ron"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::systems::interpolate_layer_frame;

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
