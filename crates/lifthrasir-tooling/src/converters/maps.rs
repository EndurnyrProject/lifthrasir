use crate::converters::map;
use crate::converters::model::{ModelCache, TexturePool};
use crate::grf_vfs::{GrfVfs, PhysicalAsset, effective_entries, normalize_path};
use crate::{BatchFailure, Diagnostic, DiagnosticLevel, OperationReport, Progress};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A map needs all three of these to be convertible.
const REQUIRED_EXTENSIONS: [&str; 3] = ["rsw", "gnd", "gat"];

pub fn run(
    vfs: &GrfVfs,
    out_dir: &Path,
    models_dir: &Path,
    ground_dir: &Path,
    force_models: bool,
    progress: &(dyn Fn(Progress<'_>) + Sync),
) -> anyhow::Result<OperationReport> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let inventory = map_inventory(&assets);

    // Everything a long run has to say goes to the host sink as it happens: a
    // collected batch stays silent until the end, which looks hung.
    let mut report = OperationReport::default();
    let total = inventory.convertible.len();
    let skipped: Vec<_> = inventory
        .incomplete
        .iter()
        .map(|(map_name, missing)| Diagnostic {
            level: DiagnosticLevel::Info,
            message: format!(
                "skipping map '{map_name}': no {} in the GRFs",
                missing.join(", ")
            ),
        })
        .collect();
    progress(Progress {
        done: 0,
        total,
        diagnostics: &skipped,
    });

    // One texture pool and one model cache for the whole run: a prop shared
    // by many maps converts once, and parallel maps never step on each other.
    let pool = TexturePool::new(models_dir);
    let ground_pool = TexturePool::new(ground_dir);
    let cache = ModelCache::default();
    let done = AtomicUsize::new(0);
    let results: Vec<_> = inventory
        .convertible
        .par_iter()
        .map(|map_name| {
            let result = map::run_shared(
                vfs,
                map_name,
                out_dir,
                models_dir,
                force_models,
                &pool,
                &ground_pool,
                &cache,
            );
            progress(Progress {
                done: done.fetch_add(1, Ordering::Relaxed) + 1,
                total,
                diagnostics: reported(&result),
            });
            result
        })
        .collect();
    let mut failures = Vec::new();
    let mut converted = 0;
    for result in results {
        match result {
            Ok(converted_map) => {
                converted += 1;
                retain_warnings(&mut report, converted_map.diagnostics);
                report.outputs.extend(converted_map.outputs);
            }
            Err(error) => match error.downcast::<BatchFailure>() {
                Ok(batch) => {
                    retain_warnings(&mut report, batch.report.diagnostics);
                    failures.extend(batch.failures);
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    report.warning(message.clone());
                    failures.push(message);
                }
            },
        }
    }
    report.diagnostics.extend(pool.diagnostics());
    report.diagnostics.extend(ground_pool.diagnostics());
    report.info(format!(
        "maps: {converted} converted, {} failed, {} skipped",
        failures.len(),
        inventory.incomplete.len()
    ));
    report.finish(failures)
}

/// What one map reported, which a partial failure keeps alongside its errors.
fn reported(result: &anyhow::Result<OperationReport>) -> &[Diagnostic] {
    match result {
        Ok(report) => &report.diagnostics,
        Err(error) => match error.downcast_ref::<BatchFailure>() {
            Some(batch) => &batch.report.diagnostics,
            None => &[],
        },
    }
}

/// Per-map info lines are progress and reach the host through its sink; only
/// warnings stay in the batch report, which a host renders once the run ends.
fn retain_warnings(report: &mut OperationReport, diagnostics: Vec<Diagnostic>) {
    report.diagnostics.extend(
        diagnostics
            .into_iter()
            .filter(|d| d.level == DiagnosticLevel::Warning),
    );
}

/// Map names split by whether every source a conversion needs is present.
///
/// The archives carry a handful of orphan entries - a `.rsw` with no matching
/// `.gnd`, for instance - which are not maps and can never be converted. They
/// are reported rather than counted as failures, so a clean run stays clean.
struct MapInventory {
    convertible: Vec<String>,
    incomplete: Vec<(String, Vec<&'static str>)>,
}

fn map_inventory(assets: &[PhysicalAsset]) -> MapInventory {
    let effective = effective_entries(assets);
    let mut present: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();

    for asset in assets
        .iter()
        .filter(|asset| effective.contains(&(asset.source_index, asset.entry_index)))
    {
        if let Some((stem, extension)) = map_source(&asset.entry.filename) {
            present.entry(stem).or_default().insert(extension);
        }
    }

    let (convertible, incomplete) = present
        .into_iter()
        // Only a `.rsw` names a map; a stray `.gat` on its own does not.
        .filter(|(_, found)| found.contains("rsw"))
        .map(|(stem, found)| {
            let missing: Vec<_> = REQUIRED_EXTENSIONS
                .into_iter()
                .filter(|extension| !found.contains(extension))
                .collect();
            (stem, missing)
        })
        .partition::<Vec<_>, _>(|(_, missing)| missing.is_empty());

    MapInventory {
        convertible: convertible.into_iter().map(|(stem, _)| stem).collect(),
        incomplete,
    }
}

/// Split `data\<name>.<ext>` into its map name and extension.
fn map_source(path: &str) -> Option<(String, &'static str)> {
    let path = normalize_path(path).to_ascii_lowercase();
    let rest = path.strip_prefix("data\\")?;
    let extension = REQUIRED_EXTENSIONS
        .into_iter()
        .find(|extension| rest.ends_with(&format!(".{extension}")))?;
    let stem = rest.strip_suffix(&format!(".{extension}"))?;

    (!stem.is_empty() && !stem.contains('\\')).then(|| (stem.to_string(), extension))
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

    /// Every source a map needs, for the given name.
    fn complete(name: &str, source_index: usize, first_entry: usize) -> Vec<PhysicalAsset> {
        REQUIRED_EXTENSIONS
            .into_iter()
            .enumerate()
            .map(|(offset, extension)| {
                asset(
                    &format!("data\\{name}.{extension}"),
                    source_index,
                    first_entry + offset,
                )
            })
            .collect()
    }

    #[test]
    fn map_stems_use_effective_entries_once_in_sorted_order() {
        let mut assets = complete("prontera", 0, 1);
        assets.extend(complete("zeta", 0, 10));
        assets.extend(complete("prontera", 1, 20)); // shadowed by the higher tier
        assets.extend(complete("alpha", 1, 30));
        assets.push(asset("data\\model\\not_a_map.rsw", 0, 40));

        let effective = effective_entries(&assets);
        assert!(effective.contains(&(0, 1)));
        assert!(!effective.contains(&(1, 20)));

        let inventory = map_inventory(&assets);
        assert_eq!(inventory.convertible, vec!["alpha", "prontera", "zeta"]);
        assert!(inventory.incomplete.is_empty());
    }

    /// `1@ch1b` and `1@ch2b` really are shipped this way: a `.rsw` and a `.gat`
    /// but no `.gnd` anywhere in the archives.
    #[test]
    fn a_map_without_every_source_is_reported_rather_than_converted() {
        let mut assets = complete("prontera", 0, 1);
        assets.push(asset("data\\1@ch1b.rsw", 0, 10));
        assets.push(asset("data\\1@ch1b.gat", 0, 11));

        let inventory = map_inventory(&assets);

        assert_eq!(inventory.convertible, vec!["prontera"]);
        assert_eq!(
            inventory.incomplete,
            vec![("1@ch1b".to_string(), vec!["gnd"])]
        );
    }

    /// A `.gat` with no `.rsw` beside it does not name a map at all.
    #[test]
    fn a_stray_non_rsw_source_is_not_treated_as_a_map() {
        let assets = vec![asset("data\\orphan.gat", 0, 1)];

        let inventory = map_inventory(&assets);

        assert!(inventory.convertible.is_empty());
        assert!(inventory.incomplete.is_empty());
    }
}
