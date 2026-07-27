use super::{AssetSource, AssetSourceError};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use zip::ZipArchive;

const MANIFEST_ENTRY: &str = ".lifthrasir/manifest.toml";
const LIFTHRASIR_PREFIX: &str = ".lifthrasir/";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct PakManifest {
    format_version: u32,
}

pub struct PakSource {
    name: String,
    priority: u32,
    archive: Mutex<ZipArchive<File>>,
    index_by_name: HashMap<String, usize>,
}

impl std::fmt::Debug for PakSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PakSource(name: {}, priority: {}, entries: {})",
            self.name,
            self.priority,
            self.index_by_name.len()
        )
    }
}

impl PakSource {
    pub fn new<P: AsRef<Path>>(path: P, priority: u32) -> Result<Self, AssetSourceError> {
        let path = path.as_ref();
        let name = format!("Pak({})", path.display());

        let file = File::open(path).map_err(|e| {
            AssetSourceError::Archive(format!(
                "Failed to open pak '{}': {e} (run `grf-utils pack` to build one)",
                path.display()
            ))
        })?;

        let mut archive = ZipArchive::new(file).map_err(|e| {
            AssetSourceError::Archive(format!(
                "Failed to read pak zip structure '{}': {e} (run `grf-utils pack` to build one)",
                path.display()
            ))
        })?;

        let manifest = read_manifest(&mut archive, path)?;
        if manifest.format_version != FORMAT_VERSION {
            return Err(AssetSourceError::Archive(format!(
                "Pak '{}' has format_version {} (expected {}); rebuild it with `grf-utils pack`",
                path.display(),
                manifest.format_version,
                FORMAT_VERSION
            )));
        }

        let mut index_by_name = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|e| {
                AssetSourceError::Archive(format!(
                    "Failed to read pak entry at index {index} in '{}': {e}",
                    path.display()
                ))
            })?;
            let entry_name = Self::normalize_path(entry.name());
            if !entry_name.starts_with(LIFTHRASIR_PREFIX) {
                index_by_name.insert(entry_name, index);
            }
        }

        Ok(Self {
            name,
            priority,
            archive: Mutex::new(archive),
            index_by_name,
        })
    }

    fn normalize_path(path: &str) -> String {
        path.replace('\\', "/").to_lowercase()
    }
}

fn read_manifest(
    archive: &mut ZipArchive<File>,
    path: &Path,
) -> Result<PakManifest, AssetSourceError> {
    let mut entry = archive.by_name(MANIFEST_ENTRY).map_err(|e| {
        AssetSourceError::Archive(format!(
            "Pak '{}' is missing {MANIFEST_ENTRY} ({e}); rebuild it with `grf-utils pack`",
            path.display()
        ))
    })?;

    let mut contents = String::new();
    entry.read_to_string(&mut contents).map_err(|e| {
        AssetSourceError::Archive(format!(
            "Failed to read manifest in pak '{}': {e}",
            path.display()
        ))
    })?;

    toml::from_str(&contents).map_err(|e| {
        AssetSourceError::Archive(format!(
            "Failed to parse manifest in pak '{}': {e}",
            path.display()
        ))
    })
}

impl AssetSource for PakSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    fn exists(&self, path: &str) -> bool {
        self.index_by_name.contains_key(&Self::normalize_path(path))
    }

    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        let normalized_path = Self::normalize_path(path);
        let index = *self
            .index_by_name
            .get(&normalized_path)
            .ok_or_else(|| AssetSourceError::NotFound(path.to_string()))?;

        let mut archive = self.archive.lock().unwrap();
        let mut entry = archive.by_index(index).map_err(|e| {
            AssetSourceError::Archive(format!("Failed to read pak entry '{path}': {e}"))
        })?;

        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data).map_err(|e| {
            AssetSourceError::Archive(format!("Failed to decompress pak entry '{path}': {e}"))
        })?;

        Ok(data)
    }

    fn list_files(&self) -> Vec<String> {
        self.index_by_name.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn write_manifest(writer: &mut ZipWriter<File>, format_version: u32) {
        use std::io::Write;

        writer
            .start_file(
                MANIFEST_ENTRY,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(
                format!(
                    "format_version = {format_version}\ncontent_version = 1\ncreated_unix = 0\n"
                )
                .as_bytes(),
            )
            .unwrap();
    }

    fn build_fixture_pak(dir: &Path, name: &str) -> std::path::PathBuf {
        use std::io::Write;

        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);

        writer
            .start_file(
                "data/sprite/x.spr",
                SimpleFileOptions::default().compression_method(CompressionMethod::Zstd),
            )
            .unwrap();
        writer.write_all(b"zstd-bytes").unwrap();

        writer
            .start_file(
                "data/sound.ogg",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"stored-bytes").unwrap();

        writer
            .start_file(
                "data/sprite/몸통/무희.spr",
                SimpleFileOptions::default().compression_method(CompressionMethod::Zstd),
            )
            .unwrap();
        writer.write_all(b"korean-bytes").unwrap();

        writer
            .start_file(
                ".lifthrasir/tombstones",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"data/deleted.txt").unwrap();

        write_manifest(&mut writer, 1);

        writer.finish().unwrap();
        path
    }

    #[test]
    fn exists_load_and_list_files_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = build_fixture_pak(dir.path(), "fixture.pak");

        let source = PakSource::new(&path, 1).unwrap();

        assert!(source.exists("data/sprite/x.spr"));
        assert_eq!(source.load("data/sprite/x.spr").unwrap(), b"zstd-bytes");

        assert!(source.exists("data/sound.ogg"));
        assert_eq!(source.load("data/sound.ogg").unwrap(), b"stored-bytes");

        assert!(source.exists("data/sprite/몸통/무희.spr"));
        assert_eq!(
            source.load("data/sprite/몸통/무희.spr").unwrap(),
            b"korean-bytes"
        );

        let files = source.list_files();
        assert!(files.contains(&"data/sprite/x.spr".to_string()));
        assert!(!files.iter().any(|f| f.starts_with(".lifthrasir/")));
        assert!(!source.exists(".lifthrasir/manifest.toml"));
        assert!(!source.exists(".lifthrasir/tombstones"));
    }

    #[test]
    fn load_normalizes_requested_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = build_fixture_pak(dir.path(), "fixture.pak");

        let source = PakSource::new(&path, 1).unwrap();

        assert!(source.exists("DATA\\Sprite\\X.SPR"));
        assert_eq!(source.load("DATA\\Sprite\\X.SPR").unwrap(), b"zstd-bytes");
    }

    #[test]
    fn new_fails_on_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.pak");

        let err = PakSource::new(&path, 1).unwrap_err();
        assert!(err.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn new_fails_on_non_zip_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-zip.pak");
        std::fs::write(&path, b"this is not a zip file").unwrap();

        let err = PakSource::new(&path, 1).unwrap_err();
        assert!(err.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn new_fails_on_zip_without_manifest() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no-manifest.pak");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("data/a.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"a").unwrap();
        writer.finish().unwrap();

        let err = PakSource::new(&path, 1).unwrap_err();
        assert!(err.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn new_fails_on_unsupported_format_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad-version.pak");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        write_manifest(&mut writer, 2);
        writer.finish().unwrap();

        let err = PakSource::new(&path, 1).unwrap_err();
        let message = err.to_string();
        assert!(message.contains('2'));
        assert!(message.contains(&path.display().to_string()));
    }
}
