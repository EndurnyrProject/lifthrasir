use crate::writer::now_unix;
use crate::{FORMAT_VERSION, MANIFEST_ENTRY, PakManifest, TOMBSTONE_ENTRY, normalize_path};
use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(super) fn open_pak(path: &Path) -> Result<ZipArchive<fs::File>> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open pak: {}", path.display()))?;
    ZipArchive::new(file)
        .with_context(|| format!("Failed to read pak zip structure: {}", path.display()))
}

fn read_manifest(archive: &mut ZipArchive<fs::File>) -> Result<PakManifest> {
    let mut entry = archive
        .by_name(MANIFEST_ENTRY)
        .context("Pak is missing .lifthrasir/manifest.toml (run grf-utils pack)")?;
    let mut contents = String::new();
    entry
        .read_to_string(&mut contents)
        .context("Failed to read pak manifest")?;
    toml::from_str(&contents).context("Failed to parse pak manifest")
}

fn read_tombstones(archive: &mut ZipArchive<fs::File>) -> Result<HashSet<String>> {
    match archive.by_name(TOMBSTONE_ENTRY) {
        Ok(mut entry) => {
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .context("Failed to read tombstones entry")?;
            Ok(contents
                .lines()
                .map(normalize_path)
                .filter(|line| !line.is_empty())
                .collect())
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(HashSet::new()),
        Err(e) => Err(e).context("Failed to read tombstones entry"),
    }
}

pub(super) fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Path of the temp file a merge streams into: `<main>.tmp`.
pub fn tmp_path_for(main_path: &Path) -> PathBuf {
    suffixed(main_path, ".tmp")
}

/// Path the previous main pak is moved aside to during a merge swap: `<main>.old`.
pub fn old_path_for(main_path: &Path) -> PathBuf {
    suffixed(main_path, ".old")
}

/// Leftover `<main>.tmp`/`<main>.old` files from a previously interrupted merge.
pub fn detect_leftovers(main_path: &Path) -> Vec<PathBuf> {
    [tmp_path_for(main_path), old_path_for(main_path)]
        .into_iter()
        .filter(|p| p.exists())
        .collect()
}

/// Raw-copies `names` from `archive` into `writer`, preserving dedup: entries that
/// share an already-copied data record (aliases in the source pak, detected via a
/// shared local header offset) are re-created as aliases of the first copied name
/// instead of being re-materialized as full copies.
fn copy_entries_preserving_dedup<'a>(
    writer: &mut ZipWriter<fs::File>,
    archive: &mut ZipArchive<fs::File>,
    names: impl IntoIterator<Item = &'a str>,
    on_entry: &mut impl FnMut(&str),
) -> Result<()> {
    let mut first_by_offset: HashMap<u64, String> = HashMap::new();

    for name in names {
        on_entry(name);
        let file = archive
            .by_name(name)
            .with_context(|| format!("Failed to read entry '{name}'"))?;

        if let Some(canonical) = first_by_offset.get(&file.header_start()) {
            let canonical = canonical.clone();
            drop(file);
            writer
                .shallow_copy_file(&canonical, name)
                .with_context(|| format!("Failed to alias entry '{name}' -> '{canonical}'"))?;
            continue;
        }

        first_by_offset.insert(file.header_start(), name.to_string());
        writer
            .raw_copy_file(file)
            .with_context(|| format!("Failed to copy entry '{name}'"))?;
    }

    Ok(())
}

/// Streams a merged pak to `<main>.tmp`: main's surviving entries (raw-copied, skipping
/// tombstoned and patch-shadowed paths), then the patch's data entries (also raw-copied,
/// no recompression anywhere), then a fresh manifest carrying the patch's `content_version`.
/// Refuses (main untouched, no tmp file created) unless `patch.content_version > main.content_version`.
fn stream_merge(
    main_path: &Path,
    patch_path: &Path,
    mut on_entry: impl FnMut(&str),
) -> Result<(PathBuf, u64, u64)> {
    let mut main_archive = open_pak(main_path)?;
    let main_manifest = read_manifest(&mut main_archive)?;
    if main_manifest.format_version != FORMAT_VERSION {
        bail!(
            "Refusing to merge: main pak '{}' has format_version {} (expected {})",
            main_path.display(),
            main_manifest.format_version,
            FORMAT_VERSION
        );
    }

    let mut patch_archive = open_pak(patch_path)?;
    let patch_manifest = read_manifest(&mut patch_archive)?;
    if patch_manifest.format_version != FORMAT_VERSION {
        bail!(
            "Refusing to merge: patch pak '{}' has format_version {} (expected {})",
            patch_path.display(),
            patch_manifest.format_version,
            FORMAT_VERSION
        );
    }

    if patch_manifest.content_version <= main_manifest.content_version {
        bail!(
            "Refusing to merge: patch content_version ({}) must be greater than main content_version ({})",
            patch_manifest.content_version,
            main_manifest.content_version
        );
    }

    let tombstones = read_tombstones(&mut patch_archive)?;

    let patch_names: Vec<String> = patch_archive
        .file_names()
        .filter(|name| *name != MANIFEST_ENTRY && *name != TOMBSTONE_ENTRY)
        .map(String::from)
        .collect();
    let patch_name_set: HashSet<&str> = patch_names.iter().map(String::as_str).collect();

    let main_names: Vec<String> = main_archive
        .file_names()
        .filter(|name| *name != MANIFEST_ENTRY)
        .map(String::from)
        .collect();
    let survivor_names: Vec<&String> = main_names
        .iter()
        .filter(|name| {
            !tombstones.contains(name.as_str()) && !patch_name_set.contains(name.as_str())
        })
        .collect();

    let tmp_path = tmp_path_for(main_path);
    let tmp_file = fs::File::create(&tmp_path)
        .with_context(|| format!("Failed to create merge temp file: {}", tmp_path.display()))?;
    let mut writer = ZipWriter::new(tmp_file);

    copy_entries_preserving_dedup(
        &mut writer,
        &mut main_archive,
        survivor_names.iter().map(|name| name.as_str()),
        &mut on_entry,
    )
    .context("Failed to copy main pak entries")?;

    copy_entries_preserving_dedup(
        &mut writer,
        &mut patch_archive,
        patch_names.iter().map(String::as_str),
        &mut on_entry,
    )
    .context("Failed to copy patch pak entries")?;

    let manifest = PakManifest {
        format_version: FORMAT_VERSION,
        content_version: patch_manifest.content_version,
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

    writer
        .finish()
        .context("Failed to finalize merge temp pak")?;

    let expected_entries = (survivor_names.len() + patch_names.len()) as u64;
    Ok((tmp_path, expected_entries, patch_manifest.content_version))
}

/// Validates `tmp_path` (reopens as a zip, checks entry count and manifest) and, on success,
/// swaps it into place at `main_path`. On failure the temp file is deleted and `main_path`
/// is left untouched.
fn finish_merge(
    main_path: &Path,
    tmp_path: &Path,
    expected_entries: u64,
    expected_content_version: u64,
) -> Result<()> {
    let validation: Result<()> = (|| {
        let mut archive = open_pak(tmp_path)?;
        if archive.len() as u64 != expected_entries + 1 {
            bail!(
                "entry count mismatch: expected {} data entries + manifest, found {}",
                expected_entries,
                archive.len()
            );
        }
        let manifest = read_manifest(&mut archive)?;
        if manifest.format_version != FORMAT_VERSION
            || manifest.content_version != expected_content_version
        {
            bail!(
                "manifest mismatch in merged pak: {manifest:?} (expected content_version {expected_content_version})"
            );
        }
        Ok(())
    })();

    if let Err(e) = validation {
        fs::remove_file(tmp_path).ok();
        return Err(e).context("Merge validation failed; temp file removed, main pak untouched");
    }

    let old_path = old_path_for(main_path);
    fs::remove_file(&old_path).ok();
    fs::rename(main_path, &old_path)
        .with_context(|| format!("Failed to move aside '{}'", main_path.display()))?;
    fs::rename(tmp_path, main_path)
        .with_context(|| format!("Failed to install merged pak at '{}'", main_path.display()))?;
    fs::remove_file(&old_path).ok();

    Ok(())
}

/// Applies a patch pak onto a main pak in place, per the merge lifecycle: refuse on a stale
/// patch version, stream survivors + patch entries to a temp file, validate, then swap.
pub fn merge_paks(main_path: &Path, patch_path: &Path, on_entry: impl FnMut(&str)) -> Result<()> {
    let (tmp_path, expected_entries, expected_content_version) =
        stream_merge(main_path, patch_path, on_entry)?;
    finish_merge(
        main_path,
        &tmp_path,
        expected_entries,
        expected_content_version,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{codec_for, write_pak};
    use crate::{SourceEntries, write_pak_parallel};

    #[test]
    fn codec_policy_stores_already_compressed_formats() {
        for ext in ["ogg", "mp3", "jpg", "png", "ktx2", "OGG", "Png", "KTX2"] {
            assert_eq!(
                codec_for(&format!("data/sound/x.{ext}")),
                CompressionMethod::Stored,
                "expected Stored for .{ext}"
            );
        }
    }

    #[test]
    fn codec_policy_zstd_for_everything_else() {
        for ext in ["spr", "act", "bmp", "tga", "wav", "txt", "lub"] {
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

    fn write_test_pak(
        dir: &Path,
        name: &str,
        content_version: u64,
        entries: &[(&str, &[u8])],
        tombstones: Option<&[&str]>,
    ) -> PathBuf {
        let path = dir.join(name);
        let file = fs::File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);

        for (entry_path, data) in entries {
            writer
                .start_file(
                    *entry_path,
                    SimpleFileOptions::default().compression_method(codec_for(entry_path)),
                )
                .unwrap();
            writer.write_all(data).unwrap();
        }

        if let Some(tombstoned_paths) = tombstones {
            writer
                .start_file(
                    TOMBSTONE_ENTRY,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
                )
                .unwrap();
            writer
                .write_all(tombstoned_paths.join("\n").as_bytes())
                .unwrap();
        }

        let manifest = PakManifest {
            format_version: FORMAT_VERSION,
            content_version,
            created_unix: 0,
        };
        writer
            .start_file(
                MANIFEST_ENTRY,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(toml::to_string_pretty(&manifest).unwrap().as_bytes())
            .unwrap();

        writer.finish().unwrap();
        path
    }

    #[test]
    fn merge_applies_add_replace_and_tombstone() {
        let dir = tempfile::tempdir().unwrap();
        let main_path = write_test_pak(
            dir.path(),
            "main.pak",
            1,
            &[
                ("data/keep.txt", b"keep-me"),
                ("data/replace.txt", b"old-bytes"),
                ("data/gone.txt", b"tombstoned-bytes"),
            ],
            None,
        );
        let patch_path = write_test_pak(
            dir.path(),
            "patch.pak",
            2,
            &[
                ("data/replace.txt", b"new-bytes"),
                ("data/added.txt", b"added-bytes"),
            ],
            Some(&["data/gone.txt"]),
        );

        merge_paks(&main_path, &patch_path, |_| {}).unwrap();

        let file = fs::File::open(&main_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut replaced = archive.by_name("data/replace.txt").unwrap();
        let mut replaced_contents = Vec::new();
        replaced.read_to_end(&mut replaced_contents).unwrap();
        assert_eq!(replaced_contents, b"new-bytes");
        drop(replaced);

        let mut added = archive.by_name("data/added.txt").unwrap();
        let mut added_contents = Vec::new();
        added.read_to_end(&mut added_contents).unwrap();
        assert_eq!(added_contents, b"added-bytes");
        drop(added);

        let mut kept = archive.by_name("data/keep.txt").unwrap();
        let mut kept_contents = Vec::new();
        kept.read_to_end(&mut kept_contents).unwrap();
        assert_eq!(kept_contents, b"keep-me");
        drop(kept);

        assert!(archive.by_name("data/gone.txt").is_err());
        assert!(archive.by_name(TOMBSTONE_ENTRY).is_err());

        let mut manifest_entry = archive.by_name(MANIFEST_ENTRY).unwrap();
        let mut manifest_contents = String::new();
        manifest_entry
            .read_to_string(&mut manifest_contents)
            .unwrap();
        let manifest: PakManifest = toml::from_str(&manifest_contents).unwrap();
        assert_eq!(manifest.content_version, 2);
        drop(manifest_entry);

        assert_eq!(archive.len(), 4);
        assert!(!tmp_path_for(&main_path).exists());
        assert!(!old_path_for(&main_path).exists());
    }

    #[test]
    fn merge_refuses_stale_patch_version() {
        let dir = tempfile::tempdir().unwrap();
        let main_path = write_test_pak(dir.path(), "main.pak", 5, &[("data/a.txt", b"a")], None);
        let patch_path = write_test_pak(dir.path(), "patch.pak", 3, &[("data/b.txt", b"b")], None);
        let main_bytes_before = fs::read(&main_path).unwrap();

        let err = merge_paks(&main_path, &patch_path, |_| {}).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains('5'), "message: {message}");
        assert!(message.contains('3'), "message: {message}");

        assert_eq!(fs::read(&main_path).unwrap(), main_bytes_before);
        assert!(!tmp_path_for(&main_path).exists());
    }

    #[test]
    fn merge_refuses_main_pak_with_unsupported_format_version() {
        let dir = tempfile::tempdir().unwrap();

        let main_path = dir.path().join("main.pak");
        let file = fs::File::create(&main_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let manifest = PakManifest {
            format_version: 2,
            content_version: 1,
            created_unix: 0,
        };
        writer
            .start_file(
                MANIFEST_ENTRY,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(toml::to_string_pretty(&manifest).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();

        let patch_path = write_test_pak(dir.path(), "patch.pak", 2, &[("data/b.txt", b"b")], None);
        let main_bytes_before = fs::read(&main_path).unwrap();

        let err = merge_paks(&main_path, &patch_path, |_| {}).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains('2'), "message: {message}");
        assert!(
            message.contains(&main_path.display().to_string()),
            "message: {message}"
        );

        assert_eq!(fs::read(&main_path).unwrap(), main_bytes_before);
        assert!(!tmp_path_for(&main_path).exists());
    }

    #[test]
    fn merge_validation_failure_removes_tmp_and_preserves_main() {
        let dir = tempfile::tempdir().unwrap();
        let main_path = write_test_pak(dir.path(), "main.pak", 1, &[("data/a.txt", b"a")], None);
        let main_bytes_before = fs::read(&main_path).unwrap();

        let tmp_path = tmp_path_for(&main_path);
        fs::write(&tmp_path, b"not a zip file").unwrap();

        let result = finish_merge(&main_path, &tmp_path, 1, 2);

        assert!(result.is_err());
        assert!(!tmp_path.exists());
        assert_eq!(fs::read(&main_path).unwrap(), main_bytes_before);
    }

    #[test]
    fn detect_leftovers_finds_tmp_and_old() {
        let dir = tempfile::tempdir().unwrap();
        let main_path = dir.path().join("main.pak");
        fs::write(&main_path, b"main").unwrap();
        assert!(detect_leftovers(&main_path).is_empty());

        fs::write(tmp_path_for(&main_path), b"tmp").unwrap();
        fs::write(old_path_for(&main_path), b"old").unwrap();

        let leftovers = detect_leftovers(&main_path);
        assert_eq!(leftovers.len(), 2);
    }

    fn entry_header_start(archive: &mut ZipArchive<fs::File>, name: &str) -> u64 {
        archive.by_name(name).unwrap().header_start()
    }

    fn read_entry(archive: &mut ZipArchive<fs::File>, name: &str) -> Vec<u8> {
        let mut entry = archive.by_name(name).unwrap();
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).unwrap();
        contents
    }

    #[test]
    fn pack_dedupes_byte_identical_entries() {
        let dir = tempfile::tempdir().unwrap();
        let entries: SourceEntries = vec![
            ("maps/a/tex/shared.spr".into(), b"same-bytes".to_vec()),
            ("data/unique.txt".into(), b"unique-bytes".to_vec()),
            ("maps/b/tex/shared.spr".into(), b"same-bytes".to_vec()),
            ("models/tex/shared.spr".into(), b"same-bytes".to_vec()),
        ];

        let out_path = dir.path().join("out.pak");
        let stats = write_pak(&entries, &out_path, 1, None, |_| {}).unwrap();

        assert_eq!(stats.aliased_entries, 2);
        assert_eq!(stats.aliased_bytes, 2 * b"same-bytes".len() as u64);

        let mut archive = open_pak(&out_path).unwrap();
        assert_eq!(archive.len(), 5, "4 data entries + manifest");

        let canonical = entry_header_start(&mut archive, "maps/a/tex/shared.spr");
        assert_eq!(
            entry_header_start(&mut archive, "maps/b/tex/shared.spr"),
            canonical,
            "duplicate must share the canonical data record"
        );
        assert_eq!(
            entry_header_start(&mut archive, "models/tex/shared.spr"),
            canonical
        );
        assert_ne!(
            entry_header_start(&mut archive, "data/unique.txt"),
            canonical
        );

        assert_eq!(
            read_entry(&mut archive, "maps/b/tex/shared.spr"),
            b"same-bytes"
        );
        assert_eq!(
            read_entry(&mut archive, "models/tex/shared.spr"),
            b"same-bytes"
        );
        assert_eq!(read_entry(&mut archive, "data/unique.txt"), b"unique-bytes");
    }

    #[test]
    fn parallel_pack_dedupes_across_shards() {
        let dir = tempfile::tempdir().unwrap();
        let mut entries: SourceEntries = (0..20)
            .map(|i| {
                (
                    format!("data/file{i:02}.txt"),
                    format!("content-{i}").into_bytes(),
                )
            })
            .collect();
        // Duplicates at opposite ends so 4 shards place them in different parts.
        entries[0].1 = b"dup-bytes".to_vec();
        entries[19].1 = b"dup-bytes".to_vec();

        let out_path = dir.path().join("out.pak");
        let stats = write_pak_parallel(&entries, &out_path, 1, None, 4, |_| {}).unwrap();

        assert_eq!(stats.aliased_entries, 1);

        let mut archive = open_pak(&out_path).unwrap();
        assert_eq!(
            entry_header_start(&mut archive, "data/file19.txt"),
            entry_header_start(&mut archive, "data/file00.txt"),
        );
        assert_eq!(read_entry(&mut archive, "data/file19.txt"), b"dup-bytes");
    }

    #[test]
    fn merge_preserves_dedup_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let entries: SourceEntries = vec![
            ("data/a.txt".into(), b"shared-bytes".to_vec()),
            ("data/b.txt".into(), b"shared-bytes".to_vec()),
            ("data/c.txt".into(), b"other-bytes".to_vec()),
        ];
        let main_path = dir.path().join("main.pak");
        write_pak(&entries, &main_path, 1, None, |_| {}).unwrap();
        let patch_path = write_test_pak(
            dir.path(),
            "patch.pak",
            2,
            &[("data/d.txt", b"patch-bytes")],
            None,
        );

        merge_paks(&main_path, &patch_path, |_| {}).unwrap();

        let mut archive = open_pak(&main_path).unwrap();
        assert_eq!(
            entry_header_start(&mut archive, "data/b.txt"),
            entry_header_start(&mut archive, "data/a.txt"),
            "merge must not re-materialize aliases as full copies"
        );
        assert_eq!(read_entry(&mut archive, "data/a.txt"), b"shared-bytes");
        assert_eq!(read_entry(&mut archive, "data/b.txt"), b"shared-bytes");
        assert_eq!(read_entry(&mut archive, "data/c.txt"), b"other-bytes");
        assert_eq!(read_entry(&mut archive, "data/d.txt"), b"patch-bytes");
    }

    #[test]
    fn merge_keeps_alias_readable_when_canonical_is_tombstoned() {
        let dir = tempfile::tempdir().unwrap();
        let entries: SourceEntries = vec![
            ("data/a.txt".into(), b"shared-bytes".to_vec()),
            ("data/b.txt".into(), b"shared-bytes".to_vec()),
        ];
        let main_path = dir.path().join("main.pak");
        write_pak(&entries, &main_path, 1, None, |_| {}).unwrap();
        let patch_path = write_test_pak(dir.path(), "patch.pak", 2, &[], Some(&["data/a.txt"]));

        merge_paks(&main_path, &patch_path, |_| {}).unwrap();

        let mut archive = open_pak(&main_path).unwrap();
        assert_eq!(read_entry(&mut archive, "data/b.txt"), b"shared-bytes");
        assert!(matches!(
            archive.by_name("data/a.txt"),
            Err(zip::result::ZipError::FileNotFound)
        ));
    }
}
