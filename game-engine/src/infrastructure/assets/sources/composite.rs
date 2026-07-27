use super::{AssetSource, AssetSourceError};
use bevy::log::debug;
use std::collections::HashMap;

pub struct CompositeAssetSource {
    name: String,
    sources: Vec<Box<dyn AssetSource>>,
    resolution_cache: HashMap<String, usize>, // path -> source index
}

impl CompositeAssetSource {
    pub fn new() -> Self {
        Self {
            name: "CompositeAssetSource".to_string(),
            sources: Vec::new(),
            resolution_cache: HashMap::new(),
        }
    }

    pub fn add_source(&mut self, source: Box<dyn AssetSource>) {
        debug!(
            "Added asset source: {} (priority: {})",
            source.name(),
            source.priority()
        );
        self.sources.push(source);
        self.sort_sources_by_priority();
        self.resolution_cache.clear(); // Clear cache when sources change
    }

    fn sort_sources_by_priority(&mut self) {
        // Sort by priority (lower number = higher priority)
        self.sources.sort_by_key(|source| source.priority());
    }

    pub fn find_source_for_asset(&self, path: &str) -> Option<usize> {
        // Check cache first
        if let Some(&source_idx) = self.resolution_cache.get(path)
            && source_idx < self.sources.len()
            && self.sources[source_idx].exists(path)
        {
            return Some(source_idx);
        }

        // Search through sources by priority
        for (idx, source) in self.sources.iter().enumerate() {
            if source.exists(path) {
                return Some(idx);
            }
        }

        debug!("Asset '{}' not found in any source", path);
        None
    }

    pub fn get_debug_info(&self) -> String {
        let mut info = format!(
            "CompositeAssetSource with {} sources:\n",
            self.sources.len()
        );
        for (idx, source) in self.sources.iter().enumerate() {
            info.push_str(&format!(
                "  [{}] {} (priority: {})\n",
                idx,
                source.name(),
                source.priority()
            ));
        }
        info.push_str(&format!("Cache entries: {}\n", self.resolution_cache.len()));
        info
    }
}

impl Default for CompositeAssetSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for CompositeAssetSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        0 // Composite source itself doesn't have a priority
    }

    fn exists(&self, path: &str) -> bool {
        self.find_source_for_asset(path).is_some()
    }

    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        if let Some(source_idx) = self.find_source_for_asset(path)
            && let Some(source) = self.sources.get(source_idx)
        {
            return source.load(path);
        }

        Err(AssetSourceError::NotFound(path.to_string()))
    }

    fn list_files(&self) -> Vec<String> {
        let mut all_files = Vec::new();
        for source in &self.sources {
            all_files.extend(source.list_files());
        }

        // Remove duplicates while preserving priority order
        let mut unique_files = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for file in all_files {
            if seen.insert(file.clone()) {
                unique_files.push(file);
            }
        }

        unique_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::sources::{DataFolderSource, PakSource};
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn build_fixture_pak(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);

        writer
            .start_file(
                "data/sprite/x.spr",
                SimpleFileOptions::default().compression_method(CompressionMethod::Zstd),
            )
            .unwrap();
        writer.write_all(b"pak-bytes").unwrap();

        writer
            .start_file(
                ".lifthrasir/manifest.toml",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(b"format_version = 1\ncontent_version = 1\ncreated_unix = 0\n")
            .unwrap();

        writer.finish().unwrap();
        path
    }

    #[test]
    fn data_folder_entry_overrides_pak_entry_for_same_path() {
        let pak_dir = tempfile::tempdir().unwrap();
        let pak_path = build_fixture_pak(pak_dir.path(), "fixture.pak");

        let data_dir = tempfile::tempdir().unwrap();
        let loose_file = data_dir.path().join("data/sprite/x.spr");
        std::fs::create_dir_all(loose_file.parent().unwrap()).unwrap();
        std::fs::write(&loose_file, b"loose-bytes").unwrap();

        let mut composite = CompositeAssetSource::new();
        composite.add_source(Box::new(PakSource::new(&pak_path, 1).unwrap()));
        composite.add_source(Box::new(DataFolderSource::new(data_dir.path())));

        assert_eq!(composite.load("data/sprite/x.spr").unwrap(), b"loose-bytes");
    }

    #[test]
    fn exists_true_for_pak_entry_true_for_data_folder_entry_false_for_missing() {
        let pak_dir = tempfile::tempdir().unwrap();
        let pak_path = build_fixture_pak(pak_dir.path(), "fixture.pak");

        let data_dir = tempfile::tempdir().unwrap();
        let loose_file = data_dir.path().join("ro/config/clientinfo.toml");
        std::fs::create_dir_all(loose_file.parent().unwrap()).unwrap();
        std::fs::write(&loose_file, b"loose-config").unwrap();

        let mut composite = CompositeAssetSource::new();
        composite.add_source(Box::new(PakSource::new(&pak_path, 1).unwrap()));
        composite.add_source(Box::new(DataFolderSource::new(data_dir.path())));

        assert!(composite.exists("data/sprite/x.spr"));
        assert!(composite.exists("ro/config/clientinfo.toml"));
        assert!(!composite.exists("data/does/not/exist.spr"));
    }

    #[test]
    fn exists_agrees_with_load_on_path_normalization() {
        let pak_dir = tempfile::tempdir().unwrap();
        let pak_path = build_fixture_pak(pak_dir.path(), "fixture.pak");

        let mut composite = CompositeAssetSource::new();
        composite.add_source(Box::new(PakSource::new(&pak_path, 1).unwrap()));

        let mixed_case_path = "DATA\\Sprite\\X.SPR";
        assert!(composite.exists(mixed_case_path));
        assert_eq!(composite.load(mixed_case_path).unwrap(), b"pak-bytes");
    }
}
