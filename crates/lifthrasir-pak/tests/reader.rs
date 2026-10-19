use lifthrasir_pak::PakReader;
use std::fs::File;
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[test]
fn rejects_missing_malformed_and_unsupported_manifests_with_archive_context() {
    let dir = tempfile::tempdir().unwrap();
    for (name, manifest, diagnostic) in [
        ("missing.pak", None, "missing .lifthrasir/manifest.toml"),
        (
            "malformed.pak",
            Some("format_version = ["),
            "Failed to parse manifest",
        ),
        (
            "version.pak",
            Some("format_version = 2"),
            "format_version 2 (expected 1)",
        ),
    ] {
        let path = dir.path().join(name);
        let mut writer = ZipWriter::new(File::create(&path).unwrap());
        if let Some(manifest) = manifest {
            writer
                .start_file(".lifthrasir/manifest.toml", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(manifest.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        let error = PakReader::open(&path).unwrap_err().to_string();
        assert!(
            error.contains(name) && error.contains(diagnostic),
            "{error}"
        );
    }
}

#[test]
fn minimal_manifest_keeps_aliases_codecs_and_hidden_metadata_compatible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("minimal.pak");
    let mut writer = ZipWriter::new(File::create(&path).unwrap());
    for (name, bytes, codec) in [
        (
            ".lifthrasir/manifest.toml",
            b"format_version = 1\n".as_slice(),
            CompressionMethod::Stored,
        ),
        (
            "DATA/Sprite/몸통/무희.SPR",
            b"sprite".as_slice(),
            CompressionMethod::Zstd,
        ),
        (
            "data/SOUND.ogg",
            b"sound".as_slice(),
            CompressionMethod::Stored,
        ),
        (
            ".LIFTHRASIR/tombstones",
            b"hidden".as_slice(),
            CompressionMethod::Stored,
        ),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default().compression_method(codec))
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    let reader = PakReader::open(&path).unwrap();
    assert_eq!(
        reader.read("data\\sprite\\몸통\\무희.spr").unwrap(),
        b"sprite"
    );
    assert_eq!(reader.read("DATA\\sound.OGG").unwrap(), b"sound");
    assert!(!reader.exists(".lifthrasir/manifest.toml"));
    assert!(!reader.exists(".lifthrasir/tombstones"));
    assert!(reader.read(".lifthrasir/manifest.toml").is_err());
    let mut paths = reader.list_files();
    paths.sort();
    assert_eq!(paths, ["data/sound.ogg", "data/sprite/몸통/무희.spr"]);
}
