use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, io::Reader},
    prelude::*,
    reflect::TypePath,
};
use thiserror::Error;

use super::asset::{
    EffectBlend, LoadedEffectAsset, LoadedFrame, LoadedLayer, build_frame_index_map, decode_blend,
};
use crate::infrastructure::ro_formats::{StrEffect, StrError};

#[derive(Default, TypePath)]
pub struct StrEffectLoader;

#[derive(Debug, Error)]
pub enum StrEffectLoaderError {
    #[error("Could not load STR effect: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse STR effect: {0}")]
    Parse(#[from] StrError),
}

impl AssetLoader for StrEffectLoader {
    type Asset = LoadedEffectAsset;
    type Settings = ();
    type Error = StrEffectLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let effect = StrEffect::from_bytes(&bytes)?;

        // Texture names are siblings of the .str file (i.e. `effect/{prefix}{name}`
        // where `{prefix}` is the .str's own subdirectory). `resolve_embed`
        // resolves each name against the .str's directory, preserving the `ro`
        // asset source.
        let base = load_context.path().clone();

        let layers = effect
            .layers
            .into_iter()
            .map(|layer| {
                let frame_index_map = build_frame_index_map(&layer.frames, effect.max_key as usize);

                // `resolve_embed` is infallible, so every name yields a handle and
                // `textures.len()` stays aligned with the per-frame `texture_index`
                // that indexes into it.
                let textures = layer
                    .texture_names
                    .iter()
                    .map(|name| {
                        let path = base.resolve_embed(&AssetPath::from(name.clone()));
                        load_context.load(path)
                    })
                    .collect();

                let blend = layer
                    .frames
                    .first()
                    .map(|frame| decode_blend(frame.src_blend, frame.dst_blend))
                    .unwrap_or(EffectBlend::Blend);

                let frames = layer
                    .frames
                    .into_iter()
                    .map(|frame| LoadedFrame {
                        frame_index: frame.frame_index.max(0) as usize,
                        offset: Vec2::new(frame.offset.x, frame.offset.y),
                        xy: frame.xy,
                        uv: frame.uv,
                        texture_index: frame.texture_index.max(0.0) as usize,
                        color: frame.color,
                        angle: frame.angle,
                        blend: decode_blend(frame.src_blend, frame.dst_blend),
                    })
                    .collect();

                LoadedLayer {
                    textures,
                    frame_index_map,
                    frames,
                    blend,
                }
            })
            .collect();

        Ok(LoadedEffectAsset {
            fps: effect.fps,
            max_key: effect.max_key,
            layers,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["str"]
    }
}
