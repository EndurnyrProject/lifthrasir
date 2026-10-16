use anyhow::Context;
use ro_formats::grf::{GrfEntry as ArchiveEntry, GrfFile};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::GrfEntry as ConfigGrfEntry;

pub(crate) trait GrfReadable {
    fn get(&self, normalized_path: &str) -> Option<Vec<u8>>;
}

impl GrfReadable for GrfFile {
    fn get(&self, p: &str) -> Option<Vec<u8>> {
        self.get_file(p)
    }
}

pub(crate) fn normalize_path(p: &str) -> String {
    p.replace('/', "\\")
}

pub(crate) fn effective_entries(assets: &[PhysicalAsset]) -> HashSet<(usize, usize)> {
    let mut winners = BTreeMap::<String, (usize, usize)>::new();
    for asset in assets {
        let key = normalize_path(&asset.entry.filename).to_ascii_lowercase();
        let candidate = (asset.source_index, asset.entry_index);
        winners
            .entry(key)
            .and_modify(|winner| {
                if candidate.0 < winner.0 || candidate.0 == winner.0 && candidate.1 > winner.1 {
                    *winner = candidate;
                }
            })
            .or_insert(candidate);
    }
    winners.into_values().collect()
}

pub(crate) fn first_hit(sources: &[impl GrfReadable], logical: &str) -> Option<Vec<u8>> {
    let normalized = normalize_path(logical);
    sources.iter().find_map(|s| s.get(&normalized))
}

/// Logical-path read access to the game content. `GrfVfs` is the only
/// production implementation; converters generic over it are unit-testable
/// against an in-memory source instead of the retail GRFs.
pub trait AssetRead {
    fn read_asset(&self, logical_path: &str) -> Option<Vec<u8>>;
}

struct GrfSource {
    archive_path: PathBuf,
    priority: u32,
    grf: GrfFile,
}

impl GrfReadable for GrfSource {
    fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.grf.get_file(path)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PhysicalAsset {
    pub source_index: usize,
    pub archive_path: PathBuf,
    pub priority: u32,
    pub entry_index: usize,
    pub entry: ArchiveEntry,
}

pub struct GrfVfs {
    sources: Vec<GrfSource>,
}

impl AssetRead for GrfVfs {
    fn read_asset(&self, logical_path: &str) -> Option<Vec<u8>> {
        self.read(logical_path)
    }
}

impl GrfVfs {
    pub fn open(grfs: &[&ConfigGrfEntry]) -> anyhow::Result<Self> {
        let mut sources = Vec::with_capacity(grfs.len());
        for entry in grfs {
            let grf_path = Path::new(&entry.path);
            let candidates = [grf_path.to_path_buf(), Path::new("assets").join(grf_path)];
            let resolved = candidates
                .iter()
                .find(|p| p.exists())
                .with_context(|| format!("GRF not found: {}", entry.path))?
                .clone();
            let grf = GrfFile::from_path(resolved.clone())
                .with_context(|| format!("Failed to open GRF: {}", resolved.display()))?;
            sources.push(GrfSource {
                archive_path: resolved,
                priority: entry.priority,
                grf,
            });
        }
        Ok(Self { sources })
    }

    pub fn read(&self, logical_path: &str) -> Option<Vec<u8>> {
        first_hit(&self.sources, logical_path)
    }

    pub(crate) fn physical_assets(&self) -> impl Iterator<Item = PhysicalAsset> + '_ {
        self.sources
            .iter()
            .enumerate()
            .flat_map(|(source_index, source)| {
                source
                    .grf
                    .entries
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(move |(entry_index, entry)| PhysicalAsset {
                        source_index,
                        archive_path: source.archive_path.clone(),
                        priority: source.priority,
                        entry_index,
                        entry,
                    })
            })
    }

    pub(crate) fn read_physical(&self, asset: &PhysicalAsset) -> Option<Vec<u8>> {
        self.sources
            .get(asset.source_index)?
            .grf
            .get_entry(asset.entry_index)
    }

    pub(crate) fn visit_physical<'a>(
        &'a self,
        assets: impl IntoIterator<Item = &'a PhysicalAsset>,
        mut visit: impl FnMut(&'a PhysicalAsset, Option<Vec<u8>>),
    ) {
        let mut current_source = None;
        let mut reader = None;
        for asset in assets {
            if current_source != Some(asset.source_index) {
                current_source = Some(asset.source_index);
                reader = self
                    .sources
                    .get(asset.source_index)
                    .and_then(|source| source.grf.entry_reader());
            }
            let bytes = reader
                .as_mut()
                .and_then(|reader| reader.get_entry(asset.entry_index));
            visit(asset, bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeGrf(HashMap<String, Vec<u8>>);

    impl GrfReadable for FakeGrf {
        fn get(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
    }

    fn fake(entries: &[(&str, &[u8])]) -> FakeGrf {
        FakeGrf(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_vec()))
                .collect(),
        )
    }

    #[test]
    fn physical_entries_keep_archive_identity_and_exact_bytes() {
        const GRF: &[u8] = &[
            77, 97, 115, 116, 101, 114, 32, 111, 102, 32, 77, 97, 103, 105, 99, 1, 2, 3, 4, 5, 6,
            7, 8, 9, 10, 11, 12, 13, 14, 0, 16, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 3, 0, 0, 120,
            156, 43, 200, 168, 44, 206, 76, 78, 204, 1, 0, 15, 112, 3, 94, 0, 0, 0, 0, 37, 0, 0, 0,
            40, 0, 0, 0, 120, 156, 75, 73, 44, 73, 140, 201, 205, 79, 73, 205, 137, 201, 207, 75,
            213, 43, 42, 206, 101, 16, 96, 96, 0, 99, 14, 32, 102, 100, 128, 2, 0, 228, 53, 7, 79,
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("physical.grf");
        std::fs::write(&path, GRF).unwrap();
        let config = ConfigGrfEntry {
            path: path.to_string_lossy().into_owned(),
            priority: 7,
        };
        let vfs = GrfVfs::open(&[&config]).unwrap();

        let asset = vfs.physical_assets().next().unwrap();
        assert_eq!(asset.source_index, 0);
        assert_eq!(asset.archive_path, path);
        assert_eq!(asset.priority, 7);
        assert_eq!(asset.entry_index, 0);
        assert_eq!(asset.entry.filename, "data\\model\\one.rsm");
        assert_eq!(
            vfs.read_physical(&asset).as_deref(),
            Some(b"physical" as &[u8])
        );
    }

    #[test]
    fn normalize_path_replaces_slashes() {
        assert_eq!(
            normalize_path("data/luafiles514/lua files/x.lub"),
            "data\\luafiles514\\lua files\\x.lub"
        );
    }

    #[test]
    fn normalize_path_already_backslash_unchanged() {
        assert_eq!(normalize_path("data\\foo.txt"), "data\\foo.txt");
    }

    #[test]
    fn first_hit_returns_higher_priority_source() {
        let a = fake(&[("data\\shared.txt", b"from_a")]);
        let b = fake(&[("data\\shared.txt", b"from_b")]);
        let result = first_hit(&[a, b], "data/shared.txt");
        assert_eq!(result.as_deref(), Some(b"from_a" as &[u8]));
    }

    #[test]
    fn first_hit_falls_through_to_later_source() {
        let a = fake(&[]);
        let b = fake(&[("data\\only_in_b.txt", b"found")]);
        let result = first_hit(&[a, b], "data/only_in_b.txt");
        assert_eq!(result.as_deref(), Some(b"found" as &[u8]));
    }

    #[test]
    fn first_hit_returns_none_when_not_found() {
        let a = fake(&[]);
        let result = first_hit(&[a], "data/missing.txt");
        assert!(result.is_none());
    }
}
