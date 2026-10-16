//! Shared content inputs, paths, and asset preparation.

pub mod animation;
pub mod catalogs;
pub use catalogs::ContentCatalogPlugin;
pub mod effects;
pub mod loaders;
pub mod paths;
mod plugin;
pub use plugin::ContentAssetsPlugin;
pub mod settings;
pub mod source;
pub mod sprite;

pub use settings::{SpriteProcessingSettings, Upscaling};
pub use sprite::Gender;
