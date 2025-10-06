use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpritePngError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid action index: {0}")]
    InvalidAction(usize),

    #[error("Invalid frame index: {0}")]
    InvalidFrame(usize),

    #[error("Invalid sprite index: {0}")]
    InvalidSpriteIndex(usize),

    #[error("No layers in animation frame")]
    NoLayers,

    #[error("Failed to create image")]
    ImageCreationFailed,

    #[error("Invalid palette file")]
    InvalidPalette,

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Asset source error: {0}")]
    AssetSource(#[from] crate::infrastructure::assets::sources::AssetSourceError),

    #[error("Sprite parsing error: {0}")]
    Sprite(#[from] crate::infrastructure::ro_formats::sprite::SpriteError),

    #[error("ACT parsing error: {0}")]
    Act(#[from] crate::infrastructure::ro_formats::act::ActError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
