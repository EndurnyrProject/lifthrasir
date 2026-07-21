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
