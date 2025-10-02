use super::{AssetSource, AssetSourceError};
use crate::infrastructure::ro_formats::GrfFile;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct GrfSource {
    name: String,
    grf: Arc<GrfFile>,
    priority: u32,
}

impl GrfSource {
    pub fn new<P: AsRef<Path>>(grf_path: P, priority: u32) -> Result<Self, AssetSourceError> {
        let grf_path = grf_path.as_ref();
        let name = format!("GRF({})", grf_path.display());

        let grf = GrfFile::from_path(grf_path.to_path_buf())
            .map_err(|e| AssetSourceError::Grf(format!("Failed to load GRF file: {}", e)))?;

        Ok(Self {
            name,
            grf: Arc::new(grf),
            priority,
        })
    }

    fn normalize_path(&self, path: &str) -> String {
        // GRF files use backslashes as separators
        path.replace('/', "\\")
    }
}

impl AssetSource for GrfSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn exists(&self, path: &str) -> bool {
        let normalized_path = self.normalize_path(path);
        self.grf.entry_map.contains_key(&normalized_path)
    }

    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        let normalized_path = self.normalize_path(path);

        self.grf
            .get_file(&normalized_path)
            .ok_or_else(|| AssetSourceError::NotFound(path.to_string()))
    }

    fn list_files(&self) -> Vec<String> {
        self.grf
            .entry_map
            .keys()
            .map(|key| key.replace('\\', "/"))
            .collect()
    }
}
