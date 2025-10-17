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

    /// Helper to execute an operation with the composite source, handling lock acquisition
    /// and error conversion.
    async fn with_composite_read<F, R>(
        &self,
        context: &str,
        operation: F,
    ) -> Result<R, AssetReaderError>
    where
        F: FnOnce(&CompositeAssetSource) -> Result<R, AssetSourceError> + Send + 'static,
        R: Send + 'static,
    {
        let context = context.to_string();
        let composite_source = self.composite_source.clone();

        AsyncComputeTaskPool::get()
            .spawn(async move {
                match composite_source.read() {
                    Ok(composite) => operation(&composite).map_err(|e| {
                        error!("Failed to {}: {}", context, e);
                        Self::convert_asset_source_error(e)
                    }),
                    Err(e) => {
                        error!("Failed to acquire read lock for {}: {}", context, e);
                        Err(AssetReaderError::Io(Arc::new(std::io::Error::other(
                            format!("Lock error: {}", e),
                        ))))
                    }
                }
            })
            .await
    }

    /// Load asset bytes asynchronously using IoTaskPool to avoid blocking
    async fn load_asset_async(&self, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
        let path_str = path.to_string_lossy().to_string();
        let context = format!("load asset '{}'", path_str);

        self.with_composite_read(&context, move |composite| {
            debug!("Loading asset: {}", path_str);
            composite.load(&path_str)
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

/// Extract the immediate child component from a file path relative to a directory path.
/// Returns None if the file doesn't start with the directory path or has no child component.
fn extract_immediate_child(file_path: &str, dir_path: &str) -> Option<String> {
    if !file_path.starts_with(dir_path) {
        return None;
    }

    let relative = file_path.strip_prefix(dir_path).unwrap_or(file_path);
    let relative = relative.trim_start_matches('/').trim_start_matches('\\');

    if relative.is_empty() {
        return None;
    }

    let parts: Vec<&str> = relative.split(&['/', '\\']).collect();
    if parts.is_empty() {
        return None;
    }

    Some(parts[0].to_string())
}

/// Reconstruct the full path by combining directory path and child name.
/// Handles separator logic to avoid double separators.
fn reconstruct_child_path(dir_path: &str, child_name: &str) -> PathBuf {
    let separator = if dir_path.ends_with('/') || dir_path.ends_with('\\') {
        ""
    } else {
        "/"
    };
    let full_path = format!("{}{}{}", dir_path, separator, child_name);
    PathBuf::from(full_path)
}

/// Filter files to only include immediate children of a directory.
/// This deduplicates entries and only returns the first level of children.
fn filter_immediate_children(all_files: Vec<String>, dir_path: &str) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    all_files
        .into_iter()
        .filter_map(|file| {
            extract_immediate_child(&file, dir_path).and_then(|child| {
                if seen.insert(child.clone()) {
                    Some(reconstruct_child_path(dir_path, &child))
                } else {
                    None
                }
            })
        })
        .collect()
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
        let context = format!("directory '{}'", path_str);

        let file_list = self
            .with_composite_read(&context, move |composite| {
                debug!("Reading directory: {}", path_str);
                let all_files = composite.list_files();
                let dir_files = filter_immediate_children(all_files, &path_str);
                debug!("Found {} items in directory: {}", dir_files.len(), path_str);
                Ok(dir_files)
            })
            .await?;

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
        let context = format!("is_directory '{}'", path_str);

        self.with_composite_read(&context, move |composite| {
            // Check if any files start with this path (indicating it's a directory)
            let all_files = composite.list_files();
            let is_dir = all_files
                .iter()
                .any(|file| file.starts_with(&path_str) && file != &path_str);

            debug!("Path '{}' is directory: {}", path_str, is_dir);
            Ok(is_dir)
        })
        .await
    }
}
