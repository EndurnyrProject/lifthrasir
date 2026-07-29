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

        // RO palette files are 1024 bytes (256 colors * 4 bytes RGBA)
        if bytes.len() != 1024 {
            return Err(RoPaletteLoaderError::InvalidFormat);
        }

        let mut colors = Vec::with_capacity(256);
        for chunk in bytes.chunks_exact(4) {
            colors.push([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        Ok(RoPaletteAsset { colors })
    }

    fn extensions(&self) -> &[&str] {
        &["pal"]
    }
}
