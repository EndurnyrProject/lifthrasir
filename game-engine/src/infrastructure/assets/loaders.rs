use bevy::{
    asset::{io::Reader, Asset, AssetLoader, LoadContext},
    log::{error, info},
    prelude::*,
    reflect::TypePath,
};
use thiserror::Error;

use crate::infrastructure::ro_formats::{
    parse_act, parse_spr as parse_sprite, ActError, GatError, GndError, GrfError, GrfFile,
    RoAction as ParsedRoAction, RoAltitude, RoGround, RoSprite as ParsedRoSprite, RoWorld,
    RsmError, RsmFile, RswError, SpriteError,
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
pub struct RoWorldAsset {
    pub world: RoWorld,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RoGroundAsset {
    pub ground: RoGround,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RoAltitudeAsset {
    pub altitude: RoAltitude,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RsmAsset {
    pub model: RsmFile,
}

#[derive(Asset, TypePath, Debug)]
pub struct GrfAsset {
    pub grf: GrfFile,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RoPaletteAsset {
    pub colors: Vec<[u8; 4]>, // RGBA
}

#[derive(Default)]
pub struct RoSpriteLoader;

#[derive(Default)]
pub struct RoActLoader;

#[derive(Default)]
pub struct RoWorldLoader;

#[derive(Default)]
pub struct RoGroundLoader;

#[derive(Default)]
pub struct RoAltitudeLoader;

#[derive(Default)]
pub struct RsmLoader;

#[derive(Default)]
pub struct GrfLoader;

#[derive(Default)]
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
pub enum RoWorldLoaderError {
    #[error("Could not load world: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse world: {0}")]
    Parse(#[from] RswError),
}

#[derive(Debug, Error)]
pub enum RoGroundLoaderError {
    #[error("Could not load ground: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse ground: {0}")]
    Parse(#[from] GndError),
}

#[derive(Debug, Error)]
pub enum RoAltitudeLoaderError {
    #[error("Could not load altitude: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse altitude: {0}")]
    Parse(#[from] GatError),
}

#[derive(Debug, Error)]
pub enum RsmLoaderError {
    #[error("Could not load RSM: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse RSM: {0}")]
    Parse(#[from] RsmError),
}

#[derive(Debug, Error)]
pub enum GrfLoaderError {
    #[error("Could not load GRF: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse GRF: {0}")]
    Parse(#[from] GrfError),
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

impl AssetLoader for RoWorldLoader {
    type Asset = RoWorldAsset;
    type Settings = ();
    type Error = RoWorldLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let world = RoWorld::from_bytes(&bytes)?;
        Ok(RoWorldAsset { world })
    }

    fn extensions(&self) -> &[&str] {
        &["rsw"]
    }
}

impl AssetLoader for RoGroundLoader {
    type Asset = RoGroundAsset;
    type Settings = ();
    type Error = RoGroundLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        info!("GND file loaded, size: {} bytes", bytes.len());
        let ground = RoGround::from_bytes(&bytes)?;
        info!("📐 GND Dimensions: width={}, height={}", ground.width, ground.height);
        Ok(RoGroundAsset { ground })
    }

    fn extensions(&self) -> &[&str] {
        &["gnd"]
    }
}

impl AssetLoader for RoAltitudeLoader {
    type Asset = RoAltitudeAsset;
    type Settings = ();
    type Error = RoAltitudeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let altitude = RoAltitude::from_bytes(&bytes)?;
        info!("📐 GAT Dimensions: width={}, height={}", altitude.width, altitude.height);
        Ok(RoAltitudeAsset { altitude })
    }

    fn extensions(&self) -> &[&str] {
        &["gat"]
    }
}

impl AssetLoader for RsmLoader {
    type Asset = RsmAsset;
    type Settings = ();
    type Error = RsmLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let model = RsmFile::from_bytes(&bytes)?;
        Ok(RsmAsset { model })
    }

    fn extensions(&self) -> &[&str] {
        &["rsm"]
    }
}

impl AssetLoader for GrfLoader {
    type Asset = GrfAsset;
    type Settings = ();
    type Error = GrfLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        // Get the file path from the load context
        let asset_path = load_context.asset_path();
        let path = asset_path.path();
        info!("Loading GRF metadata from: {:?}", path);

        // Convert to absolute path by resolving against the assets directory
        let assets_dir = std::env::current_dir().unwrap().join("assets");
        let full_path = assets_dir.join(path);

        // Use the new path-based approach for lazy loading
        let grf = GrfFile::from_path(full_path)?;
        info!(
            "GRF metadata loaded successfully: {} files",
            grf.entries.len()
        );

        Ok(GrfAsset { grf })
    }

    fn extensions(&self) -> &[&str] {
        &["grf"]
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
