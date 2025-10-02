use super::{AssetSource, AssetSourceError};
use std::fs;
use std::path::{Path, PathBuf};

pub struct DataFolderSource {
    name: String,
    root_path: PathBuf,
    priority: u32,
}

impl DataFolderSource {
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        let root_path = root_path.as_ref().to_path_buf();
        let name = format!("DataFolder({})", root_path.display());

        Self {
            name,
            root_path,
            priority: 0, // Data folder always has highest priority
        }
    }

    fn get_full_path(&self, path: &str) -> PathBuf {
        // Normalize the path - remove leading slashes and backslashes
        let normalized_path = path.trim_start_matches('/').trim_start_matches('\\');
        self.root_path.join(normalized_path)
    }

    fn scan_directory_recursive(&self, dir: &Path, relative_to: &Path) -> Vec<String> {
        let mut files = Vec::new();

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() {
                    if let Ok(relative_path) = path.strip_prefix(relative_to) {
                        if let Some(path_str) = relative_path.to_str() {
                            files.push(path_str.replace('\\', "/"));
                        }
                    }
                } else if path.is_dir() {
                    files.extend(self.scan_directory_recursive(&path, relative_to));
                }
            }
        }

        files
    }
}

impl AssetSource for DataFolderSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn exists(&self, path: &str) -> bool {
        let full_path = self.get_full_path(path);
        full_path.exists() && full_path.is_file()
    }

    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        let full_path = self.get_full_path(path);

        if !full_path.exists() {
            return Err(AssetSourceError::NotFound(path.to_string()));
        }

        fs::read(full_path).map_err(AssetSourceError::Io)
    }

    fn list_files(&self) -> Vec<String> {
        if !self.root_path.exists() || !self.root_path.is_dir() {
            return Vec::new();
        }

        self.scan_directory_recursive(&self.root_path, &self.root_path)
    }
}
