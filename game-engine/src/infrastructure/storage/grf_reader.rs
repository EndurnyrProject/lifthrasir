use crate::infrastructure::ro_formats::GrfFile;
use std::path::{Path, PathBuf};

pub struct GrfReader {
    grf: Option<GrfFile>,
    file_path: String,
}

impl GrfReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file_path = path.as_ref().to_string_lossy().to_string();
        Ok(Self {
            grf: None,
            file_path,
        })
    }

    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(grf) = GrfFile::from_path(PathBuf::from(&self.file_path)) {
            self.grf = Some(grf);
            Ok(())
        } else {
            Err("Failed to load GRF file".into())
        }
    }

    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        self.grf.as_ref().and_then(|grf| grf.get_file(path))
    }

    pub fn is_loaded(&self) -> bool {
        self.grf.is_some()
    }
}
