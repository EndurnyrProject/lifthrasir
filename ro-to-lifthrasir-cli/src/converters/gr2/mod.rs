//! Offline conversion of RO GR2 actors and their action clips.

pub(crate) mod animation;

use crate::grf_vfs::{GrfVfs, effective_entries};

/// Effective GR2 archive entries, with normalized paths and deterministic ordering.
pub(crate) fn source_paths(vfs: &GrfVfs) -> Vec<String> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let winners = effective_entries(&assets);
    let mut paths: Vec<_> = assets
        .iter()
        .filter(|a| winners.contains(&(a.source_index, a.entry_index)))
        .map(|a| a.entry.filename.replace('\\', "/").to_lowercase())
        .filter(|p| p.ends_with(".gr2"))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

pub(crate) fn source_models(vfs: &GrfVfs) -> Vec<String> {
    source_paths(vfs)
        .into_iter()
        .filter_map(|path| {
            path.strip_prefix("data/model/3dmob/")
                .filter(|name| !name.contains('/'))
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
mod tests;
