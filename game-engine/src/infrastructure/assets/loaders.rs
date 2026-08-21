use bevy::{
    asset::{Asset, AssetLoader, LoadContext, io::Reader},
    prelude::*,
    reflect::TypePath,
};
use thiserror::Error;

use crate::infrastructure::ro_formats::{
    ActError, RoAction as ParsedRoAction, RoAltitude, RoSprite as ParsedRoSprite, SpriteError,
    parse_act, parse_spr as parse_sprite,
};

// Re-export BGM name table types
pub use super::bgm_name_table_loader::{
    BgmNameTableAsset, BgmNameTableLoader, BgmNameTableLoaderError,
};

#[derive(Asset, TypePath, Debug)]
pub struct RoSpriteAsset {
    pub sprite: ParsedRoSprite,
}

#[derive(Asset, TypePath, Debug)]
pub struct RoActAsset {
    pub action: ParsedRoAction,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RoAltitudeAsset {
    pub altitude: RoAltitude,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RoPaletteAsset {
    pub colors: Vec<[u8; 4]>, // RGBA
}

#[derive(Default, TypePath)]
pub struct RoSpriteLoader;

#[derive(Default, TypePath)]
pub struct RoActLoader;

#[derive(Default, TypePath)]
pub struct RoPaletteLoader;

#[derive(Debug, Error)]
pub enum RoSpriteLoaderError {
    #[error("Could not load sprite: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse sprite: {0}")]
    Parse(#[from] SpriteError),
}

#[derive(Debug, Error)]
pub enum RoActLoaderError {
    #[error("Could not load action: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse action: {0}")]
    Parse(#[from] ActError),
}

#[derive(Debug, Error)]
pub enum RoPaletteLoaderError {
    #[error("Could not load palette: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid palette format")]
    InvalidFormat,
}

fn parse_palette(bytes: &[u8]) -> Result<RoPaletteAsset, RoPaletteLoaderError> {
    if bytes.len() != 1024 {
        return Err(RoPaletteLoaderError::InvalidFormat);
    }

    let colors = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            [
                chunk[0],
                chunk[1],
                chunk[2],
                if index == 0 { 0 } else { 255 },
            ]
        })
        .collect();

    Ok(RoPaletteAsset { colors })
}

impl AssetLoader for RoSpriteLoader {
    type Asset = RoSpriteAsset;
    type Settings = ();
    type Error = RoSpriteLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let sprite = parse_sprite(&bytes)?;
        Ok(RoSpriteAsset { sprite })
    }

    fn extensions(&self) -> &[&str] {
        &["spr"]
    }
}

impl AssetLoader for RoActLoader {
    type Asset = RoActAsset;
    type Settings = ();
    type Error = RoActLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let action = parse_act(&bytes)?;
        Ok(RoActAsset { action })
    }

    fn extensions(&self) -> &[&str] {
        &["act"]
    }
}

impl AssetLoader for RoPaletteLoader {
    type Asset = RoPaletteAsset;
    type Settings = ();
    type Error = RoPaletteLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        parse_palette(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["pal"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_alpha_is_normalized_from_reserved_bytes() {
        let mut bytes = vec![0; 1024];
        bytes[..8].copy_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);
        bytes[1020..].copy_from_slice(&[90, 100, 110, 120]);

        let palette = parse_palette(&bytes).unwrap();

        assert_eq!(palette.colors[0], [10, 20, 30, 0]);
        assert_eq!(palette.colors[1], [50, 60, 70, 255]);
        assert_eq!(palette.colors[255], [90, 100, 110, 255]);
    }
}
