use crate::{FORMAT_VERSION, LIFTHRASIR_PREFIX, MANIFEST_ENTRY, PakError, normalize_path};
use memmap2::Mmap;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct PakManifest {
    format_version: u32,
}

/// Shared read-only mapping of the pak file, so a cloned archive costs one `Arc` bump.
#[derive(Debug, Clone)]
struct PakBytes(Arc<Mmap>);

impl AsRef<[u8]> for PakBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Memory-mapped pak. Reads clone the archive handle (shared central directory,
/// private cursor) so they run in parallel without a lock. Lookups normalize the
/// request and use the zip crate's own name index; `unnormalized` only covers
/// entries whose stored name differs from its normalized form, which is empty for
/// paks written by `pack`.
#[derive(Debug)]
pub struct PakReader {
    archive: ZipArchive<Cursor<PakBytes>>,
    unnormalized: HashMap<String, usize>,
}

impl PakReader {
    /// Open an archive accepting the runtime's minimal manifest schema.
    ///
    /// The file is memory-mapped, so it must not be modified in place while open.
    /// `pack` and `merge` replace the pak by rename, which leaves an open mapping intact.
    pub fn open(path: &Path) -> Result<Self, PakError> {
        let file = File::open(path).map_err(|e| {
            PakError::Archive(format!(
                "Failed to open pak '{}': {e} (run `grf-utils pack` to build one)",
                path.display()
            ))
        })?;

        // SAFETY: the mapping is read-only and the pak is only ever replaced by rename,
        // never truncated or rewritten in place while a reader holds it.
        let mmap = unsafe { Mmap::map(&file) }.map_err(|e| {
            PakError::Archive(format!("Failed to map pak '{}': {e}", path.display()))
        })?;

        let mut archive = ZipArchive::new(Cursor::new(PakBytes(Arc::new(mmap)))).map_err(|e| {
            PakError::Archive(format!(
                "Failed to read pak zip structure '{}': {e} (run `grf-utils pack` to build one)",
                path.display()
            ))
        })?;

        let manifest = read_manifest(&mut archive, path)?;
        if manifest.format_version != FORMAT_VERSION {
            return Err(PakError::Archive(format!(
                "Pak '{}' has format_version {} (expected {}); rebuild it with `grf-utils pack`",
                path.display(),
                manifest.format_version,
                FORMAT_VERSION
            )));
        }

        let mut unnormalized = HashMap::new();
        for (index, name) in archive.file_names().enumerate() {
            let name = name.map_err(|e| {
                PakError::Archive(format!(
                    "Failed to decode entry name at index {index} in '{}': {e}",
                    path.display()
                ))
            })?;
            let normalized = normalize_path(&name);
            if normalized != *name {
                unnormalized.insert(normalized, index);
            }
        }

        Ok(Self {
            archive,
            unnormalized,
        })
    }
}

fn read_manifest(
    archive: &mut ZipArchive<Cursor<PakBytes>>,
    path: &Path,
) -> Result<PakManifest, PakError> {
    let mut entry = archive.by_name(MANIFEST_ENTRY).map_err(|e| {
        PakError::Archive(format!(
            "Pak '{}' is missing {MANIFEST_ENTRY} ({e}); rebuild it with `grf-utils pack`",
            path.display()
        ))
    })?;

    let mut contents = String::new();
    entry.read_to_string(&mut contents).map_err(|e| {
        PakError::Archive(format!(
            "Failed to read manifest in pak '{}': {e}",
            path.display()
        ))
    })?;

    toml::from_str(&contents).map_err(|e| {
        PakError::Archive(format!(
            "Failed to parse manifest in pak '{}': {e}",
            path.display()
        ))
    })
}

impl PakReader {
    fn index_of(&self, path: &str) -> Option<usize> {
        let normalized = normalize_path(path);
        if normalized.starts_with(LIFTHRASIR_PREFIX) {
            return None;
        }
        self.unnormalized
            .get(&normalized)
            .copied()
            .or_else(|| self.archive.index_for_name(&normalized))
    }

    pub fn exists(&self, path: &str) -> bool {
        self.index_of(path).is_some()
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, PakError> {
        let index = self
            .index_of(path)
            .ok_or_else(|| PakError::NotFound(path.to_string()))?;

        let mut archive = self.archive.clone();
        let mut entry = archive
            .by_index(index)
            .map_err(|e| PakError::Archive(format!("Failed to read pak entry '{path}': {e}")))?;

        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data).map_err(|e| {
            PakError::Archive(format!("Failed to decompress pak entry '{path}': {e}"))
        })?;

        Ok(data)
    }

    pub fn list_files(&self) -> Vec<String> {
        self.archive
            .file_names()
            .flatten()
            .map(|name| normalize_path(&name))
            .filter(|name| !name.starts_with(LIFTHRASIR_PREFIX))
            .collect()
    }
}
