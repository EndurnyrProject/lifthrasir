use crate::merge::{open_pak, suffixed, tmp_path_for};
use crate::{FORMAT_VERSION, MANIFEST_ENTRY, PakManifest, SourceEntries};
use anyhow::{Context, Result};
use std::collections::{HashMap, hash_map::Entry};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const STORED_EXTENSIONS: [&str; 5] = ["ogg", "mp3", "jpg", "png", "ktx2"];

/// Pack-time codec policy: already-compressed formats are stored as-is, everything else is zstd.
pub(super) fn codec_for(normalized_path: &str) -> CompressionMethod {
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

pub(super) fn now_unix() -> u64 {
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
pub(super) fn write_pak(
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
