use super::sources::{AssetSource, AssetSourceError, CompositeAssetSource};
use bevy::{
    asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader},
    log::{debug, error},
    tasks::AsyncComputeTaskPool,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

/// AssetReader implementation that integrates with our hierarchical asset source system.
///
/// This reader bridges the existing CompositeAssetSource (which handles priority-based
/// resolution between data folder and GRF files) with Bevy's async AssetReader trait.
///
/// Features:
/// - Preserves priority-based asset resolution (data folder > GRF files)
/// - Converts synchronous GRF operations to async using IoTaskPool
/// - Maintains existing caching behavior
/// - Supports all existing asset formats
pub struct HierarchicalAssetReader {
    composite_source: Arc<RwLock<CompositeAssetSource>>,
}

impl HierarchicalAssetReader {
    /// Create a new HierarchicalAssetReader with the given composite source
    pub fn new(composite_source: Arc<RwLock<CompositeAssetSource>>) -> Self {
        Self { composite_source }
    }

    /// Create from existing CompositeAssetSource (for migration compatibility)
    pub fn from_composite_source(composite_source: Arc<RwLock<CompositeAssetSource>>) -> Self {
        Self::new(composite_source)
    }

    /// Load asset bytes asynchronously using IoTaskPool to avoid blocking
    async fn load_asset_async(&self, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
        let path_str = path.to_string_lossy().to_string();
        let composite_source = self.composite_source.clone();

        // Use AsyncComputeTaskPool to run the synchronous operation without blocking
        AsyncComputeTaskPool::get()
            .spawn(async move {
                match composite_source.read() {
                    Ok(composite) => {
                        debug!("Loading asset: {}", path_str);
                        composite.load(&path_str).map_err(|e| {
                            error!("Failed to load asset '{}': {}", path_str, e);
                            Self::convert_asset_source_error(e)
                        })
                    }
                    Err(e) => {
                        error!(
                            "Failed to acquire read lock for asset '{}': {}",
                            path_str, e
                        );
                        Err(AssetReaderError::Io(Arc::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Lock error: {}", e),
                        ))))
                    }
                }
            })
            .await
    }

    /// Check if asset exists asynchronously
    async fn exists_async(&self, path: &Path) -> Result<bool, AssetReaderError> {
        let path_str = path.to_string_lossy().to_string();
        let composite_source = self.composite_source.clone();

        AsyncComputeTaskPool::get()
            .spawn(async move {
                match composite_source.read() {
                    Ok(composite) => {
                        let exists = composite.exists(&path_str);
                        debug!("Asset '{}' exists: {}", path_str, exists);
                        Ok(exists)
                    }
                    Err(e) => {
                        error!(
                            "Failed to acquire read lock for exists check '{}': {}",
                            path_str, e
                        );
                        Err(AssetReaderError::Io(Arc::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Lock error: {}", e),
                        ))))
                    }
                }
            })
            .await
    }

    /// Convert AssetSourceError to AssetReaderError
    fn convert_asset_source_error(error: AssetSourceError) -> AssetReaderError {
        match error {
            AssetSourceError::NotFound(path) => AssetReaderError::NotFound(PathBuf::from(path)),
            AssetSourceError::Io(io_error) => AssetReaderError::Io(Arc::new(io_error)),
            AssetSourceError::Grf(grf_error) => {
                AssetReaderError::Io(Arc::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("GRF error: {}", grf_error),
                )))
            }
        }
    }
}

impl AssetReader for HierarchicalAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let bytes = self.load_asset_async(path).await?;
        debug!(
            "Successfully read {} bytes for asset: {}",
            bytes.len(),
            path.display()
        );
        Ok(VecReader::new(bytes))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // For now, we don't support .meta files in our hierarchical system
        // This is primarily used for hot reloading and asset metadata
        // We can implement this later if needed for hot reloading support
        debug!(
            "Meta file requested for: {} - not supported in hierarchical reader",
            path.display()
        );
        Err::<VecReader, AssetReaderError>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let path_str = path.to_string_lossy().to_string();
        let composite_source = self.composite_source.clone();

        let file_list = AsyncComputeTaskPool::get()
            .spawn(async move {
                match composite_source.read() {
                    Ok(composite) => {
                        debug!("Reading directory: {}", path_str);
                        let all_files = composite.list_files();

                        // Filter files that start with the directory path
                        let mut seen = std::collections::HashSet::new();
                        let dir_files: Vec<PathBuf> = all_files
                            .into_iter()
                            .filter_map(|file| {
                                if file.starts_with(&path_str) {
                                    // Remove the directory prefix and get the next path component
                                    let relative = file.strip_prefix(&path_str).unwrap_or(&file);
                                    let relative =
                                        relative.trim_start_matches('/').trim_start_matches('\\');

                                    if !relative.is_empty() {
                                        // Get only the immediate child (not nested paths)
                                        let parts: Vec<&str> =
                                            relative.split(&['/', '\\']).collect();
                                        if !parts.is_empty() {
                                            let child = parts[0];
                                            // Deduplicate entries
                                            if seen.insert(child.to_string()) {
                                                // Return full path: directory + separator + child
                                                let separator = if path_str.ends_with('/') || path_str.ends_with('\\') {
                                                    ""
                                                } else {
                                                    "/"
                                                };
                                                let full_path = format!("{}{}{}", path_str, separator, child);
                                                return Some(PathBuf::from(full_path));
                                            }
                                        }
                                    }
                                }
                                None
                            })
                            .collect();

                        debug!("Found {} items in directory: {}", dir_files.len(), path_str);
                        Ok(dir_files)
                    }
                    Err(e) => {
                        error!(
                            "Failed to acquire read lock for directory '{}': {}",
                            path_str, e
                        );
                        Err(AssetReaderError::Io(Arc::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Lock error: {}", e),
                        ))))
                    }
                }
            })
            .await
            .map_err(|e| {
                AssetReaderError::Io(Arc::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Task join error: {}", e),
                )))
            })?;

        // Convert to PathStream - PathStream expects Stream<Item = PathBuf>
        let stream = futures_lite::stream::iter(file_list);

        // Box the stream to match PathStream type
        Ok(Box::new(stream)
            as Box<
                dyn futures_lite::stream::Stream<Item = PathBuf> + Send + Unpin,
            >)
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let path_str = path.to_string_lossy().to_string();
        let composite_source = self.composite_source.clone();

        AsyncComputeTaskPool::get()
            .spawn(async move {
                match composite_source.read() {
                    Ok(composite) => {
                        // Check if any files start with this path (indicating it's a directory)
                        let all_files = composite.list_files();
                        let is_dir = all_files
                            .iter()
                            .any(|file| file.starts_with(&path_str) && file != &path_str);

                        debug!("Path '{}' is directory: {}", path_str, is_dir);
                        Ok(is_dir)
                    }
                    Err(e) => {
                        error!(
                            "Failed to acquire read lock for is_directory '{}': {}",
                            path_str, e
                        );
                        Err(AssetReaderError::Io(Arc::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Lock error: {}", e),
                        ))))
                    }
                }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::sources::DataFolderSource;

    #[tokio::test]
    async fn test_hierarchical_reader_basic_functionality() {
        // Create a test composite source
        let mut composite = CompositeAssetSource::new();

        // Add a data folder source for testing (if it exists)
        if let Ok(current_dir) = std::env::current_dir() {
            let test_data_path = current_dir.join("assets").join("data");
            if test_data_path.exists() {
                let data_source = DataFolderSource::new(test_data_path);
                composite.add_source(Box::new(data_source));
            }
        }

        let composite_arc = Arc::new(RwLock::new(composite));
        let reader = HierarchicalAssetReader::new(composite_arc);

        // Test basic functionality - these might fail if no assets exist, but shouldn't panic
        let test_path = Path::new("nonexistent_file.txt");
        let exists = reader.exists_async(test_path).await.unwrap_or(false);
        assert!(!exists, "Nonexistent file should not exist");

        let is_dir = reader
            .is_directory(Path::new("data"))
            .await
            .unwrap_or(false);
        // This test is environment dependent, so we just ensure it doesn't panic
        println!("'data' directory exists: {}", is_dir);
    }
}
