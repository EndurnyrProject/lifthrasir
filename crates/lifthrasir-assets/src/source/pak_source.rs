use super::{AssetSource, AssetSourceError};
use lifthrasir_pak::{PakError, PakReader};
use std::path::Path;

#[cfg(test)]
use std::fs::File;
#[cfg(test)]
const MANIFEST_ENTRY: &str = ".lifthrasir/manifest.toml";

pub struct PakSource {
    name: String,
    priority: u32,
    reader: PakReader,
}

impl std::fmt::Debug for PakSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PakSource(name: {}, priority: {}, entries: {})",
            self.name,
            self.priority,
            self.reader.list_files().len()
        )
    }
}

fn source_error(error: PakError) -> AssetSourceError {
    match error {
        PakError::NotFound(path) => AssetSourceError::NotFound(path),
        PakError::Archive(message) => AssetSourceError::Archive(message),
    }
}

impl PakSource {
    pub fn new<P: AsRef<Path>>(path: P, priority: u32) -> Result<Self, AssetSourceError> {
        let path = path.as_ref();
        Ok(Self {
            name: format!("Pak({})", path.display()),
            priority,
            reader: PakReader::open(path).map_err(source_error)?,
        })
    }
}

impl AssetSource for PakSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn priority(&self) -> u32 {
        self.priority
    }
    fn exists(&self, path: &str) -> bool {
        self.reader.exists(path)
    }
    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        self.reader.read(path).map_err(source_error)
    }
    fn list_files(&self) -> Vec<String> {
        self.reader.list_files()
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
