pub mod composite;
pub mod data_folder;
pub mod grf_source;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetSourceError {
    #[error("Asset not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("GRF error: {0}")]
    Grf(String),
}

pub trait AssetSource: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u32;
    fn exists(&self, path: &str) -> bool;
    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError>;
    fn list_files(&self) -> Vec<String>;
}

impl std::fmt::Debug for dyn AssetSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AssetSource(name: {}, priority: {})",
            self.name(),
            self.priority()
        )
    }
}

pub use composite::*;
pub use data_folder::*;
pub use grf_source::*;
