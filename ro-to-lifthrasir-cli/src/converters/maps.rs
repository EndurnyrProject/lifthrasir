use crate::converters::map;
use crate::grf_vfs::{GrfVfs, PhysicalAsset, effective_entries, normalize_path};
use std::collections::BTreeSet;
use std::path::Path;

pub fn run(
    vfs: &GrfVfs,
    out_dir: &Path,
    models_dir: &Path,
    force_models: bool,
) -> anyhow::Result<()> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let mut converted = 0;
    let mut failed = 0;

    for map_name in map_stems(&assets) {
        match map::run(vfs, &map_name, out_dir, models_dir, force_models) {
            Ok(()) => converted += 1,
            Err(error) => {
                failed += 1;
                eprintln!("failed to convert map '{map_name}': {error:#}");
            }
        }
    }

    println!("maps: {converted} converted, {failed} failed");
    anyhow::ensure!(failed == 0, "map conversion failed for {failed} maps");
    Ok(())
}

fn map_stems(assets: &[PhysicalAsset]) -> Vec<String> {
    let effective = effective_entries(assets);
    assets
        .iter()
        .filter(|asset| effective.contains(&(asset.source_index, asset.entry_index)))
        .filter_map(|asset| map_stem(&asset.entry.filename))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn map_stem(path: &str) -> Option<String> {
    let path = normalize_path(path).to_ascii_lowercase();
    let stem = path.strip_prefix("data\\")?.strip_suffix(".rsw")?;
    (!stem.is_empty() && !stem.contains('\\')).then_some(stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grf_vfs::PhysicalAsset;
    use ro_formats::grf::GrfEntry;
    use std::path::PathBuf;

    fn asset(path: &str, source_index: usize, entry_index: usize) -> PhysicalAsset {
        PhysicalAsset {
            source_index,
            archive_path: PathBuf::from(format!("archive-{source_index}.grf")),
            priority: source_index as u32,
            entry_index,
            entry: GrfEntry {
                filename: path.to_string(),
                pack_size: 0,
                length_aligned: 0,
                real_size: 0,
                file_type: 1,
                offset: 0,
            },
        }
    }

    #[test]
    fn map_stems_use_effective_rsw_entries_once_in_sorted_order() {
        let assets = vec![
            asset("data\\prontera.rsw", 0, 1),
            asset("data\\zeta.rsw", 0, 2),
            asset("data\\prontera.rsw", 1, 3),
            asset("data\\alpha.rsw", 1, 4),
            asset("data\\model\\not_a_map.rsw", 0, 5),
        ];

        let effective = effective_entries(&assets);

        assert!(effective.contains(&(0, 1)));
        assert!(!effective.contains(&(1, 3)));
        assert_eq!(map_stems(&assets), vec!["alpha", "prontera", "zeta"]);
    }
}
