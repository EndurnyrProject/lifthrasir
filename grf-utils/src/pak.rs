use anyhow::{Context, Result, bail};
use ro_formats::GrfFile;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Manifest entry name inside a pak (hidden from the runtime's `list_files`/`exists`).
pub const MANIFEST_ENTRY: &str = ".lifthrasir/manifest.toml";
/// Tombstone entry name inside a patch pak (newline-separated normalized paths to delete).
pub const TOMBSTONE_ENTRY: &str = ".lifthrasir/tombstones";
/// Current pak manifest format.
pub const FORMAT_VERSION: u32 = 1;

const GRF_FILE_TYPE_FILE: u8 = 0x01;
const STORED_EXTENSIONS: [&str; 5] = ["ogg", "mp3", "jpg", "png", "ktx2"];
const EXCLUDED_EXTENSIONS: [&str; 5] = ["rsm", "rsm2", "gnd", "gat", "rsw"];

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

fn is_excluded_path(normalized_path: &str) -> bool {
    let extension = Path::new(normalized_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();

    EXCLUDED_EXTENSIONS.contains(&extension.as_str())
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

/// Pack-time dedup outcome: how many entries were written as aliases of
/// byte-identical content already in the pak, and the uncompressed bytes saved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DedupStats {
    pub aliased_entries: usize,
    pub aliased_bytes: u64,
}

/// [`split_duplicates`] output: canonical entries, `(canonical_path, alias_path)`
/// duplicate pairs, and the dedup stats.
type DedupSplit<'a> = (
    Vec<&'a (String, Vec<u8>)>,
    Vec<(&'a str, &'a str)>,
    DedupStats,
);

/// Splits entries into canonical entries (first occurrence of each distinct
/// content) and aliases `(canonical_path, alias_path)` for byte-identical
/// duplicates, which share the canonical entry's data record in the pak.
fn split_duplicates(entries: &SourceEntries) -> DedupSplit<'_> {
    let mut first_by_content: HashMap<&[u8], &str> = HashMap::new();
    let mut canonical = Vec::new();
    let mut aliases = Vec::new();
    let mut stats = DedupStats::default();

    for entry in entries {
        let (path, data) = entry;
        match first_by_content.entry(data.as_slice()) {
            Entry::Vacant(slot) => {
                slot.insert(path);
                canonical.push(entry);
            }
            Entry::Occupied(slot) => {
                aliases.push((*slot.get(), path.as_str()));
                stats.aliased_entries += 1;
                stats.aliased_bytes += data.len() as u64;
            }
        }
    }

    (canonical, aliases, stats)
}

fn exclude_source_formats(entries: SourceEntries) -> (SourceEntries, usize) {
    let mut excluded = 0;
    let entries = entries
        .into_iter()
        .filter(|(path, _)| {
            let exclude = is_excluded_path(path);
            excluded += usize::from(exclude);
            !exclude
        })
        .collect();

    (entries, excluded)
}

fn scan_dir_recursive(dir: &Path, root: &Path, out: &mut SourceEntries) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        // Skip dotfiles (`.DS_Store`, editor droppings): no RO or Lifthrasir asset
        // path starts with a dot, and the pak's own `.lifthrasir/` entries are
        // written separately.
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }

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

/// Recursively scans a Lifthrasir content folder into normalized (path, bytes)
/// entries. Unlike [`scan_data_folder`], paths are taken relative to the folder
/// itself, so the folder *is* the pak root — this mirrors the runtime, where the
/// configured `data_folder` is the root of the `ro://` namespace.
pub fn scan_content_folder(content_folder: &Path) -> SourceEntries {
    let mut entries = Vec::new();
    scan_dir_recursive(content_folder, content_folder, &mut entries);
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
    content_folders: &[PathBuf],
) -> (SourceEntries, usize, usize) {
    let mut tiers: Vec<SourceEntries> = Vec::new();
    if let Some(folder) = data_folder {
        tiers.push(scan_data_folder(folder));
    }
    for folder in content_folders {
        tiers.push(scan_content_folder(folder));
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

    let (entries, excluded) = exclude_source_formats(union_entries(tiers));
    (entries, skipped, excluded)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Writes `entries` into `writer` in order, calling `on_entry` with each normalized
/// path as it is written. Shared by the sequential and sharded pack writers.
fn write_entries(
    writer: &mut ZipWriter<fs::File>,
    entries: &[&(String, Vec<u8>)],
    zstd_level: Option<i32>,
    mut on_entry: impl FnMut(&str),
) -> Result<()> {
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

    Ok(())
}

/// Writes central-directory aliases for duplicate entries: each alias points at the
/// canonical entry's data record (zip shallow copy), so duplicate content is stored once.
/// The runtime's zip reader resolves entries via the central directory, so aliases read
/// like normal entries.
fn write_aliases(
    writer: &mut ZipWriter<fs::File>,
    aliases: &[(&str, &str)],
    mut on_entry: impl FnMut(&str),
) -> Result<()> {
    for (canonical, alias) in aliases {
        on_entry(alias);
        writer
            .shallow_copy_file(canonical, alias)
            .with_context(|| format!("Failed to alias pak entry '{alias}' -> '{canonical}'"))?;
    }

    Ok(())
}

/// Writes the `.lifthrasir/manifest.toml` entry, always last and uncompressed.
fn write_manifest(writer: &mut ZipWriter<fs::File>, content_version: u64) -> Result<()> {
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

    Ok(())
}

/// Writes entries plus the `.lifthrasir/manifest.toml` (written last, uncompressed)
/// to a zip64 pak at `out_path`. Byte-identical duplicate entries are stored once and
/// aliased (see [`write_aliases`]). `on_entry` is called with each normalized path
/// as it is written, for caller-driven progress reporting.
pub fn write_pak(
    entries: &SourceEntries,
    out_path: &Path,
    content_version: u64,
    zstd_level: Option<i32>,
    mut on_entry: impl FnMut(&str),
) -> Result<DedupStats> {
    let (canonical, aliases, stats) = split_duplicates(entries);
    let file = fs::File::create(out_path)
        .with_context(|| format!("Failed to create pak file: {}", out_path.display()))?;
    let mut writer = ZipWriter::new(file);

    write_entries(&mut writer, &canonical, zstd_level, &mut on_entry)?;
    write_aliases(&mut writer, &aliases, &mut on_entry)?;
    write_manifest(&mut writer, content_version)?;
    writer.finish().context("Failed to finalize pak")?;

    Ok(stats)
}

/// Writes one shard to its own part-pak (no manifest): same codec policy as `write_pak`,
/// just a plain zip of that shard's entries.
fn write_shard(
    shard: &[&(String, Vec<u8>)],
    part_path: &Path,
    zstd_level: Option<i32>,
    on_entry: &(impl Fn(&str) + Sync),
) -> Result<()> {
    let file = fs::File::create(part_path)
        .with_context(|| format!("Failed to create pak shard: {}", part_path.display()))?;
    let mut writer = ZipWriter::new(file);

    write_entries(&mut writer, shard, zstd_level, |path| on_entry(path))?;
    writer.finish().context("Failed to finalize pak shard")?;

    Ok(())
}

/// Compresses `canonical` across `jobs` worker threads (one part-pak per shard, no manifest),
/// then concatenates the parts in shard order via `raw_copy_file` (IO-bound, no
/// recompression) plus the duplicate aliases and a fresh manifest, into `out_path`.
/// Part files are always cleaned
/// up, on success or failure.
fn write_pak_sharded(
    canonical: &[&(String, Vec<u8>)],
    aliases: &[(&str, &str)],
    out_path: &Path,
    content_version: u64,
    zstd_level: Option<i32>,
    jobs: usize,
    on_entry: &(impl Fn(&str) + Sync),
) -> Result<()> {
    let chunk_size = canonical.len().div_ceil(jobs).max(1);
    let shards: Vec<&[&(String, Vec<u8>)]> = canonical.chunks(chunk_size).collect();
    let part_paths: Vec<PathBuf> = (0..shards.len())
        .map(|i| suffixed(out_path, &format!(".part{i}")))
        .collect();

    let shard_results: Vec<Result<()>> = thread::scope(|scope| {
        let handles: Vec<_> = shards
            .iter()
            .zip(part_paths.iter())
            .map(|(shard, part_path)| {
                scope.spawn(|| write_shard(shard, part_path, zstd_level, on_entry))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let cleanup_parts = || {
        for part_path in &part_paths {
            fs::remove_file(part_path).ok();
        }
    };

    if let Some(err) = shard_results.into_iter().find_map(|r| r.err()) {
        cleanup_parts();
        return Err(err);
    }

    let concat_result = (|| -> Result<()> {
        let file = fs::File::create(out_path)
            .with_context(|| format!("Failed to create pak file: {}", out_path.display()))?;
        let mut writer = ZipWriter::new(file);

        for part_path in &part_paths {
            let mut part_archive = open_pak(part_path)?;
            let names: Vec<String> = part_archive.file_names().map(String::from).collect();
            for name in &names {
                let part_entry = part_archive
                    .by_name(name)
                    .with_context(|| format!("Failed to read shard entry '{name}'"))?;
                writer
                    .raw_copy_file(part_entry)
                    .with_context(|| format!("Failed to copy shard entry '{name}'"))?;
            }
        }

        write_aliases(&mut writer, aliases, |path| on_entry(path))?;
        write_manifest(&mut writer, content_version)?;
        writer.finish().context("Failed to finalize pak")?;
        Ok(())
    })();

    cleanup_parts();
    concat_result
}

/// Parallel counterpart to `write_pak`: shards `entries` across `jobs` worker threads for
/// the CPU-bound compression step, then concatenates the results. Falls back to the
/// sequential path for `jobs <= 1` or a single entry. Writes to `<out_path>.tmp` and
/// renames onto `out_path` at the end, so a crash never leaves a partial pak at the
/// final path.
pub fn write_pak_parallel(
    entries: &SourceEntries,
    out_path: &Path,
    content_version: u64,
    zstd_level: Option<i32>,
    jobs: usize,
    on_entry: impl Fn(&str) + Sync,
) -> Result<DedupStats> {
    let tmp_path = tmp_path_for(out_path);
    let jobs = jobs.max(1);

    let write_result = if jobs == 1 || entries.len() <= 1 {
        write_pak(entries, &tmp_path, content_version, zstd_level, |path| {
            on_entry(path)
        })
    } else {
        let (canonical, aliases, stats) = split_duplicates(entries);
        write_pak_sharded(
            &canonical,
            &aliases,
            &tmp_path,
            content_version,
            zstd_level,
            jobs,
            &on_entry,
        )
        .map(|()| stats)
    };

    let stats = match write_result {
        Ok(stats) => stats,
        Err(e) => {
            fs::remove_file(&tmp_path).ok();
            return Err(e);
        }
    };

    if out_path.exists() {
        fs::remove_file(out_path)
            .with_context(|| format!("Failed to remove existing pak: {}", out_path.display()))?;
    }
    fs::rename(&tmp_path, out_path)
        .with_context(|| format!("Failed to install pak at '{}'", out_path.display()))?;

    Ok(stats)
}

fn open_pak(path: &Path) -> Result<ZipArchive<fs::File>> {
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

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
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
    use std::io::Read;

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
    fn exclusion_applies_after_all_precedence_tiers_are_unioned() {
        let data_folder = vec![("data/loose.gnd".to_string(), b"loose".to_vec())];
        let content_folder = vec![("maps/content.rsw".to_string(), b"content".to_vec())];
        let grf = vec![
            ("data/model/grf.rsm2".to_string(), b"grf".to_vec()),
            ("data/sprite/grf.spr".to_string(), b"sprite".to_vec()),
        ];

        let (entries, excluded) =
            exclude_source_formats(union_entries(vec![data_folder, content_folder, grf]));

        assert_eq!(excluded, 3);
        assert_eq!(
            entries,
            vec![("data/sprite/grf.spr".to_string(), b"sprite".to_vec())]
        );
    }

    /// A content folder is the pak root: its own name is dropped, so the runtime
    /// finds `assets/data/fonts/x.ttf` at `ro://fonts/x.ttf`. The data folder keeps
    /// its name instead, which is what lets it shadow the GRFs' `data/...` entries.
    #[test]
    fn content_folder_mounts_at_pak_root_unlike_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("data");
        fs::create_dir_all(content.join("fonts")).unwrap();
        fs::write(content.join("fonts/Manrope.ttf"), b"ttf-bytes").unwrap();

        let (entries, skipped, _) = collect_entries(&[], None, std::slice::from_ref(&content));
        assert_eq!(skipped, 0);
        assert_eq!(
            entries,
            vec![("fonts/manrope.ttf".to_string(), b"ttf-bytes".to_vec())]
        );

        let (as_data_folder, _, _) = collect_entries(&[], Some(&content), &[]);
        assert_eq!(
            as_data_folder,
            vec![("data/fonts/manrope.ttf".to_string(), b"ttf-bytes".to_vec())]
        );
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

        let (entries, skipped, _) = collect_entries(&[], Some(&data_folder), &[]);
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

    #[test]
    fn pack_excludes_map_and_model_source_formats_from_loose_data() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for extension in ["rsm", "gnd", "gat", "rsw", "act", "spr", "png", "ogg"] {
            fs::write(data_folder.join(format!("asset.{extension}")), b"bytes").unwrap();
        }

        let (entries, skipped, excluded) = collect_entries(&[], Some(&data_folder), &[]);
        assert_eq!(skipped, 0);
        assert_eq!(excluded, 4);

        let out_path = dir.path().join("out.pak");
        write_pak(&entries, &out_path, 1, None, |_| {}).unwrap();
        let archive_entries = read_all_entries(&out_path);
        let names: HashSet<&str> = archive_entries.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            HashSet::from([
                "data/asset.act",
                "data/asset.spr",
                "data/asset.png",
                "data/asset.ogg",
                MANIFEST_ENTRY,
            ])
        );
    }

    fn read_all_entries(path: &Path) -> std::collections::HashMap<String, Vec<u8>> {
        let file = fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = archive.file_names().map(String::from).collect();

        names
            .into_iter()
            .map(|name| {
                let mut entry = archive.by_name(&name).unwrap();
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents).unwrap();
                (name, contents)
            })
            .collect()
    }

    #[test]
    fn parallel_pack_matches_sequential_pack_content() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for i in 0..7 {
            fs::write(
                data_folder.join(format!("file{i}.txt")),
                vec![b'a' + i as u8; 64],
            )
            .unwrap();
        }
        fs::write(data_folder.join("sound.ogg"), b"ogg-bytes").unwrap();

        let (entries, skipped, _) = collect_entries(&[], Some(&data_folder), &[]);
        assert_eq!(skipped, 0);

        let sequential_path = dir.path().join("sequential.pak");
        write_pak(&entries, &sequential_path, 3, None, |_| {}).unwrap();

        let parallel_path = dir.path().join("parallel.pak");
        write_pak_parallel(&entries, &parallel_path, 3, None, 4, |_| {}).unwrap();

        let sequential_entries = read_all_entries(&sequential_path);
        let parallel_entries = read_all_entries(&parallel_path);

        let sequential_names: HashSet<&String> = sequential_entries.keys().collect();
        let parallel_names: HashSet<&String> = parallel_entries.keys().collect();
        assert_eq!(sequential_names, parallel_names);

        for name in sequential_names {
            if name == MANIFEST_ENTRY {
                continue;
            }
            assert_eq!(
                sequential_entries[name], parallel_entries[name],
                "entry '{name}' differs between sequential and parallel pack"
            );
        }

        let sequential_manifest: PakManifest =
            toml::from_str(std::str::from_utf8(&sequential_entries[MANIFEST_ENTRY]).unwrap())
                .unwrap();
        let parallel_manifest: PakManifest =
            toml::from_str(std::str::from_utf8(&parallel_entries[MANIFEST_ENTRY]).unwrap())
                .unwrap();
        assert_eq!(
            sequential_manifest.format_version,
            parallel_manifest.format_version
        );
        assert_eq!(
            sequential_manifest.content_version,
            parallel_manifest.content_version
        );
    }

    #[test]
    fn parallel_pack_leaves_no_tmp_or_part_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let data_folder = dir.path().join("data");
        fs::create_dir_all(&data_folder).unwrap();
        for i in 0..5 {
            fs::write(data_folder.join(format!("file{i}.txt")), b"payload").unwrap();
        }

        let (entries, _, _) = collect_entries(&[], Some(&data_folder), &[]);
        let out_path = dir.path().join("out.pak");

        write_pak_parallel(&entries, &out_path, 1, None, 3, |_| {}).unwrap();

        assert!(out_path.exists());
        assert!(!tmp_path_for(&out_path).exists());

        let leftover_parts: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".part"))
            .collect();
        assert!(
            leftover_parts.is_empty(),
            "leftover part files: {leftover_parts:?}"
        );

        let file = fs::File::open(&out_path).unwrap();
        zip::ZipArchive::new(file).unwrap();
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
