use super::{ConvertOutcome, ModelFormat, TexturePool, classify_header, convert_model_bytes};
use crate::converters::gltf_out::hash_hex;
use crate::grf_vfs::{GrfVfs, PhysicalAsset, normalize_path};
use ro_formats::{RoWorld, Rsm2, Rsm2NodeTextures, Rsm2TextureChannelType, RswObject};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
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
    pub archive: String,
    pub priority: u32,
    pub source_index: usize,
    pub entry_index: usize,
    pub rsw_path: String,
    pub model_path: String,
    pub anim_type: u32,
    pub anim_speed: f32,
    pub gate_negative_speed: bool,
}

fn inspect_placements(
    asset: &PhysicalAsset,
    rsw_path: &str,
    world: &RoWorld,
) -> Vec<PlacementInventoryRow> {
    world
        .objects
        .iter()
        .filter_map(|object| match object {
            RswObject::Model(model) => Some(PlacementInventoryRow {
                archive: asset.archive_path.to_string_lossy().replace('\\', "/"),
                priority: asset.priority,
                source_index: asset.source_index,
                entry_index: asset.entry_index,
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
                            Ok(inspect_placements(asset, &logical_path, &world))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusOutcome {
    Converted,
    Skipped,
    UnsupportedLegacy,
    UnsupportedVersion,
    Malformed,
    Failed,
    Gated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PilotFeatures {
    pub roots: usize,
    pub nodes: usize,
    pub position_animation: bool,
    pub rotation_animation: bool,
    pub scale_animation: bool,
    pub uv_translate_u: bool,
    pub uv_translate_v: bool,
    pub uv_scale_u: bool,
    pub uv_scale_v: bool,
    pub uv_rotate: bool,
    pub two_sided_faces: bool,
    pub mixed_face_culling: bool,
    pub tga_texture: bool,
    pub animated_tga: bool,
    pub animated_non_tga: bool,
    pub multiple_roots: bool,
    pub no_shade: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorpusModelRow {
    #[serde(flatten)]
    pub inventory: ModelInventoryRow,
    pub scratch_path: String,
    #[serde(rename = "conversion_outcome")]
    pub outcome: CorpusOutcome,
    pub stage: Option<String>,
    pub context_error: Option<String>,
    pub known_malformed: bool,
    pub texture_fallbacks: Vec<String>,
    pub features: Option<PilotFeatures>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CorpusCounts {
    pub physical_models: usize,
    pub effective_models: usize,
    pub effective_worlds: usize,
    pub placements: usize,
    pub supported_rsm2: usize,
    pub well_formed_rsm2: usize,
    pub well_formed_no_shade: usize,
    pub converted: usize,
    pub skipped: usize,
    pub unsupported_legacy: usize,
    pub unsupported_version: usize,
    pub malformed: usize,
    pub failed: usize,
    pub gated: usize,
    pub shadowed: usize,
    pub extension_mismatches: usize,
    pub texture_fallback_models: usize,
    pub texture_fallbacks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArchiveCounts {
    pub archive: String,
    pub priority: u32,
    pub source_index: usize,
    #[serde(flatten)]
    pub counts: CorpusCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorpusReport {
    pub models: Vec<CorpusModelRow>,
    pub placements: Vec<PlacementInventoryRow>,
    pub inventory_errors: Vec<InventoryError>,
    pub archives: Vec<ArchiveCounts>,
    pub totals: CorpusCounts,
    pub blockers: Vec<String>,
}

impl CorpusReport {
    pub fn has_blockers(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub fn blocking_paths(&self, limit: usize) -> Vec<&str> {
        self.blockers
            .iter()
            .map(String::as_str)
            .take(limit)
            .collect()
    }
}

const KNOWN_MALFORMED_RSM2: [(&str, &str); 24] = [
    (
        "data/model/gevent/banner_s_02.rsm2",
        "92ea87671da14578cfd17b5d98668b5ddb71ef9f31b4ce875e0273c4d13c8b19",
    ),
    (
        "data/model/gevent/botmorocc_s_01.rsm2",
        "1d12b0dc5a25483a858e8c576a83c65a82913bcd29226fb72cfe3db60c5896de",
    ),
    (
        "data/model/gevent/paper_s_01.rsm2",
        "948d22516d91e0d3205d37edb38fb945f5132b497a330950c34fb3509411c5c0",
    ),
    (
        "data/model/gevent/plants_s_10.rsm2",
        "5724ccc3f28363f96402b86819dc6a0863b96bc1b2b61856523c9139af052dd0",
    ),
    (
        "data/model/gevent/poring_h_09.rsm2",
        "9455c2fce83d34297c2bb8312076031f78f3a9fcf9c76019f340adc4adbd4a63",
    ),
    (
        "data/model/graywolf/tree_e_07.rsm2",
        "d67289821a794f3e173fda09e37f05bd6246fb8ec00d27e0b768a14a6f143e44",
    ),
    (
        "data/model/herosria/bench_d_01.rsm2",
        "db6c513ca720f3d3ededacaf9c01cb30d0f60922338c57a00ed9915e8be85c44",
    ),
    (
        "data/model/herosria/bridge_d_01.rsm2",
        "5a43b3c36595be0527a4a7b20e3daae50a352d3d228ff000b2bfeec795c1dade",
    ),
    (
        "data/model/hyper/windowlib_s_05.rsm2",
        "91b4235312e75486151cc7f7053e8b05c3cc0b89a162f22c227eb7ae7d275ae1",
    ),
    (
        "data/model/ilusion/bench_y_01.rsm2",
        "5ea3fb1e961df1ec132147c6a27679086a2dd8056696452debdb8fc67f47bc63",
    ),
    (
        "data/model/ilusion/hammock_y_01.rsm2",
        "e2f7bf730c1da851935e4331ede36949ac10781302ff947f7a237e57fccf8bae",
    ),
    (
        "data/model/ilusion/labsample_y_01.rsm2",
        "d9d115d50dfa04fd6d57dece1f2d82c411b01c8b795a3f8295d974f9270470b5",
    ),
    (
        "data/model/ilusion/waterfall_s_02.rsm2",
        "761f3b85eb4b4f87cf48c476ba7d932f931050f9911bc1e0be6ba3ba376dcda4",
    ),
    (
        "data/model/issgard/clock_s_02.rsm2",
        "57bd4b11fa2e37e43a5d5db4212336c309e80e4411df8da6873e6eeb21445b40",
    ),
    (
        "data/model/issgard/tree_s_01.rsm2",
        "8e88505d69a399f59e10fed673c92848a310cb8baa421d844ad25d07ebc09978",
    ),
    (
        "data/model/job4for/chandelier_s_01.rsm2",
        "f61de5f70d3c0e5340b62ebae9242df972693168559f67caa7483f9e79f53b84",
    ),
    (
        "data/model/job4for/clock_s_01.rsm2",
        "611e19b88afbade69cd1a31b8138fc21cfee6bee15827407cad7bd6f0d826b45",
    ),
    (
        "data/model/job4for/incense_s_01.rsm2",
        "36472008645eb38d08f761e229e7a1d9d643baeacbe3c8f29c883a8d0ac27aae",
    ),
    (
        "data/model/job4for/light_s_01.rsm2",
        "1264ed17fd786c01dcda250f2156236cd1557d439fdf4fe5b44a3fc677cd8a70",
    ),
    (
        "data/model/job4for/xchair_j_03.rsm2",
        "9528ec7383c9cc3a5a45f7195565dcfd9b387f8ec4043e3c364784b17fe765b6",
    ),
    (
        "data/model/neomd/stone_s_02.rsm2",
        "6c63170de41f62266757c6c33634d594715684b34d30824cbccd14782aada404",
    ),
    (
        "data/model/prt/flowerbed_s_01.rsm2",
        "2b95404f94e1aaf15cc6e750eac2b07bc74200c57630877f13620f389ecea9c1",
    ),
    (
        "data/model/prt/house_s_01_1.rsm2",
        "4b8e8d415132096af5b78af28552c2038d0afb34430135443edfb90ae7642711",
    ),
    (
        "data/model/prt/motel_s_01.rsm2",
        "a82b04b4963df76475ba48d8ec562fcfd670f3916f9291eadd89c59c5796030a",
    ),
];

const KNOWN_RSM2_TEXTURE_FALLBACKS: [(&str, &str); 13] = [
    (
        "data/model/colosseum/bottom_s_01.rsm2",
        "colosseum\\colo_s_01.bmp",
    ),
    (
        "data/model/colosseum/bottom_s_01.rsm2",
        "colosseum\\colo_s_02.bmp",
    ),
    (
        "data/model/colosseum/colo_j_01.rsm2",
        "colosseum\\colo_j_01.bmp",
    ),
    (
        "data/model/colosseum/colo_j_01.rsm2",
        "colosseum\\colo_j_02.bmp",
    ),
    (
        "data/model/colosseum/colo_j_01.rsm2",
        "colosseum\\colo_j_03.bmp",
    ),
    (
        "data/model/colosseum/colo_j_01.rsm2",
        "colosseum\\colo_j_04.bmp",
    ),
    (
        "data/model/colosseum/colo_j_01.rsm2",
        "colosseum\\colo_j_05.bmp",
    ),
    (
        "data/model/colosseum/gate_d_01.rsm2",
        "colosseum\\colo_d_03.bmp",
    ),
    (
        "data/model/colosseum/gate_d_01.rsm2",
        "colosseum\\colo_d_04.bmp",
    ),
    (
        "data/model/colosseum/gate_d_02.rsm2",
        "colosseum\\colo_d_03.bmp",
    ),
    ("data/model/gevent/vrag02_40.rsm2", "gevent\\vrag02_40.bik"),
    ("data/model/gevent/vrag02_70.rsm2", "gevent\\vrag02_70.bik"),
    (
        "data/model/gevent/vrag02d_40.rsm2",
        "gevent\\vrag02d_40.bik",
    ),
];

fn pilot_features(model: &Rsm2) -> PilotFeatures {
    let mut facts = PilotFeatures {
        roots: model.roots.len(),
        nodes: model.nodes.len(),
        multiple_roots: model.roots.len() > 1,
        no_shade: model.shade_type == 0,
        ..Default::default()
    };
    let mut one_sided = false;
    for node in &model.nodes {
        facts.position_animation |= !node.position_keys.is_empty();
        facts.rotation_animation |= !node.rotation_keys.is_empty();
        facts.scale_animation |= !node.scale_keys.is_empty();
        facts.two_sided_faces |= node.faces.iter().any(|face| face.two_sided != 0);
        one_sided |= node.faces.iter().any(|face| face.two_sided == 0);
        let texture_names: Vec<&str> = match &node.textures {
            Rsm2NodeTextures::GlobalIndices(indices) => indices
                .iter()
                .map(|index| model.global_textures[*index].as_str())
                .collect(),
            Rsm2NodeTextures::Names(names) => names.iter().map(String::as_str).collect(),
        };
        facts.tga_texture |= texture_names.iter().any(|name| is_tga(name));
        for animation in &node.texture_animations {
            if is_tga(texture_names[animation.texture_index]) {
                facts.animated_tga = true;
            } else {
                facts.animated_non_tga = true;
            }
            for channel in &animation.channels {
                match channel.channel_type {
                    Rsm2TextureChannelType::TranslateU => facts.uv_translate_u = true,
                    Rsm2TextureChannelType::TranslateV => facts.uv_translate_v = true,
                    Rsm2TextureChannelType::ScaleU => facts.uv_scale_u = true,
                    Rsm2TextureChannelType::ScaleV => facts.uv_scale_v = true,
                    Rsm2TextureChannelType::Rotate => facts.uv_rotate = true,
                }
            }
        }
    }
    facts.mixed_face_culling = facts.two_sided_faces && one_sided;
    facts
}

fn is_tga(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("tga"))
}

fn texture_fallbacks(vfs: &impl crate::grf_vfs::AssetRead, model: &Rsm2) -> Vec<String> {
    let mut fallbacks = BTreeSet::new();
    for node in &model.nodes {
        let names: Vec<&str> = match &node.textures {
            Rsm2NodeTextures::GlobalIndices(indices) => indices
                .iter()
                .map(|index| model.global_textures[*index].as_str())
                .collect(),
            Rsm2NodeTextures::Names(names) => names.iter().map(String::as_str).collect(),
        };
        for name in names {
            let is_bik = name
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bik"));
            if is_bik || vfs.read_asset(&format!("data/texture/{name}")).is_none() {
                fallbacks.insert(name.to_string());
            }
        }
    }
    fallbacks.into_iter().collect()
}

fn scratch_path(root: &Path, asset: &PhysicalAsset, source_hash: &str) -> PathBuf {
    let stem = asset
        .archive_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("archive");
    root.join(format!("{}-{}-{stem}", asset.priority, asset.source_index))
        .join(format!("{}-{source_hash}", asset.entry_index))
}

fn count_row(counts: &mut CorpusCounts, row: &CorpusModelRow) {
    counts.physical_models += 1;
    counts.effective_models += usize::from(row.inventory.effective);
    counts.shadowed += usize::from(!row.inventory.effective);
    counts.extension_mismatches += usize::from(row.inventory.extension_mismatch);
    counts.texture_fallback_models += usize::from(!row.texture_fallbacks.is_empty());
    counts.texture_fallbacks += row.texture_fallbacks.len();
    counts.supported_rsm2 += usize::from(matches!(
        (row.inventory.header_major, row.inventory.header_minor),
        (Some(2), Some(2 | 3))
    ));
    counts.well_formed_rsm2 += usize::from(row.features.is_some());
    counts.well_formed_no_shade +=
        usize::from(row.features.as_ref().is_some_and(|facts| facts.no_shade));
    match row.outcome {
        CorpusOutcome::Converted => counts.converted += 1,
        CorpusOutcome::Skipped => counts.skipped += 1,
        CorpusOutcome::UnsupportedLegacy => counts.unsupported_legacy += 1,
        CorpusOutcome::UnsupportedVersion => counts.unsupported_version += 1,
        CorpusOutcome::Malformed => counts.malformed += 1,
        CorpusOutcome::Failed => counts.failed += 1,
        CorpusOutcome::Gated => counts.gated += 1,
    }
}

fn stage_from_error(error: &str) -> Option<String> {
    error
        .split("stage ")
        .nth(1)
        .map(|stage| stage.split(':').next().unwrap_or(stage).to_string())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    let payload = &*payload;
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else {
        "conversion panicked without a message".to_string()
    }
}

fn process_models(
    vfs: &impl crate::grf_vfs::AssetRead,
    assets: &[PhysicalAsset],
    preflight: &PreflightReport,
    extracted_root: &Path,
    output_root: &Path,
    force: bool,
) -> CorpusReport {
    let mut rows_by_id: BTreeMap<_, _> = preflight
        .models
        .iter()
        .cloned()
        .map(|row| ((row.source_index, row.entry_index), row))
        .collect();
    let mut model_assets: Vec<_> = assets
        .iter()
        .filter(|asset| has_extension(asset, &["rsm", "rsm2"]))
        .collect();
    model_assets.sort_by_key(|asset| (asset.source_index, asset.entry_index));
    let gated = preflight.has_gates();
    let mut models = Vec::with_capacity(model_assets.len());
    let mut blockers: Vec<String> = preflight
        .blocking_paths(usize::MAX)
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut observed_known = HashSet::new();

    for asset in model_assets {
        let mut inventory = rows_by_id
            .remove(&(asset.source_index, asset.entry_index))
            .expect("preflight row for physical model");
        let bytes = std::fs::read(extracted_path(extracted_root, asset));
        let (source_hash, scratch) = match &bytes {
            Ok(bytes) => {
                let hash = hash_hex(bytes);
                (hash.clone(), scratch_path(output_root, asset, &hash))
            }
            Err(_) => (
                String::new(),
                scratch_path(output_root, asset, "unreadable"),
            ),
        };
        inventory.source_hash = (!source_hash.is_empty()).then_some(source_hash.clone());
        let supported_source = matches!(
            (inventory.header_major, inventory.header_minor),
            (Some(2), Some(2 | 3))
        );
        let mut row = CorpusModelRow {
            inventory,
            scratch_path: scratch.to_string_lossy().replace('\\', "/"),
            outcome: CorpusOutcome::Failed,
            stage: None,
            context_error: None,
            known_malformed: false,
            texture_fallbacks: Vec::new(),
            features: None,
        };

        match bytes {
            Err(error) => {
                row.stage = Some("read extracted source".to_string());
                row.context_error = Some(error.to_string());
                blockers.push(row.inventory.logical_path.clone());
            }
            Ok(bytes) => {
                let format = classify_header(&bytes);
                if let Ok(ModelFormat::Unsupported { major, minor }) = &format {
                    row.stage = Some("dispatch".to_string());
                    row.context_error = Some(format!("unsupported model version {major}.{minor}"));
                    if *major == 1 {
                        row.outcome = CorpusOutcome::UnsupportedLegacy;
                    } else {
                        row.outcome = CorpusOutcome::UnsupportedVersion;
                    }
                    models.push(row);
                    continue;
                }
                if let Ok(ModelFormat::Rsm2 { .. }) = format {
                    match Rsm2::from_bytes(&bytes) {
                        Ok(model) => {
                            row.texture_fallbacks = texture_fallbacks(vfs, &model);
                            row.features = Some(pilot_features(&model));
                        }
                        Err(error) => {
                            row.outcome = CorpusOutcome::Malformed;
                            row.stage = Some("parse RSM2".to_string());
                            row.context_error = Some(error.to_string());
                            row.known_malformed = KNOWN_MALFORMED_RSM2.contains(&(
                                row.inventory.logical_path.as_str(),
                                source_hash.as_str(),
                            ));
                            if row.known_malformed {
                                observed_known.insert((
                                    row.inventory.logical_path.clone(),
                                    source_hash.clone(),
                                ));
                            } else {
                                blockers.push(row.inventory.logical_path.clone());
                            }
                            models.push(row);
                            continue;
                        }
                    }
                }
                if gated {
                    row.outcome = if row.inventory.outcome == ModelPreflightOutcome::MalformedHeader
                    {
                        CorpusOutcome::Malformed
                    } else {
                        CorpusOutcome::Gated
                    };
                    row.stage = Some("preflight gate".to_string());
                } else {
                    let mut pool = TexturePool::new(&scratch);
                    let conversion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        convert_model_bytes(
                            vfs,
                            &row.inventory.logical_path,
                            &bytes,
                            &scratch,
                            &mut pool,
                            force,
                        )
                    }));
                    match conversion {
                        Ok(Ok(ConvertOutcome::Converted)) => row.outcome = CorpusOutcome::Converted,
                        Ok(Ok(ConvertOutcome::Skipped)) => {
                            row.outcome = CorpusOutcome::Skipped;
                            row.stage = Some("validate skipped output".to_string());
                            row.context_error = Some(
                                "existing output was not revalidated; rerun corpus conversion with --force"
                                    .to_string(),
                            );
                            if supported_source {
                                blockers.push(row.inventory.logical_path.clone());
                            }
                        }
                        Ok(Ok(ConvertOutcome::UnsupportedVersion)) => {
                            row.outcome = CorpusOutcome::UnsupportedLegacy
                        }
                        Ok(Err(error)) => {
                            let message = format!("{error:#}");
                            row.stage = stage_from_error(&message);
                            row.context_error = Some(message);
                            if supported_source {
                                blockers.push(row.inventory.logical_path.clone());
                            }
                        }
                        Err(payload) => {
                            row.stage = Some("panic during conversion".to_string());
                            row.context_error = Some(panic_message(payload));
                            if supported_source {
                                blockers.push(row.inventory.logical_path.clone());
                            }
                        }
                    }
                }
            }
        }
        models.push(row);
    }

    if !gated
        && assets.iter().any(|asset| {
            asset
                .archive_path
                .file_name()
                .is_some_and(|name| name == "data.grf")
        })
    {
        for (path, hash) in KNOWN_MALFORMED_RSM2 {
            if !observed_known.contains(&(path.to_string(), hash.to_string())) {
                blockers.push(path.to_string());
            }
        }
    }
    blockers.sort();
    blockers.dedup();

    let mut archive_map = BTreeMap::<(usize, u32, String), CorpusCounts>::new();
    for asset in assets {
        archive_map
            .entry((
                asset.source_index,
                asset.priority,
                asset.archive_path.to_string_lossy().replace('\\', "/"),
            ))
            .or_default();
    }
    for row in &models {
        let key = (
            row.inventory.source_index,
            row.inventory.priority,
            row.inventory.archive.clone(),
        );
        count_row(archive_map.get_mut(&key).expect("archive count"), row);
    }
    let effective = effective_entries(assets);
    for asset in assets.iter().filter(|asset| {
        has_extension(asset, &["rsw"])
            && effective.contains(&(asset.source_index, asset.entry_index))
    }) {
        let key = (
            asset.source_index,
            asset.priority,
            asset.archive_path.to_string_lossy().replace('\\', "/"),
        );
        archive_map
            .get_mut(&key)
            .expect("archive count")
            .effective_worlds += 1;
    }
    for placement in &preflight.placements {
        let key = (
            placement.source_index,
            placement.priority,
            placement.archive.clone(),
        );
        archive_map.get_mut(&key).expect("archive count").placements += 1;
    }
    let archives: Vec<_> = archive_map
        .into_iter()
        .map(
            |((source_index, priority, archive), counts)| ArchiveCounts {
                archive,
                priority,
                source_index,
                counts,
            },
        )
        .collect();
    let mut totals = CorpusCounts::default();
    for row in &models {
        count_row(&mut totals, row);
    }
    totals.effective_worlds = archives
        .iter()
        .map(|archive| archive.counts.effective_worlds)
        .sum();
    totals.placements = preflight.placements.len();

    if assets.iter().any(|asset| {
        asset
            .archive_path
            .file_name()
            .is_some_and(|name| name == "data.grf")
    }) {
        let fallback_identities: BTreeSet<_> = models
            .iter()
            .flat_map(|row| {
                row.texture_fallbacks
                    .iter()
                    .map(|texture| (row.inventory.logical_path.as_str(), texture.as_str()))
            })
            .collect();
        let expected_fallbacks: BTreeSet<_> = KNOWN_RSM2_TEXTURE_FALLBACKS.into_iter().collect();
        if fallback_identities != expected_fallbacks {
            blockers.push(format!(
                "retail texture fallback identities changed: expected {expected_fallbacks:?}, got {fallback_identities:?}"
            ));
        }
        let expected = (9_561, 1_104, 603_548, 2_491, 2_467, 399, 7, 13);
        let actual = (
            totals.physical_models,
            totals.effective_worlds,
            totals.placements,
            totals.supported_rsm2,
            totals.well_formed_rsm2,
            totals.well_formed_no_shade,
            totals.texture_fallback_models,
            totals.texture_fallbacks,
        );
        if actual != expected {
            blockers.push(format!(
                "retail totals changed: expected {expected:?}, got {actual:?}"
            ));
        }
        for (name, expected) in [("data.grf", (9_561, 1_104, 603_548)), ("en.grf", (0, 0, 0))] {
            let actual = archives
                .iter()
                .find(|archive| {
                    Path::new(&archive.archive)
                        .file_name()
                        .is_some_and(|value| value == name)
                })
                .map(|archive| {
                    (
                        archive.counts.physical_models,
                        archive.counts.effective_worlds,
                        archive.counts.placements,
                    )
                })
                .unwrap_or_default();
            if actual != expected {
                blockers.push(format!(
                    "retail archive {name} totals changed: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }

    CorpusReport {
        models,
        placements: preflight.placements.clone(),
        inventory_errors: preflight.errors.clone(),
        archives,
        totals,
        blockers,
    }
}

fn extraction_failure_report(
    vfs: &GrfVfs,
    assets: &[PhysicalAsset],
    extracted_root: &Path,
    output_root: &Path,
    error: &anyhow::Error,
) -> CorpusReport {
    let effective = effective_entries(assets);
    let context = format!("{error:#}");
    let mut model_assets: Vec<_> = assets
        .iter()
        .filter(|asset| has_extension(asset, &["rsm", "rsm2"]))
        .collect();
    model_assets.sort_by_key(|asset| (asset.source_index, asset.entry_index));
    let models: Vec<_> = model_assets
        .into_iter()
        .map(|asset| {
            let bytes = vfs.read_physical(asset);
            let mut inventory = bytes.as_ref().map_or_else(
                || {
                    unreadable_model(
                        asset,
                        effective.contains(&(asset.source_index, asset.entry_index)),
                    )
                },
                |bytes| {
                    inspect_model(
                        asset,
                        bytes.clone(),
                        effective.contains(&(asset.source_index, asset.entry_index)),
                    )
                },
            );
            inventory.error = Some(context.clone());
            let hash = bytes
                .as_ref()
                .map(|bytes| hash_hex(bytes))
                .unwrap_or_default();
            inventory.source_hash = (!hash.is_empty()).then_some(hash.clone());
            CorpusModelRow {
                inventory,
                scratch_path: scratch_path(
                    output_root,
                    asset,
                    if hash.is_empty() { "unreadable" } else { &hash },
                )
                .to_string_lossy()
                .replace('\\', "/"),
                outcome: CorpusOutcome::Gated,
                stage: Some("validate extraction cache".to_string()),
                context_error: Some(context.clone()),
                known_malformed: false,
                texture_fallbacks: Vec::new(),
                features: None,
            }
        })
        .collect();
    let mut archive_map = BTreeMap::<(usize, u32, String), CorpusCounts>::new();
    for asset in assets {
        archive_map
            .entry((
                asset.source_index,
                asset.priority,
                asset.archive_path.to_string_lossy().replace('\\', "/"),
            ))
            .or_default();
    }
    for row in &models {
        let key = (
            row.inventory.source_index,
            row.inventory.priority,
            row.inventory.archive.clone(),
        );
        count_row(archive_map.get_mut(&key).expect("archive count"), row);
    }
    for asset in assets.iter().filter(|asset| {
        has_extension(asset, &["rsw"])
            && effective.contains(&(asset.source_index, asset.entry_index))
    }) {
        let key = (
            asset.source_index,
            asset.priority,
            asset.archive_path.to_string_lossy().replace('\\', "/"),
        );
        archive_map
            .get_mut(&key)
            .expect("archive count")
            .effective_worlds += 1;
    }
    let archives: Vec<_> = archive_map
        .into_iter()
        .map(
            |((source_index, priority, archive), counts)| ArchiveCounts {
                archive,
                priority,
                source_index,
                counts,
            },
        )
        .collect();
    let mut totals = CorpusCounts::default();
    for row in &models {
        count_row(&mut totals, row);
    }
    totals.effective_worlds = archives
        .iter()
        .map(|archive| archive.counts.effective_worlds)
        .sum();
    CorpusReport {
        models,
        placements: Vec::new(),
        inventory_errors: vec![InventoryError {
            logical_path: extracted_root.to_string_lossy().into_owned(),
            error: context,
        }],
        archives,
        totals,
        blockers: vec![extracted_root.to_string_lossy().into_owned()],
    }
}

pub fn convert_corpus(
    vfs: &GrfVfs,
    extracted_root: &Path,
    output_root: &Path,
    report_path: &Path,
    force: bool,
) -> anyhow::Result<CorpusReport> {
    let assets: Vec<_> = vfs.physical_assets().collect();
    let report = match inventory(vfs, extracted_root) {
        Ok(preflight) => {
            process_models(vfs, &assets, &preflight, extracted_root, output_root, force)
        }
        Err(error) => extraction_failure_report(vfs, &assets, extracted_root, output_root, &error),
    };
    write_report(report_path, &report)?;
    Ok(report)
}

fn write_report(path: &Path, report: &CorpusReport) -> anyhow::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
    let mut json = serde_json::to_vec_pretty(report)?;
    json.push(b'\n');
    std::fs::write(path, json)?;
    Ok(())
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

        let source = asset("data\\prontera.rsw", 0, 7);
        let rows = inspect_placements(&source, "data\\prontera.rsw", &world);

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

    #[test]
    fn corpus_keeps_duplicate_physical_bytes_isolated_and_continues_after_failure() {
        use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm2};

        let extracted = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let assets = vec![
            asset("data\\model\\same.rsm2", 0, 3),
            asset("data\\model\\same.rsm2", 0, 4),
            asset("data\\model\\broken.rsm2", 0, 5),
        ];
        let sources = [
            encode_rsm2(2, "bark.bmp"),
            encode_rsm2(3, "bark.bmp"),
            b"GRSM\x02\x03".to_vec(),
        ];
        for (candidate, bytes) in assets.iter().zip(&sources) {
            let path = extracted_path(extracted.path(), candidate);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let models = assets
            .iter()
            .zip(&sources)
            .map(|(candidate, bytes)| inspect_model(candidate, bytes.clone(), true))
            .collect();
        let preflight = PreflightReport {
            models,
            placements: Vec::new(),
            errors: Vec::new(),
            summary: PreflightSummary {
                physical_models: 3,
                effective_models: 3,
                ..Default::default()
            },
        };
        let vfs = FakeVfs::with(&[("data/texture/bark.bmp", bmp_bytes([255; 4]))]);

        let report = process_models(
            &vfs,
            &assets,
            &preflight,
            extracted.path(),
            output.path(),
            false,
        );

        assert_eq!(report.models.len(), 3);
        assert_eq!(report.models[0].outcome, CorpusOutcome::Converted);
        assert_eq!(report.models[1].outcome, CorpusOutcome::Converted);
        assert_eq!(report.models[2].outcome, CorpusOutcome::Malformed);
        assert_eq!(report.blockers, ["data/model/broken.rsm2"]);
        assert_eq!(
            report.models[0].inventory.source_hash.as_deref(),
            Some(hash_hex(&sources[0]).as_str())
        );
        assert_eq!(
            report.models[1].inventory.source_hash.as_deref(),
            Some(hash_hex(&sources[1]).as_str())
        );
        assert_ne!(report.models[0].scratch_path, report.models[1].scratch_path);
        assert!(report.models[0].scratch_path.contains("0-0-archive-0/3-"));
        assert!(report.models[1].scratch_path.contains("0-0-archive-0/4-"));
        assert!(
            Path::new(&report.models[0].scratch_path)
                .join("data/model/same.glb")
                .is_file()
        );
        let first_glb = Path::new(&report.models[0].scratch_path).join("data/model/same.glb");
        let second_glb = Path::new(&report.models[1].scratch_path).join("data/model/same.glb");
        assert!(first_glb.is_file());
        assert!(second_glb.is_file());
        for (glb, source) in [(first_glb, &sources[0]), (second_glb, &sources[1])] {
            let document = gltf::import(glb).unwrap().0.into_json();
            let provenance: lifthrasir_data::lif::LifModel = serde_json::from_value(
                document.extensions.unwrap().others[lifthrasir_data::lif::EXTENSION_MODEL].clone(),
            )
            .unwrap();
            assert_eq!(provenance.rsm_hash, hash_hex(source));
        }
        let report_path = output.path().join("report.json");
        write_report(&report_path, &report).unwrap();
        let first_json = std::fs::read(&report_path).unwrap();
        write_report(&report_path, &report).unwrap();
        assert_eq!(std::fs::read(report_path).unwrap(), first_json);
        let json: serde_json::Value = serde_json::from_slice(&first_json).unwrap();
        let first_row = json["models"][0].as_object().unwrap();
        assert!(first_row.contains_key("outcome"));
        assert!(first_row.contains_key("conversion_outcome"));

        let rerun = process_models(
            &vfs,
            &assets,
            &preflight,
            extracted.path(),
            output.path(),
            false,
        );
        assert_eq!(rerun.totals.skipped, 2);
        assert!(rerun.blockers.contains(&"data/model/same.rsm2".to_string()));
        assert!(
            rerun.models[0]
                .context_error
                .as_deref()
                .unwrap()
                .contains("--force")
        );
    }

    #[test]
    fn corpus_failure_policy_pins_known_malformed_identity_and_gates_all_work() {
        let (path, hash) = KNOWN_MALFORMED_RSM2[0];
        assert!(KNOWN_MALFORMED_RSM2.contains(&(path, hash)));
        assert!(!KNOWN_MALFORMED_RSM2.contains(&(path, "changed")));

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
        assert_eq!(report.blocking_paths(1), ["data/map.rsw"]);
    }
}
