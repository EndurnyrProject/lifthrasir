use bevy::{
    app::{App, Plugin},
    asset::{Asset, AssetApp, AssetLoader, LoadContext, io::Reader},
    log::info,
    prelude::*,
    reflect::TypePath,
};
use bevy_common_assets::toml::TomlAssetPlugin;
use thiserror::Error;

use crate::infrastructure::assets::{
    HierarchicalAssetManager, bmp_loader::BmpLoader, config::AssetConfig,
};
use crate::infrastructure::config::ClientConfig;
use crate::infrastructure::ro_formats::{
    ActError, GatError, GndError, GrfError, GrfFile, RoAction as ParsedRoAction, RoAltitude,
    RoGround, RoSprite as ParsedRoSprite, RoWorld, RsmError, RsmFile, RswError, SpriteError,
    parse_act, parse_spr,
};

pub struct RoAssetsPlugin;

impl Plugin for RoAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            TomlAssetPlugin::<AssetConfig>::new(&["data.toml"]),
            TomlAssetPlugin::<ClientConfig>::new(&["client.toml"]),
        ))
        .init_resource::<HierarchicalAssetManager>()
        .init_asset::<RoSpriteAsset>()
        .init_asset_loader::<RoSpriteLoader>()
        .init_asset::<RoActAsset>()
        .init_asset_loader::<RoActLoader>()
        .init_asset::<RoWorldAsset>()
        .init_asset_loader::<RoWorldLoader>()
        .init_asset::<RoGroundAsset>()
        .init_asset_loader::<RoGroundLoader>()
        .init_asset::<RoAltitudeAsset>()
        .init_asset_loader::<RoAltitudeLoader>()
        .init_asset::<RsmAsset>()
        .init_asset_loader::<RsmLoader>()
        .init_asset::<GrfAsset>()
        .init_asset_loader::<GrfLoader>()
        .init_asset_loader::<BmpLoader>();
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
        use futures_lite::AsyncReadExt;
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
        use futures_lite::AsyncReadExt;
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
        use futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        info!("RSW file loaded, size: {} bytes", bytes.len());
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
        use futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        info!("GND file loaded, size: {} bytes", bytes.len());
        let ground = RoGround::from_bytes(&bytes)?;
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
        use futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let altitude = RoAltitude::from_bytes(&bytes)?;
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
        use futures_lite::AsyncReadExt;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        info!("RSM file loaded, size: {} bytes", bytes.len());
        let model = RsmFile::from_bytes(&bytes)?;
        info!(
            "RSM parsed successfully: version {}, {} nodes, {} textures",
            model.version,
            model.nodes.len(),
            model.textures.len()
        );
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
