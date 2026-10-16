//! Lifthrasir archive mechanics, independent of game and source formats.

mod merge;
mod reader;
mod writer;

pub use merge::{detect_leftovers, merge_paks};
pub use reader::PakReader;
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use writer::{DedupStats, write_pak_parallel};

/// Normalized paths and their source bytes, in precedence-resolved order.
pub type SourceEntries = Vec<(String, Vec<u8>)>;

/// Full metadata required by writers and merge operations, not by runtime reading.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PakManifest {
    pub format_version: u32,
    pub content_version: u64,
    pub created_unix: u64,
}

pub const MANIFEST_ENTRY: &str = ".lifthrasir/manifest.toml";
pub(crate) const LIFTHRASIR_PREFIX: &str = ".lifthrasir/";
pub const FORMAT_VERSION: u32 = 1;
pub const TOMBSTONE_ENTRY: &str = ".lifthrasir/tombstones";

/// Normalize pak entry names while preserving Unicode filenames.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[derive(Debug, Error)]
pub enum PakError {
    #[error("Asset not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Archive(String),
}
