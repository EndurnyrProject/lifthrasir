use crate::{FORMAT_VERSION, LIFTHRASIR_PREFIX, MANIFEST_ENTRY, PakError, normalize_path};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct PakManifest {
    format_version: u32,
}

/// Indexed pak entries with synchronized access to the ZIP reader.
#[derive(Debug)]
pub struct PakReader {
    archive: Mutex<ZipArchive<File>>,
    index_by_name: HashMap<String, usize>,
}

impl PakReader {
    /// Open an archive accepting the runtime's minimal manifest schema.
    pub fn open(path: &Path) -> Result<Self, PakError> {
        let file = File::open(path).map_err(|e| {
            PakError::Archive(format!(
                "Failed to open pak '{}': {e} (run `grf-utils pack` to build one)",
                path.display()
            ))
        })?;

        let mut archive = ZipArchive::new(file).map_err(|e| {
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

        let mut index_by_name = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|e| {
                PakError::Archive(format!(
                    "Failed to read pak entry at index {index} in '{}': {e}",
                    path.display()
                ))
            })?;
            let entry_name = normalize_path(entry.name());
            if !entry_name.starts_with(LIFTHRASIR_PREFIX) {
                index_by_name.insert(entry_name, index);
            }
        }

        Ok(Self {
            archive: Mutex::new(archive),
            index_by_name,
        })
    }
}

fn read_manifest(archive: &mut ZipArchive<File>, path: &Path) -> Result<PakManifest, PakError> {
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
    pub fn exists(&self, path: &str) -> bool {
        self.index_by_name.contains_key(&normalize_path(path))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, PakError> {
        let normalized_path = normalize_path(path);
        let index = *self
            .index_by_name
            .get(&normalized_path)
            .ok_or_else(|| PakError::NotFound(path.to_string()))?;

        let mut archive = self.archive.lock().unwrap();
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
        self.index_by_name.keys().cloned().collect()
    }
}
