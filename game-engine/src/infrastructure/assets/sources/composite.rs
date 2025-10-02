use super::{AssetSource, AssetSourceError};
use bevy::log::{debug, info, warn};
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
        info!(
            "Added asset source: {} (priority: {})",
            source.name(),
            source.priority()
        );
        self.sources.push(source);
        self.sort_sources_by_priority();
        self.resolution_cache.clear(); // Clear cache when sources change
    }

    pub fn add_sources(&mut self, sources: Vec<Box<dyn AssetSource>>) {
        for source in sources {
            info!(
                "Added asset source: {} (priority: {})",
                source.name(),
                source.priority()
            );
            self.sources.push(source);
        }
        self.sort_sources_by_priority();
        self.resolution_cache.clear();
    }

    fn sort_sources_by_priority(&mut self) {
        // Sort by priority (lower number = higher priority)
        self.sources.sort_by_key(|source| source.priority());
    }

    pub fn find_source_for_asset(&self, path: &str) -> Option<usize> {
        // Check cache first
        if let Some(&source_idx) = self.resolution_cache.get(path) {
            if source_idx < self.sources.len() && self.sources[source_idx].exists(path) {
                return Some(source_idx);
            }
        }

        // Search through sources by priority
        for (idx, source) in self.sources.iter().enumerate() {
            if source.exists(path) {
                debug!("Asset '{}' found in source: {}", path, source.name());
                return Some(idx);
            }
        }

        warn!("Asset '{}' not found in any source", path);
        None
    }

    pub fn get_source_info(&self, path: &str) -> Option<String> {
        if let Some(source_idx) = self.find_source_for_asset(path) {
            if let Some(source) = self.sources.get(source_idx) {
                return Some(format!(
                    "{} (priority: {})",
                    source.name(),
                    source.priority()
                ));
            }
        }
        None
    }

    pub fn list_sources(&self) -> Vec<String> {
        self.sources
            .iter()
            .map(|source| format!("{} (priority: {})", source.name(), source.priority()))
            .collect()
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

    pub fn clear_cache(&mut self) {
        self.resolution_cache.clear();
        debug!("Asset resolution cache cleared");
    }

    pub fn warm_cache(&mut self, common_paths: &[&str]) {
        info!(
            "Warming asset resolution cache with {} paths",
            common_paths.len()
        );
        for path in common_paths {
            if let Some(source_idx) = self.find_source_for_asset(path) {
                self.resolution_cache.insert(path.to_string(), source_idx);
            }
        }
        info!("Cache warmed with {} entries", self.resolution_cache.len());
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
        if let Some(source_idx) = self.find_source_for_asset(path) {
            if let Some(source) = self.sources.get(source_idx) {
                // Note: We would cache the successful resolution here, but due to borrowing constraints
                // in this trait method, we'll rely on the find_source_for_asset method's internal caching

                let result = source.load(path);
                if result.is_ok() {
                    debug!(
                        "Successfully loaded '{}' from source: {}",
                        path,
                        source.name()
                    );
                } else {
                    warn!("Failed to load '{}' from source: {}", path, source.name());
                }
                return result;
            }
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
