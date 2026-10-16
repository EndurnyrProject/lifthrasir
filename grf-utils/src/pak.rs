use anyhow::{Context, Result};
use lifthrasir_pak::{SourceEntries, normalize_path};
use ro_formats::GrfFile;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const GRF_FILE_TYPE_FILE: u8 = 0x01;
const EXCLUDED_EXTENSIONS: [&str; 5] = ["rsm", "rsm2", "gnd", "gat", "rsw"];

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

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_pak::{MANIFEST_ENTRY, PakManifest, write_pak_parallel};
    use std::io::Read;
    use zip::CompressionMethod;

    fn tmp_path_for(path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_owned();
        name.push(".tmp");
        PathBuf::from(name)
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
        write_pak_parallel(&entries, &out_path, 7, None, 1, |_| {}).unwrap();

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
        write_pak_parallel(&entries, &out_path, 1, None, 1, |_| {}).unwrap();
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
        write_pak_parallel(&entries, &sequential_path, 3, None, 1, |_| {}).unwrap();

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
}
