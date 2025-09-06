use bevy::{
    app::{App, Plugin},
    asset::{io::Reader, Asset, AssetApp, AssetLoader, LoadContext},
    prelude::*,
    reflect::TypePath,
};
use thiserror::Error;

use crate::ro_formats::{parse_spr, RoSprite as ParsedRoSprite, SpriteError, parse_act, RoAction as ParsedRoAction, ActError};

pub struct RoAssetsPlugin;

impl Plugin for RoAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RoSpriteAsset>()
            .init_asset_loader::<RoSpriteLoader>()
            .init_asset::<RoActAsset>()
            .init_asset_loader::<RoActLoader>();
    }
}

#[derive(Asset, TypePath, Debug)]
pub struct RoSpriteAsset {
    pub sprite: ParsedRoSprite,
}

#[derive(Asset, TypePath, Debug)]
pub struct RoActAsset {
    pub action: ParsedRoAction,
}

#[derive(Default)]
pub struct RoSpriteLoader;

#[derive(Default)]
pub struct RoActLoader;

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
        use bevy::tasks::futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let sprite = parse_spr(&bytes)?;
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
        use bevy::tasks::futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let action = parse_act(&bytes)?;
        Ok(RoActAsset { action })
    }

    fn extensions(&self) -> &[&str] {
        &["act"]
    }
}