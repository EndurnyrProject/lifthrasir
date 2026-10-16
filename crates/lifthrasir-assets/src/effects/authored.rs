use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
    reflect::TypePath,
};
pub use lifthrasir_data::effects::{
    AuthoredBlend, AuthoredEffect, AuthoredEffectError, AuthoredKey, AuthoredLayer,
};
use thiserror::Error;

use super::asset::{EffectBlend, LoadedEffectAsset, LoadedFrame, LoadedLayer};

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

/// Expand `quad: (w, h)` into the centred `xy[8]` corner array consumed by
/// `corners_from_xy`: `xy[0..4]` are the four corner x's, `xy[4..8]` the y's.
fn quad_to_xy(w: f32, h: f32) -> [f32; 8] {
    let (hw, hh) = (w / 2.0, h / 2.0);
    [-hw, hw, hw, -hw, -hh, -hh, hh, hh]
}

/// Convert a key whose quad/xy exclusivity has already been validated.
fn key_xy(key: &AuthoredKey) -> [f32; 8] {
    match (key.quad, key.xy) {
        (Some((w, h)), None) => quad_to_xy(w, h),
        (None, Some(xy)) => xy,
        _ => unreachable!("authored effect validated before conversion"),
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
    load_texture: &mut impl FnMut(&str) -> Handle<Image>,
) -> LoadedLayer {
    let blend: EffectBlend = layer.blend.into();
    let textures = layer.textures.iter().map(|p| load_texture(p)).collect();

    let mut frames = Vec::with_capacity(layer.keys.len() * 2);
    for key in &layer.keys {
        let frame = LoadedFrame {
            frame_index: key.at as usize,
            offset: Vec2::new(key.offset.0, key.offset.1),
            xy: key_xy(key),
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

    LoadedLayer {
        textures,
        frame_index_map,
        frames,
        blend,
    }
}

/// Pure conversion of an `AuthoredEffect` into a `LoadedEffectAsset`. Validates
/// loudly; textures are resolved via the supplied closure (the loader passes
/// `load_context.load`, tests pass `Handle::default`). Effect-level `max_key` is
/// the max over layers of `last_key.at + 1`.
pub fn convert_authored_effect(
    effect: &AuthoredEffect,
    mut load_texture: impl FnMut(&str) -> Handle<Image>,
) -> Result<LoadedEffectAsset, AuthoredEffectError> {
    effect.validate()?;

    let layers: Vec<_> = effect
        .layers
        .iter()
        .map(|layer| convert_layer(layer, &mut load_texture))
        .collect();

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
