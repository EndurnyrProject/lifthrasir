#[cfg(test)]
pub mod fixtures;
pub mod terrain;
pub mod textures;
pub mod validate;
pub mod writer;

use crate::converters::model::{self, ConvertOutcome, TexturePool};
use crate::grf_vfs::{AssetRead, GrfVfs};
use anyhow::Context;
use ro_formats::{RoGround, RoWorld, RswObject};
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

/// Convert one map's RSW/GND/GAT into `<out_dir>/<map_name>/<map_name>.glb`
/// plus its `tex/*.png`, then validate the written file against the sources.
/// Every RSM1 prop the RSW references is converted into `<models_dir>` first
/// (`--force-models` re-converts even a prop already on disk), and the map
/// glb's `lif_prop` refs point at the resulting library glbs.
pub fn run(
    vfs: &GrfVfs,
    map_name: &str,
    out_dir: &Path,
    models_dir: &Path,
    force_models: bool,
) -> anyhow::Result<()> {
    let rsw_bytes = read_source(vfs, map_name, "rsw")?;
    let gnd_bytes = read_source(vfs, map_name, "gnd")?;
    let gat_bytes = read_source(vfs, map_name, "gat")?;

    let world = RoWorld::from_bytes(&rsw_bytes)
        .with_context(|| format!("parsing RSW of map '{map_name}'"))?;
    let ground = RoGround::from_bytes(&gnd_bytes)
        .with_context(|| format!("parsing GND of map '{map_name}'"))?;
    let primitives = terrain::build_terrain(&ground)
        .with_context(|| format!("building terrain of map '{map_name}'"))?;

    let (converted_models, prop_counts) = convert_props(vfs, &world, models_dir, force_models)
        .with_context(|| format!("converting props of map '{map_name}'"))?;

    let map_dir = out_dir.join(map_name);
    std::fs::create_dir_all(&map_dir).with_context(|| format!("creating {}", map_dir.display()))?;
    let textures = textures::normalize_textures(vfs, &ground.textures, &map_dir)?;

    let inputs = writer::MapGlbInputs {
        map_name,
        ground: &ground,
        world: &world,
        primitives: &primitives,
        textures: &textures,
        gat_bytes: &gat_bytes,
        gnd_bytes: &gnd_bytes,
        rsw_bytes: &rsw_bytes,
        converted_models: &converted_models,
    };
    let glb_path = map_dir.join(format!("{map_name}.glb"));
    writer::write_glb(&glb_path, &inputs)?;

    let counts = validate::validate(&glb_path, &inputs)
        .with_context(|| format!("validating {}", glb_path.display()))?;
    println!(
        "{map_name}: {counts}, {} textures, {prop_counts} -> {}",
        textures.len(),
        glb_path.display()
    );

    Ok(())
}

fn read_source(vfs: &GrfVfs, map_name: &str, extension: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!("data/{map_name}.{extension}");
    vfs.read(&path)
        .with_context(|| format!("map source not found in GRFs: {path}"))
}

/// Prop conversion outcome, printed alongside the map's own counts.
#[derive(Debug, Default, PartialEq, Eq)]
struct PropCounts {
    converted: usize,
    skipped: usize,
    fallback: usize,
}

impl fmt::Display for PropCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} props converted, {} skipped, {} fallback to native",
            self.converted, self.skipped, self.fallback
        )
    }
}

/// Converts every distinct RSM1 the RSW references (`emitted_objects` is the
/// same list the writer emits, so a prop dropped there never reaches the
/// converter either) and returns the filenames now backed by a library glb --
/// Converted this run or already Skipped -- for the writer to flip refs
/// against. Unsupported RSM versions are counted but left on the native ref;
/// any other failure aborts the whole map (`convert_model`'s own context names
/// the offending file).
fn convert_props(
    vfs: &impl AssetRead,
    world: &RoWorld,
    models_dir: &Path,
    force_models: bool,
) -> anyhow::Result<(HashSet<String>, PropCounts)> {
    let mut pool = TexturePool::new(models_dir);
    let mut converted_models = HashSet::new();
    let mut counts = PropCounts::default();
    let mut seen = HashSet::new();

    for object in writer::emitted_objects(world) {
        let RswObject::Model(rsw_model) = object else {
            continue;
        };
        let filename = rsw_model.filename.as_str();
        if !seen.insert(filename) {
            continue;
        }

        let outcome = model::convert_model(vfs, filename, models_dir, &mut pool, force_models)
            .with_context(|| format!("converting prop model '{filename}'"))?;
        match outcome {
            ConvertOutcome::Converted => {
                counts.converted += 1;
                converted_models.insert(filename.to_string());
            }
            ConvertOutcome::Skipped => {
                counts.skipped += 1;
                converted_models.insert(filename.to_string());
            }
            ConvertOutcome::UnsupportedVersion => counts.fallback += 1,
        }
    }

    Ok((converted_models, counts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GrfEntry, LoaderConfig};

    /// End-to-end against the retail GRFs: convert a real map and let the
    /// validator judge the output. Ignored by default because it needs
    /// `assets/*.grf`, which are not in the repo.
    #[test]
    #[ignore = "requires the retail GRFs in assets/"]
    fn converts_a_real_map_and_validates_the_output() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let config =
            LoaderConfig::from_path(&workspace.join("assets/convert.toml")).expect("convert.toml");
        let entries: Vec<GrfEntry> = config
            .grfs_by_priority()
            .iter()
            .map(|entry| GrfEntry {
                path: workspace
                    .join("assets")
                    .join(&entry.path)
                    .to_string_lossy()
                    .into_owned(),
                priority: entry.priority,
            })
            .collect();
        let vfs = GrfVfs::open(&entries.iter().collect::<Vec<_>>()).expect("open GRFs");
        let out = tempfile::tempdir().expect("tempdir");
        let models_dir = out.path().join("models");

        run(&vfs, "prt_fild08", out.path(), &models_dir, false).expect("convert prt_fild08");

        assert!(out.path().join("prt_fild08/prt_fild08.glb").is_file());
        let textures = std::fs::read_dir(out.path().join("prt_fild08/tex"))
            .expect("tex dir")
            .count();
        assert!(textures > 0, "no textures exported");
    }

    fn props_world(objects: Vec<RswObject>) -> RoWorld {
        RoWorld {
            version: "2.2".into(),
            ini_file: String::new(),
            gnd_file: String::new(),
            gat_file: String::new(),
            src_file: None,
            water: ro_formats::RswWater::default(),
            light: ro_formats::RswLight::default(),
            ground: ro_formats::RswGround::default(),
            objects,
        }
    }

    fn model_object(name: &str, filename: &str) -> RswObject {
        RswObject::Model(ro_formats::RswModel {
            name: name.to_string(),
            anim_type: 0,
            anim_speed: 1.0,
            block_type: 0,
            filename: filename.to_string(),
            node_name: String::new(),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        })
    }

    #[test]
    fn convert_props_dedupes_filenames_and_counts_the_unsupported_fallback() {
        use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm};

        let vfs = FakeVfs::with(&[
            (
                "data/model/prontera/tree01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
            (
                "data/model/prontera/relic01.rsm",
                encode_rsm((2, 2), &["bark.bmp"]),
            ),
            ("data/texture/bark.bmp", bmp_bytes([255, 255, 255, 255])),
        ]);
        let world = props_world(vec![
            model_object("tree_a", "prontera\\tree01.rsm"),
            model_object("tree_b", "prontera\\tree01.rsm"),
            model_object("relic", "prontera\\relic01.rsm"),
        ]);
        let out = tempfile::tempdir().expect("tempdir");

        let (converted_models, counts) =
            convert_props(&vfs, &world, out.path(), false).expect("convert props");

        assert_eq!(counts.converted, 1);
        assert_eq!(counts.skipped, 0);
        assert_eq!(counts.fallback, 1);
        assert_eq!(vfs.reads("data/model/prontera/tree01.rsm"), 1);
        assert!(converted_models.contains("prontera\\tree01.rsm"));
        assert!(!converted_models.contains("prontera\\relic01.rsm"));
    }

    #[test]
    fn convert_props_counts_skips_on_a_rerun() {
        use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm};

        let vfs = FakeVfs::with(&[
            (
                "data/model/prontera/tree01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
            ("data/texture/bark.bmp", bmp_bytes([255, 255, 255, 255])),
        ]);
        let world = props_world(vec![model_object("tree_a", "prontera\\tree01.rsm")]);
        let out = tempfile::tempdir().expect("tempdir");

        convert_props(&vfs, &world, out.path(), false).expect("first run");
        let (converted_models, counts) =
            convert_props(&vfs, &world, out.path(), false).expect("second run");

        assert_eq!(counts.converted, 0);
        assert_eq!(counts.skipped, 1);
        assert!(converted_models.contains("prontera\\tree01.rsm"));
    }
}
