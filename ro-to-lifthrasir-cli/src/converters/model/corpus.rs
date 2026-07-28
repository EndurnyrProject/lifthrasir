use super::{ModelFormat, classify_header};
use crate::grf_vfs::{GrfVfs, PhysicalAsset, normalize_path};
use ro_formats::{RoWorld, RswObject};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPreflightOutcome {
    Ready,
    ObservedNoShade,
    UnsupportedVersion,
    MalformedHeader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelInventoryRow {
    pub archive: String,
    pub priority: u32,
    pub source_index: usize,
    pub entry_index: usize,
    pub logical_path: String,
    pub extension: String,
    pub source_hash: Option<String>,
    pub header_major: Option<u8>,
    pub header_minor: Option<u8>,
    pub shade_type: Option<i32>,
    pub effective: bool,
    pub extension_mismatch: bool,
    pub outcome: ModelPreflightOutcome,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlacementInventoryRow {
    pub rsw_path: String,
    pub model_path: String,
    pub anim_type: u32,
    pub anim_speed: f32,
    pub gate_negative_speed: bool,
}

fn inspect_placements(rsw_path: &str, world: &RoWorld) -> Vec<PlacementInventoryRow> {
    world
        .objects
        .iter()
        .filter_map(|object| match object {
            RswObject::Model(model) => Some(PlacementInventoryRow {
                rsw_path: rsw_path.replace('\\', "/"),
                model_path: model.filename.replace('\\', "/"),
                anim_type: model.anim_type,
                anim_speed: model.anim_speed,
                gate_negative_speed: model.anim_speed.is_sign_negative(),
            }),
            _ => None,
        })
        .collect()
}

fn effective_entries(assets: &[PhysicalAsset]) -> HashSet<(usize, usize)> {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InventoryError {
    pub logical_path: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PreflightSummary {
    pub physical_models: usize,
    pub effective_models: usize,
    pub placements: usize,
    pub no_shade_models: usize,
    pub negative_speed_placements: usize,
    pub inventory_errors: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreflightReport {
    pub models: Vec<ModelInventoryRow>,
    pub placements: Vec<PlacementInventoryRow>,
    pub errors: Vec<InventoryError>,
    pub summary: PreflightSummary,
}

impl PreflightReport {
    pub fn has_gates(&self) -> bool {
        self.summary.negative_speed_placements > 0 || self.summary.inventory_errors > 0
    }

    pub fn gate_message(&self) -> String {
        format!(
            "{} observed RSM2 no-shade models, {} negative-speed placements, {} inventory errors",
            self.summary.no_shade_models,
            self.summary.negative_speed_placements,
            self.summary.inventory_errors
        )
    }

    pub fn blocking_paths(&self, limit: usize) -> Vec<&str> {
        self.placements
            .iter()
            .filter(|row| row.gate_negative_speed)
            .map(|row| row.model_path.as_str())
            .chain(self.errors.iter().map(|row| row.logical_path.as_str()))
            .take(limit)
            .collect()
    }
}

pub fn extract(vfs: &GrfVfs, extracted_root: &Path) -> anyhow::Result<usize> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let effective = effective_entries(&assets);
    let mut relevant: Vec<_> = assets
        .iter()
        .filter(|asset| is_relevant(asset, &effective))
        .collect();
    relevant.sort_by_key(|asset| (asset.source_index, asset.entry.offset));

    if extracted_root.exists() {
        std::fs::remove_dir_all(extracted_root)?;
    }
    std::fs::create_dir_all(extracted_root)?;
    let mut errors = Vec::new();
    extract_all(vfs, &relevant, extracted_root, &mut errors);
    anyhow::ensure!(
        errors.is_empty(),
        "{} corpus extraction failures; first: {}: {}",
        errors.len(),
        errors[0].logical_path,
        errors[0].error
    );
    std::fs::write(extracted_root.join(".complete"), extraction_id(&relevant)?)?;
    Ok(relevant.len())
}

pub fn inventory(vfs: &GrfVfs, extracted_root: &Path) -> anyhow::Result<PreflightReport> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let effective = effective_entries(&assets);
    let mut model_assets: Vec<_> = assets
        .iter()
        .filter(|asset| has_extension(asset, &["rsm", "rsm2"]))
        .collect();
    let mut rsw_assets: Vec<_> = assets
        .iter()
        .filter(|asset| has_extension(asset, &["rsw"]) && is_relevant(asset, &effective))
        .collect();
    let mut errors = Vec::new();

    let mut relevant = Vec::with_capacity(model_assets.len() + rsw_assets.len());
    relevant.extend(model_assets.iter().copied());
    relevant.extend(rsw_assets.iter().copied());
    relevant.sort_by_key(|asset| (asset.source_index, asset.entry.offset));
    validate_extraction(&relevant, extracted_root)?;
    eprintln!(
        "corpus: scanning {} extracted models and {} effective worlds",
        model_assets.len(),
        rsw_assets.len()
    );

    model_assets.sort_by_key(|asset| (asset.source_index, asset.entry_index));
    let headers = read_headers(&model_assets, extracted_root);
    let mut models = Vec::with_capacity(model_assets.len());
    for (asset, bytes) in model_assets.into_iter().zip(headers) {
        let is_effective = effective.contains(&(asset.source_index, asset.entry_index));
        match bytes {
            Some(bytes) => {
                let row = inspect_model(asset, bytes, is_effective);
                record_model(row, &mut models, &mut errors);
            }
            None => {
                let logical_path = display_path(&asset.entry.filename);
                models.push(unreadable_model(asset, is_effective));
                errors.push(InventoryError {
                    logical_path,
                    error: "could not read extracted model header".to_string(),
                });
            }
        }
    }

    rsw_assets.sort_by_key(|asset| (asset.source_index, asset.entry_index));
    let (mut placements, placement_errors) = read_extracted_placements(&rsw_assets, extracted_root);
    errors.extend(placement_errors);

    placements.sort_by(|left, right| {
        left.rsw_path
            .cmp(&right.rsw_path)
            .then_with(|| left.model_path.cmp(&right.model_path))
            .then_with(|| left.anim_type.cmp(&right.anim_type))
            .then_with(|| left.anim_speed.total_cmp(&right.anim_speed))
    });
    errors.sort_by(|left, right| {
        left.logical_path
            .cmp(&right.logical_path)
            .then_with(|| left.error.cmp(&right.error))
    });

    let summary = PreflightSummary {
        physical_models: models.len(),
        effective_models: models.iter().filter(|row| row.effective).count(),
        placements: placements.len(),
        no_shade_models: models
            .iter()
            .filter(|row| row.outcome == ModelPreflightOutcome::ObservedNoShade)
            .count(),
        negative_speed_placements: placements
            .iter()
            .filter(|row| row.gate_negative_speed)
            .count(),
        inventory_errors: errors.len(),
    };
    Ok(PreflightReport {
        models,
        placements,
        errors,
        summary,
    })
}

pub fn write_preflight(
    vfs: &GrfVfs,
    output: &Path,
    extracted_root: &Path,
) -> anyhow::Result<PreflightReport> {
    let report_dir = output.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(report_dir)?;
    let report = inventory(vfs, extracted_root)?;
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    std::fs::write(output, json)?;
    Ok(report)
}

fn extract_all(
    vfs: &GrfVfs,
    assets: &[&PhysicalAsset],
    root: &Path,
    errors: &mut Vec<InventoryError>,
) {
    eprintln!("corpus: extracting {} files from GRFs", assets.len());
    let total = assets.len();
    let mut extracted = 0;
    vfs.visit_physical(assets.iter().copied(), |asset, bytes| {
        let path = extracted_path(root, asset);
        let result = bytes
            .ok_or_else(|| std::io::Error::other("could not read physical GRF entry"))
            .and_then(|bytes| {
                std::fs::create_dir_all(path.parent().expect("extracted file parent"))?;
                std::fs::write(&path, bytes)
            });
        if let Err(error) = result {
            errors.push(InventoryError {
                logical_path: display_path(&asset.entry.filename),
                error: format!("extracting physical GRF entry: {error}"),
            });
        }
        extracted += 1;
        if extracted % 1_000 == 0 || extracted == total {
            eprintln!("corpus: extracted {extracted}/{total}");
        }
    });
}

fn validate_extraction(assets: &[&PhysicalAsset], root: &Path) -> anyhow::Result<()> {
    let expected = extraction_id(assets)?;
    let actual = std::fs::read_to_string(root.join(".complete"))?;
    anyhow::ensure!(
        actual == expected,
        "extracted corpus does not match the configured GRFs; run model-corpus extract"
    );
    Ok(())
}

fn extraction_id(assets: &[&PhysicalAsset]) -> anyhow::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut sources = HashSet::new();
    for asset in assets {
        if sources.insert(asset.source_index) {
            let metadata = std::fs::metadata(&asset.archive_path)?;
            let modified = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?;
            hasher.update(&asset.source_index.to_le_bytes());
            hasher.update(&asset.priority.to_le_bytes());
            hasher.update(asset.archive_path.to_string_lossy().as_bytes());
            hasher.update(&metadata.len().to_le_bytes());
            hasher.update(&modified.as_nanos().to_le_bytes());
        }
        hasher.update(&asset.entry_index.to_le_bytes());
        hasher.update(asset.entry.filename.as_bytes());
        hasher.update(&asset.entry.offset.to_le_bytes());
        hasher.update(&asset.entry.pack_size.to_le_bytes());
        hasher.update(&asset.entry.real_size.to_le_bytes());
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn extracted_path(root: &Path, asset: &PhysicalAsset) -> PathBuf {
    let archive = asset
        .archive_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("archive");
    let extension = asset
        .entry
        .filename
        .rsplit_once('.')
        .map_or("bin", |(_, extension)| extension);
    root.join(format!(
        "{}-{}-{}",
        asset.source_index, asset.priority, archive
    ))
    .join(format!(
        "{}-{}-{}-{}.{}",
        asset.entry_index,
        asset.entry.offset,
        asset.entry.pack_size,
        asset.entry.real_size,
        extension
    ))
}

fn read_extracted_placements(
    assets: &[&PhysicalAsset],
    root: &Path,
) -> (Vec<PlacementInventoryRow>, Vec<InventoryError>) {
    if assets.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let chunk_size = assets.len().div_ceil(8.min(assets.len()));
    let rows = std::thread::scope(|scope| {
        let handles: Vec<_> = assets
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|asset| {
                            let logical_path = display_path(&asset.entry.filename);
                            let bytes =
                                std::fs::read(extracted_path(root, asset)).map_err(|error| {
                                    InventoryError {
                                        logical_path: logical_path.clone(),
                                        error: format!("reading extracted RSW: {error}"),
                                    }
                                })?;
                            let world =
                                RoWorld::from_bytes(&bytes).map_err(|error| InventoryError {
                                    logical_path: logical_path.clone(),
                                    error: error.to_string(),
                                })?;
                            Ok(inspect_placements(&logical_path, &world))
                        })
                        .collect::<Vec<Result<Vec<_>, InventoryError>>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("extracted RSW reader panicked"))
            .collect::<Vec<_>>()
    });

    let mut placements = Vec::new();
    let mut errors = Vec::new();
    for row in rows {
        match row {
            Ok(mut row) => placements.append(&mut row),
            Err(error) => errors.push(error),
        }
    }
    (placements, errors)
}

fn read_headers(assets: &[&PhysicalAsset], root: &Path) -> Vec<Option<Vec<u8>>> {
    if assets.is_empty() {
        return Vec::new();
    }
    let chunk_size = assets.len().div_ceil(8.min(assets.len()));
    std::thread::scope(|scope| {
        let handles: Vec<_> = assets
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|asset| {
                            let file = std::fs::File::open(extracted_path(root, asset)).ok()?;
                            let mut header = Vec::with_capacity(14);
                            file.take(14).read_to_end(&mut header).ok()?;
                            Some(header)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("extracted model reader panicked"))
            .collect()
    })
}

fn is_relevant(asset: &PhysicalAsset, effective: &HashSet<(usize, usize)>) -> bool {
    has_extension(asset, &["rsm", "rsm2"])
        || has_extension(asset, &["rsw"])
            && effective.contains(&(asset.source_index, asset.entry_index))
}

fn has_extension(asset: &PhysicalAsset, extensions: &[&str]) -> bool {
    asset
        .entry
        .filename
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extensions.contains(&extension.to_ascii_lowercase().as_str()))
}

fn display_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn record_model(
    row: ModelInventoryRow,
    models: &mut Vec<ModelInventoryRow>,
    errors: &mut Vec<InventoryError>,
) {
    if row.outcome == ModelPreflightOutcome::MalformedHeader {
        errors.push(InventoryError {
            logical_path: row.logical_path.clone(),
            error: row
                .error
                .clone()
                .unwrap_or_else(|| "malformed RSM header".to_string()),
        });
    }
    models.push(row);
}

fn unreadable_model(asset: &PhysicalAsset, effective: bool) -> ModelInventoryRow {
    let extension = asset
        .entry
        .filename
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension)
        .to_ascii_lowercase();
    ModelInventoryRow {
        archive: asset.archive_path.to_string_lossy().replace('\\', "/"),
        priority: asset.priority,
        source_index: asset.source_index,
        entry_index: asset.entry_index,
        logical_path: display_path(&asset.entry.filename),
        extension,
        source_hash: None,
        header_major: None,
        header_minor: None,
        shade_type: None,
        effective,
        extension_mismatch: false,
        outcome: ModelPreflightOutcome::MalformedHeader,
        error: Some("could not read physical GRF entry".to_string()),
    }
}

fn inspect_model(asset: &PhysicalAsset, bytes: Vec<u8>, effective: bool) -> ModelInventoryRow {
    let extension = asset
        .entry
        .filename
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension)
        .to_ascii_lowercase();
    let classified = classify_header(&bytes);
    let shade_type = bytes
        .get(10..14)
        .map(|raw| i32::from_le_bytes(raw.try_into().expect("four bytes")));
    let (header_major, header_minor, extension_mismatch, outcome, error) = match classified {
        Ok(format) => {
            let (major, minor) = format.version();
            let mismatch = major == 1 && extension != "rsm" || major == 2 && extension != "rsm2";
            let outcome = match (major, format, shade_type) {
                (_, _, None) => ModelPreflightOutcome::MalformedHeader,
                (2, _, Some(0)) => ModelPreflightOutcome::ObservedNoShade,
                (_, ModelFormat::Unsupported { .. }, _) => {
                    ModelPreflightOutcome::UnsupportedVersion
                }
                _ => ModelPreflightOutcome::Ready,
            };
            let error = (shade_type.is_none())
                .then(|| "file is shorter than the RSM shade header".to_string());
            (Some(major), Some(minor), mismatch, outcome, error)
        }
        Err(error) => (
            None,
            None,
            false,
            ModelPreflightOutcome::MalformedHeader,
            Some(error.to_string()),
        ),
    };

    ModelInventoryRow {
        archive: asset.archive_path.to_string_lossy().replace('\\', "/"),
        priority: asset.priority,
        source_index: asset.source_index,
        entry_index: asset.entry_index,
        logical_path: asset.entry.filename.replace('\\', "/"),
        extension,
        source_hash: None,
        header_major,
        header_minor,
        shade_type,
        effective,
        extension_mismatch,
        outcome,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn placement_rows_report_negative_model_speed() {
        let mut world = RoWorld {
            version: "1.9".to_string(),
            ini_file: String::new(),
            gnd_file: String::new(),
            gat_file: String::new(),
            src_file: None,
            water: ro_formats::RswWater::default(),
            light: ro_formats::RswLight::default(),
            ground: ro_formats::RswGround::default(),
            objects: Vec::new(),
        };
        world.objects.push(RswObject::Model(ro_formats::RswModel {
            name: "tree".to_string(),
            anim_type: 2,
            anim_speed: -1.5,
            block_type: 0,
            filename: "tree.rsm".to_string(),
            node_name: String::new(),
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }));

        let rows = inspect_placements("data\\prontera.rsw", &world);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].model_path, "tree.rsm");
        assert!(rows[0].gate_negative_speed);
    }

    #[test]
    fn effective_entries_use_overlay_order_and_last_duplicate_in_one_archive() {
        let assets = vec![
            asset("DATA\\MODEL\\tree.rsm", 0, 2),
            asset("data/model/tree.rsm", 0, 5),
            asset("data\\model\\tree.rsm", 1, 9),
            asset("data\\model\\rock.rsm", 1, 3),
        ];

        let effective = effective_entries(&assets);

        assert_eq!(effective.len(), 2);
        assert!(effective.contains(&(0, 5)));
        assert!(effective.contains(&(1, 3)));
    }

    #[test]
    fn shadowed_worlds_are_not_relevant_but_all_physical_models_are() {
        let assets = vec![
            asset("data\\map.rsw", 0, 1),
            asset("data\\map.rsw", 1, 2),
            asset("data\\model\\tree.rsm", 0, 3),
            asset("data\\model\\tree.rsm", 1, 4),
        ];
        let effective = effective_entries(&assets);
        let relevant: Vec<_> = assets
            .iter()
            .filter(|asset| is_relevant(asset, &effective))
            .map(|asset| (asset.source_index, asset.entry_index))
            .collect();

        assert_eq!(relevant, vec![(0, 1), (0, 3), (1, 4)]);
    }

    #[test]
    fn inventory_errors_are_preflight_gates() {
        let report = PreflightReport {
            models: Vec::new(),
            placements: Vec::new(),
            errors: vec![InventoryError {
                logical_path: "data/map.rsw".to_string(),
                error: "broken".to_string(),
            }],
            summary: PreflightSummary {
                inventory_errors: 1,
                ..Default::default()
            },
        };

        assert!(report.has_gates());
        assert_eq!(report.blocking_paths(10), vec!["data/map.rsw"]);
    }

    #[test]
    fn unsupported_rsm2_still_uses_header_family_for_gates_and_mismatches() {
        let mut bytes = b"GRSM\x02\x04".to_vec();
        bytes.extend_from_slice(&100_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        let row = inspect_model(&asset("data\\model\\future.rsm", 0, 0), bytes, true);
        let truncated = inspect_model(
            &asset("data\\model\\truncated.rsm2", 0, 2),
            b"GRSM\x02\x04".to_vec(),
            true,
        );

        let mut rsm1 = b"GRSM\x01\x04".to_vec();
        rsm1.extend_from_slice(&100_i32.to_le_bytes());
        rsm1.extend_from_slice(&2_i32.to_le_bytes());
        let rsm1 = inspect_model(&asset("data\\model\\old.rsm2", 0, 1), rsm1, true);

        assert!(row.extension_mismatch);
        assert_eq!(row.outcome, ModelPreflightOutcome::ObservedNoShade);
        assert_eq!(truncated.outcome, ModelPreflightOutcome::MalformedHeader);
        assert!(rsm1.extension_mismatch);
    }

    #[test]
    fn extraction_identity_changes_with_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("data.grf");
        std::fs::write(&archive, b"first").unwrap();
        let mut candidate = asset("data\\model\\tree.rsm", 0, 1);
        candidate.archive_path = archive.clone();
        let first = extraction_id(&[&candidate]).unwrap();
        std::fs::write(&archive, b"changed-size").unwrap();
        let second = extraction_id(&[&candidate]).unwrap();
        let extracted = dir.path().join("extracted");
        std::fs::create_dir(&extracted).unwrap();
        std::fs::write(extracted.join(".complete"), "stale").unwrap();

        assert_ne!(first, second);
        assert!(validate_extraction(&[&candidate], &extracted).is_err());
    }

    #[test]
    fn malformed_model_rows_become_inventory_errors() {
        let row = inspect_model(
            &asset("data\\model\\broken.rsm2", 0, 0),
            b"wrong!".to_vec(),
            true,
        );
        let mut models = Vec::new();
        let mut errors = Vec::new();

        record_model(row, &mut models, &mut errors);

        assert_eq!(models.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].logical_path, "data/model/broken.rsm2");
    }

    #[test]
    fn model_rows_keep_exact_header_and_observe_no_shade() {
        let mut bytes = b"GRSM\x02\x03".to_vec();
        bytes.extend_from_slice(&100_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        let row = inspect_model(&asset("data\\model\\tree.rsm", 1, 4), bytes, true);

        assert_eq!(row.header_major, Some(2));
        assert_eq!(row.header_minor, Some(3));
        assert_eq!(row.shade_type, Some(0));
        assert!(row.effective);
        assert!(row.extension_mismatch);
        assert_eq!(row.outcome, ModelPreflightOutcome::ObservedNoShade);
        assert!(row.source_hash.is_none());
    }
}
