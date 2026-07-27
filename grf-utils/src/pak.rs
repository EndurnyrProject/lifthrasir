use anyhow::{Context, Result};
use ro_formats::GrfFile;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Manifest entry name inside a pak (hidden from the runtime's `list_files`/`exists`).
pub const MANIFEST_ENTRY: &str = ".lifthrasir/manifest.toml";
/// Tombstone entry name inside a patch pak (newline-separated normalized paths to delete).
// NOTE: unused until the `merge` subcommand (Task 2) consumes it.
#[allow(dead_code)]
pub const TOMBSTONE_ENTRY: &str = ".lifthrasir/tombstones";
/// Current pak manifest format.
pub const FORMAT_VERSION: u32 = 1;

const GRF_FILE_TYPE_FILE: u8 = 0x01;
const STORED_EXTENSIONS: [&str; 4] = ["ogg", "mp3", "jpg", "png"];

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PakManifest {
    pub format_version: u32,
    pub content_version: u64,
    pub created_unix: u64,
}

/// One precedence tier's entries: normalized path -> file bytes.
pub type SourceEntries = Vec<(String, Vec<u8>)>;

/// Converts a raw path (GRF backslashes or data-folder relative path) into the
/// pak profile's normalized form: forward slashes, lowercase.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Pack-time codec policy: already-compressed formats are stored as-is, everything else is zstd.
pub fn codec_for(normalized_path: &str) -> CompressionMethod {
    let extension = Path::new(normalized_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if STORED_EXTENSIONS.contains(&extension.as_str()) {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Zstd
    }
}

/// Merges precedence tiers into a flat entry list. Earlier tiers win on path collisions.
pub fn union_entries(sources: Vec<SourceEntries>) -> SourceEntries {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for tier in sources {
        for (path, data) in tier {
            if seen.insert(path.clone()) {
                result.push((path, data));
            }
        }
    }

    result
}

fn scan_dir_recursive(dir: &Path, root: &Path, out: &mut SourceEntries) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        if path.is_dir() {
            scan_dir_recursive(&path, root, out);
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let Some(relative_str) = relative.to_str() else {
            continue;
        };
        let Ok(data) = fs::read(&path) else {
            continue;
        };

        out.push((normalize_path(relative_str), data));
    }
}

/// Recursively scans a loose data folder into normalized (path, bytes) entries.
/// Paths are taken relative to the folder's parent, so its own name (conventionally
/// `data`, mirroring the GRF-internal root) is kept as the leading path component —
/// this is what lets an entry shadow the same-named GRF entry.
pub fn scan_data_folder(data_folder: &Path) -> SourceEntries {
    let root = data_folder.parent().unwrap_or(Path::new(""));
    let mut entries = Vec::new();
    scan_dir_recursive(data_folder, root, &mut entries);
    entries
}

/// Reads a GRF's file entries. Directory placeholders are skipped silently;
/// entries that fail to decompress are warned, skipped, and counted (same
/// tolerance as the existing `extract` flow).
fn scan_grf(path: &Path, skipped: &mut usize) -> Result<SourceEntries> {
    let grf = GrfFile::from_path(path.to_path_buf())
        .with_context(|| format!("Failed to load GRF file: {}", path.display()))?;

    let mut entries = Vec::new();
    for entry in &grf.entries {
        if entry.file_type & GRF_FILE_TYPE_FILE == 0 {
            continue;
        }

        match grf.get_file(&entry.filename) {
            Some(data) => entries.push((normalize_path(&entry.filename), data)),
            None => {
                eprintln!(
                    "Warning: skipping corrupt entry '{}' in {}",
                    entry.filename,
                    path.display()
                );
                *skipped += 1;
            }
        }
    }

    Ok(entries)
}

/// Builds the precedence-ordered union of a data folder and GRF files:
/// data folder wins over all GRFs, earlier `--grf` flags win over later ones.
/// Unreadable GRFs are warned and skipped rather than failing the whole pack.
pub fn collect_entries(
    grf_paths: &[PathBuf],
    data_folder: Option<&Path>,
) -> (SourceEntries, usize) {
    let mut tiers: Vec<SourceEntries> = Vec::new();
    if let Some(folder) = data_folder {
        tiers.push(scan_data_folder(folder));
    }

    let mut skipped = 0usize;
    for grf_path in grf_paths {
        match scan_grf(grf_path, &mut skipped) {
            Ok(entries) => tiers.push(entries),
            Err(e) => {
                eprintln!(
                    "Warning: skipping unreadable GRF '{}': {e:#}",
                    grf_path.display()
                );
                skipped += 1;
            }
        }
    }

    (union_entries(tiers), skipped)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Writes entries plus the `.lifthrasir/manifest.toml` (written last, uncompressed)
/// to a zip64 pak at `out_path`. `on_entry` is called with each normalized path
/// as it is written, for caller-driven progress reporting.
pub fn write_pak(
    entries: &SourceEntries,
    out_path: &Path,
    content_version: u64,
    zstd_level: Option<i32>,
    mut on_entry: impl FnMut(&str),
) -> Result<()> {
    let file = fs::File::create(out_path)
        .with_context(|| format!("Failed to create pak file: {}", out_path.display()))?;
    let mut writer = ZipWriter::new(file);

    for (path, data) in entries {
        on_entry(path);

        let method = codec_for(path);
        let mut options = SimpleFileOptions::default().compression_method(method);
        if method == CompressionMethod::Zstd
            && let Some(level) = zstd_level
        {
            options = options.compression_level(Some(level as i64));
        }
        if data.len() as u64 >= u32::MAX as u64 {
            options = options.large_file(true);
        }

        writer
            .start_file(path, options)
            .with_context(|| format!("Failed to start pak entry '{path}'"))?;
        writer
            .write_all(data)
            .with_context(|| format!("Failed to write pak entry '{path}'"))?;
    }

    let manifest = PakManifest {
        format_version: FORMAT_VERSION,
        content_version,
        created_unix: now_unix(),
    };
    let manifest_toml =
        toml::to_string_pretty(&manifest).context("Failed to serialize manifest")?;

    writer
        .start_file(
            MANIFEST_ENTRY,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .context("Failed to start manifest entry")?;
    writer
        .write_all(manifest_toml.as_bytes())
        .context("Failed to write manifest entry")?;

    writer.finish().context("Failed to finalize pak")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn codec_policy_stores_already_compressed_formats() {
        for ext in ["ogg", "mp3", "jpg", "png", "OGG", "Png"] {
            assert_eq!(
                codec_for(&format!("data/sound/x.{ext}")),
                CompressionMethod::Stored,
                "expected Stored for .{ext}"
            );
        }
    }

    #[test]
    fn codec_policy_zstd_for_everything_else() {
        for ext in [
            "spr", "act", "gnd", "gat", "rsw", "rsm", "bmp", "tga", "wav", "txt", "lub",
        ] {
            assert_eq!(
                codec_for(&format!("data/model/x.{ext}")),
                CompressionMethod::Zstd,
                "expected Zstd for .{ext}"
            );
        }
    }

    #[test]
    fn normalize_path_converts_backslashes_and_lowercases() {
        assert_eq!(
            normalize_path("data\\Sprite\\몸통\\X.spr"),
            "data/sprite/몸통/x.spr"
        );
    }

    #[test]
    fn union_entries_prefers_earlier_tiers() {
        let data_folder = vec![("data/a.txt".to_string(), b"from-data-folder".to_vec())];
        let first_grf = vec![
            ("data/a.txt".to_string(), b"from-first-grf".to_vec()),
            ("data/b.txt".to_string(), b"from-first-grf-b".to_vec()),
        ];
        let second_grf = vec![
            ("data/a.txt".to_string(), b"from-second-grf".to_vec()),
            ("data/b.txt".to_string(), b"from-second-grf-b".to_vec()),
            ("data/c.txt".to_string(), b"from-second-grf-c".to_vec()),
        ];

        let merged = union_entries(vec![data_folder, first_grf, second_grf]);
        let as_map: std::collections::HashMap<_, _> = merged.into_iter().collect();

        assert_eq!(as_map["data/a.txt"], b"from-data-folder");
        assert_eq!(as_map["data/b.txt"], b"from-first-grf-b");
        assert_eq!(as_map["data/c.txt"], b"from-second-grf-c");
    }

    #[test]
    fn pack_produces_zip_openable_by_plain_zip_archive() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(data_folder.join("sprite/몸통")).unwrap();
        fs::write(data_folder.join("sprite/x.spr"), b"spr-bytes").unwrap();
        fs::write(data_folder.join("sound.ogg"), b"ogg-bytes").unwrap();
        fs::write(
            data_folder.join("sprite/몸통/무희.spr"),
            b"korean-name-bytes",
        )
        .unwrap();

        let (entries, skipped) = collect_entries(&[], Some(&data_folder));
        assert_eq!(skipped, 0);

        let out_path = dir.path().join("out.pak");
        write_pak(&entries, &out_path, 7, None, |_| {}).unwrap();

        let file = fs::File::open(&out_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut spr = archive.by_name("data/sprite/x.spr").unwrap();
        let mut spr_contents = Vec::new();
        spr.read_to_end(&mut spr_contents).unwrap();
        assert_eq!(spr_contents, b"spr-bytes");
        drop(spr);

        let ogg = archive.by_name("data/sound.ogg").unwrap();
        assert_eq!(ogg.compression(), CompressionMethod::Stored);
        drop(ogg);

        let mut korean = archive.by_name("data/sprite/몸통/무희.spr").unwrap();
        let mut korean_contents = Vec::new();
        korean.read_to_end(&mut korean_contents).unwrap();
        assert_eq!(korean_contents, b"korean-name-bytes");
        drop(korean);

        let mut manifest_entry = archive.by_name(MANIFEST_ENTRY).unwrap();
        let mut manifest_contents = String::new();
        manifest_entry
            .read_to_string(&mut manifest_contents)
            .unwrap();
        let manifest: PakManifest = toml::from_str(&manifest_contents).unwrap();
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.content_version, 7);
    }
}
