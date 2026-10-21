use crate::{Diagnostic, DiagnosticLevel, PackInputs};
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
fn scan_grf(
    path: &Path,
    skipped: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<SourceEntries> {
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
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!(
                        "skipping corrupt entry '{}' in {}",
                        entry.filename,
                        path.display()
                    ),
                });
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
) -> PackInputs {
    collect_entries_with_pruning(grf_paths, data_folder, content_folders, false)
}

/// As [`collect_entries`], optionally omitting converted prop GLBs that are no
/// longer referenced by any map GLB in the content folders. Actor GLBs under
/// `models/3dmob` are never treated as map props.
pub fn collect_entries_with_pruning(
    grf_paths: &[PathBuf],
    data_folder: Option<&Path>,
    content_folders: &[PathBuf],
    prune_unused_props: bool,
) -> PackInputs {
    let mut tiers: Vec<SourceEntries> = Vec::new();
    if let Some(folder) = data_folder {
        tiers.push(scan_data_folder(folder));
    }
    for folder in content_folders {
        tiers.push(scan_content_folder(folder));
    }

    let mut skipped = 0usize;
    let mut diagnostics = Vec::new();
    for grf_path in grf_paths {
        match scan_grf(grf_path, &mut skipped, &mut diagnostics) {
            Ok(entries) => tiers.push(entries),
            Err(e) => {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("skipping unreadable GRF '{}': {e:#}", grf_path.display()),
                });
                skipped += 1;
            }
        }
    }

    let (mut entries, excluded) = exclude_source_formats(union_entries(tiers));
    let mut pruned = 0;
    if prune_unused_props {
        let referenced = referenced_prop_models(&entries);
        entries.retain(|(path, _)| {
            let candidate = path.starts_with("models/")
                && path.ends_with(".glb")
                && !path.starts_with("models/3dmob/");
            let remove = candidate && !referenced.contains(path);
            pruned += usize::from(remove);
            !remove
        });
        if pruned > 0 {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Info,
                message: format!("pruned {pruned} unreferenced prop model(s)"),
            });
        }
    }
    PackInputs {
        entries,
        skipped,
        excluded,
        diagnostics,
    }
}

fn referenced_prop_models(entries: &SourceEntries) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for (path, bytes) in entries {
        if !path.starts_with("maps/") || !path.ends_with(".glb") {
            continue;
        }
        let Ok(json) = glb_json(bytes) else { continue };
        let Some(nodes) = json.get("nodes").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for node in nodes {
            let Some(model) = node
                .pointer("/extras/lif_prop/model")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let model = model.trim_start_matches("ro://").to_lowercase();
            if let Some(model) = model.strip_prefix("data/") {
                // Old maps can still point at source models; those are not
                // candidates for pruning, but keeping the normalized key is
                // useful for converted references below.
                referenced.insert(model.to_string());
            } else {
                referenced.insert(model);
            }
        }
    }
    referenced
}

fn glb_json(bytes: &[u8]) -> Result<serde_json::Value, ()> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return Err(());
    }
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| ())?) as usize;
    if &bytes[16..20] != b"JSON" || 20 + json_len > bytes.len() {
        return Err(());
    }
    serde_json::from_slice(&bytes[20..20 + json_len]).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let PackInputs {
            entries, skipped, ..
        } = collect_entries(&[], None, std::slice::from_ref(&content));
        assert_eq!(skipped, 0);
        assert_eq!(
            entries,
            vec![("fonts/manrope.ttf".to_string(), b"ttf-bytes".to_vec())]
        );

        let PackInputs {
            entries: as_data_folder,
            ..
        } = collect_entries(&[], Some(&content), &[]);
        assert_eq!(
            as_data_folder,
            vec![("data/fonts/manrope.ttf".to_string(), b"ttf-bytes".to_vec())]
        );
    }
}
