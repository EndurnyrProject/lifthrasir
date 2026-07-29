#[cfg(test)]
pub mod fixtures;
pub mod terrain;
pub mod textures;
pub mod validate;
pub mod water;
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
/// Every supported prop the RSW references is converted into `<models_dir>`
/// first (`--force-models` re-converts even a prop already on disk), and the
/// map glb's `lif_prop` refs point at the resulting library glbs.
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

    let (converted_models, prop_counts) =
        convert_props(vfs, map_name, &world, models_dir, force_models)?;

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
    let counts = write_and_validate_glb_atomically(&glb_path, &inputs)?;
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

fn write_and_validate_glb_atomically(
    glb_path: &Path,
    inputs: &writer::MapGlbInputs,
) -> anyhow::Result<validate::Counts> {
    let parent = glb_path
        .parent()
        .with_context(|| format!("finding parent for {}", glb_path.display()))?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temporary GLB beside {}", glb_path.display()))?;
    writer::write_glb(temporary.path(), inputs)?;
    let counts = validate::validate(temporary.path(), inputs)
        .with_context(|| format!("validating {}", glb_path.display()))?;
    temporary
        .persist(glb_path)
        .map_err(|error| error.error)
        .with_context(|| format!("publishing {}", glb_path.display()))?;
    Ok(counts)
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

/// Converts every distinct supported model the RSW references
/// (`emitted_objects` is the same list the writer emits, so a prop dropped
/// there never reaches the converter either) and returns the filenames now
/// backed by a library glb -- Converted this run or already Skipped -- for the
/// writer to flip refs against. Only unsupported legacy RSM1 versions are
/// counted and left on the native ref; every other failure aborts the map.
fn convert_props(
    vfs: &impl AssetRead,
    map_name: &str,
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
            .with_context(|| format!("map '{map_name}', converting prop model '{filename}'"))?;
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

    #[test]
    fn failed_glb_write_leaves_no_destination() {
        let ground = fixtures::mini_ground();
        let world = fixtures::mini_world();
        let primitives = terrain::build_terrain(&ground).expect("terrain");
        let textures = fixtures::textures();
        let converted_models = HashSet::from(["prontera\\tree01.rsm".to_string()]);
        let out = tempfile::tempdir().expect("tempdir");
        let glb_path = out.path().join("mini.glb");
        let inputs = writer::MapGlbInputs {
            map_name: "mini",
            ground: &ground,
            world: &world,
            primitives: &primitives,
            textures: &textures,
            gat_bytes: b"not a GAT",
            gnd_bytes: b"gnd",
            rsw_bytes: b"rsw",
            converted_models: &converted_models,
        };

        assert!(write_and_validate_glb_atomically(&glb_path, &inputs).is_err());
        assert!(!glb_path.exists());
    }

    /// The Phase 1 pilot map, and the one the prop closure is proven on.
    const PILOT_MAP: &str = "izlude";

    fn open_retail_grfs() -> GrfVfs {
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
        GrfVfs::open(&entries.iter().collect::<Vec<_>>()).expect("open GRFs")
    }

    /// The `lif_prop.model` ref of every prop node in a written map glb.
    fn prop_model_refs(glb_path: &Path) -> Vec<String> {
        let gltf::Gltf { document, .. } = gltf::Gltf::open(glb_path).expect("reopen map glb");
        document
            .nodes()
            .filter_map(|node| {
                let raw = node.extras().as_ref()?;
                let extras: serde_json::Value =
                    serde_json::from_str(raw.get()).expect("parsing node extras");
                let prop = extras.get(lifthrasir_data::lif::EXTRAS_PROP)?;
                let prop: lifthrasir_data::lif::LifProp =
                    serde_json::from_value(prop.clone()).expect("decoding lif_prop");
                Some(prop.model)
            })
            .collect()
    }

    /// End-to-end against the retail GRFs: convert a real map and let the
    /// validator judge the output. Ignored by default because it needs
    /// `assets/*.grf`, which are not in the repo.
    #[test]
    #[ignore = "requires the retail GRFs in assets/"]
    fn converts_a_real_map_and_validates_the_output() {
        let vfs = open_retail_grfs();
        let out = tempfile::tempdir().expect("tempdir");
        let models_dir = out.path().join("models");

        run(&vfs, "prt_fild08", out.path(), &models_dir, false).expect("convert prt_fild08");

        assert!(out.path().join("prt_fild08/prt_fild08.glb").is_file());
        let textures = std::fs::read_dir(out.path().join("prt_fild08/tex"))
            .expect("tex dir")
            .count();
        assert!(textures > 0, "no textures exported");
    }

    /// The prop closure on real data: every prop the pilot map references
    /// resolves to a library glb that exists on disk, and the only refs left on
    /// the native `.rsm` path are unsupported legacy RSM1 models.
    #[test]
    #[ignore = "requires the retail GRFs in assets/"]
    fn the_pilot_map_prop_refs_close_over_the_glb_library() {
        let vfs = open_retail_grfs();
        let out = tempfile::tempdir().expect("tempdir");
        let models_dir = out.path().join("models");

        run(&vfs, PILOT_MAP, out.path(), &models_dir, false).expect("convert pilot map");

        let refs = prop_model_refs(&out.path().join(format!("{PILOT_MAP}/{PILOT_MAP}.glb")));
        assert!(!refs.is_empty(), "{PILOT_MAP} referenced no props");

        let mut library = HashSet::new();
        for model_ref in &refs {
            if let Some(relative) = model_ref.strip_prefix("ro://models/") {
                assert!(
                    models_dir.join(relative).is_file(),
                    "prop ref '{model_ref}' has no glb under {}",
                    models_dir.display()
                );
                library.insert(relative.to_string());
                continue;
            }

            let native = model_ref
                .strip_prefix("ro://data/model/")
                .unwrap_or_else(|| panic!("unexpected prop ref '{model_ref}'"));
            let bytes = vfs
                .read_asset(&format!("data/model/{native}"))
                .unwrap_or_else(|| panic!("prop '{native}' not in the GRFs"));
            assert!(
                !model::is_supported_version(&bytes).expect("RSM header"),
                "prop '{native}' kept its native ref at a supported RSM version"
            );
        }
        assert!(!library.is_empty(), "{PILOT_MAP} produced no library glbs");

        // `gltf::import` resolves the `../tex/` image URIs off disk and decodes
        // every PNG, which `Gltf::open` (what the converter's own validator
        // uses) does not.
        let animated = library
            .iter()
            .map(|relative| {
                let path = models_dir.join(relative);
                let (document, _, images) =
                    gltf::import(&path).unwrap_or_else(|error| panic!("{relative}: {error}"));
                assert!(!images.is_empty(), "{relative} resolved no images");
                document.animations().count()
            })
            .filter(|count| *count > 0)
            .count();
        assert!(animated > 0, "{PILOT_MAP} produced no animated prop glbs");
    }

    fn props_world(objects: Vec<RswObject>) -> RoWorld {
        RoWorld {
            version: "2.2".into(),
            raw_version: 0x0202,
            build_number: 0,
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
    fn convert_props_closes_supported_models_dedupes_and_counts_legacy_fallback() {
        use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm, encode_rsm2};

        let vfs = FakeVfs::with(&[
            (
                "data/model/prontera/tree01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
            (
                "data/model/prontera/sign22.rsm2",
                encode_rsm2(2, "bark.bmp"),
            ),
            ("data/model/prontera/sign23.rsm", encode_rsm2(3, "bark.bmp")),
            (
                "data/model/prontera/relic01.rsm",
                encode_rsm((1, 6), &["bark.bmp"]),
            ),
            ("data/texture/bark.bmp", bmp_bytes([255, 255, 255, 255])),
        ]);
        let world = props_world(vec![
            model_object("tree_a", "prontera\\tree01.rsm"),
            model_object("tree_b", "prontera\\tree01.rsm"),
            model_object("sign22_a", "prontera\\sign22.rsm2"),
            model_object("sign22_b", "prontera\\sign22.rsm2"),
            model_object("sign23", "prontera\\sign23.rsm"),
            model_object("relic", "prontera\\relic01.rsm"),
        ]);
        let out = tempfile::tempdir().expect("tempdir");

        let (converted_models, counts) =
            convert_props(&vfs, "synthetic", &world, out.path(), false).expect("convert props");

        assert_eq!(counts.converted, 3);
        assert_eq!(counts.skipped, 0);
        assert_eq!(counts.fallback, 1);
        assert_eq!(vfs.reads("data/model/prontera/tree01.rsm"), 1);
        assert_eq!(vfs.reads("data/model/prontera/sign22.rsm2"), 1);
        for filename in [
            "prontera\\tree01.rsm",
            "prontera\\sign22.rsm2",
            "prontera\\sign23.rsm",
        ] {
            assert!(converted_models.contains(filename));
            assert!(
                out.path()
                    .join(model::glb_relative_path(filename))
                    .is_file()
            );
            let object = model_object("prop", filename);
            let RswObject::Model(model) = &object else {
                unreachable!();
            };
            let prop = writer::lif_prop(model, &converted_models).expect("converted prop ref");
            assert_eq!(
                prop.model,
                format!("ro://models/{}", model::glb_relative_path(filename))
            );
        }
        assert!(!converted_models.contains("prontera\\relic01.rsm"));
    }

    #[test]
    fn convert_props_counts_skips_stably_on_a_rerun() {
        use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm, encode_rsm2};

        let vfs = FakeVfs::with(&[
            (
                "data/model/prontera/tree01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
            (
                "data/model/prontera/sign22.rsm2",
                encode_rsm2(2, "bark.bmp"),
            ),
            ("data/texture/bark.bmp", bmp_bytes([255, 255, 255, 255])),
        ]);
        let world = props_world(vec![
            model_object("tree_a", "prontera\\tree01.rsm"),
            model_object("tree_b", "prontera\\tree01.rsm"),
            model_object("sign_a", "prontera\\sign22.rsm2"),
            model_object("sign_b", "prontera\\sign22.rsm2"),
        ]);
        let out = tempfile::tempdir().expect("tempdir");

        convert_props(&vfs, "synthetic", &world, out.path(), false).expect("first run");
        let (converted_models, counts) =
            convert_props(&vfs, "synthetic", &world, out.path(), false).expect("second run");

        assert_eq!(counts.converted, 0);
        assert_eq!(counts.skipped, 2);
        assert_eq!(counts.fallback, 0);
        assert_eq!(vfs.reads("data/model/prontera/tree01.rsm"), 1);
        assert_eq!(vfs.reads("data/model/prontera/sign22.rsm2"), 1);
        assert!(converted_models.contains("prontera\\tree01.rsm"));
        assert!(converted_models.contains("prontera\\sign22.rsm2"));
    }

    #[test]
    fn convert_props_aborts_on_unsupported_or_malformed_rsm2_with_context() {
        use crate::converters::model::fixtures::FakeVfs;

        for (filename, bytes, version, stage) in [
            (
                "prontera\\future.rsm2",
                b"GRSM\x02\x04".to_vec(),
                "2.4",
                "stage dispatch",
            ),
            (
                "prontera\\broken.rsm2",
                b"GRSM\x02\x03".to_vec(),
                "2.3",
                "stage parse RSM2",
            ),
        ] {
            let path = format!("data/model/{}", filename.replace('\\', "/"));
            let vfs = FakeVfs::with(&[(path.as_str(), bytes)]);
            let world = props_world(vec![model_object("bad", filename)]);
            let out = tempfile::tempdir().expect("tempdir");

            let error = convert_props(&vfs, "test_map", &world, out.path(), false)
                .expect_err("RSM2 must abort map conversion");
            let message = format!("{error:#}");
            assert!(message.contains("map 'test_map'"), "{message}");
            assert!(message.contains(filename), "{message}");
            assert!(message.contains(&format!("version {version}")), "{message}");
            assert!(message.contains(stage), "{message}");
            assert!(!out.path().join(model::glb_relative_path(filename)).exists());
        }
    }

    #[test]
    fn convert_props_uses_the_rsm2_missing_texture_fallback() {
        use crate::converters::model::fixtures::{FakeVfs, encode_rsm2};

        let filename = "prontera\\missing_texture.rsm2";
        let path = format!("data/model/{}", filename.replace('\\', "/"));
        let vfs = FakeVfs::with(&[(path.as_str(), encode_rsm2(2, "missing.bmp"))]);
        let world = props_world(vec![model_object("fallback", filename)]);
        let out = tempfile::tempdir().expect("tempdir");

        convert_props(&vfs, "test_map", &world, out.path(), false).unwrap();

        let glb = out.path().join(model::glb_relative_path(filename));
        assert!(glb.is_file());
        let (_, _, images) = gltf::import(glb).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(
            images[0].pixels,
            [
                255, 0, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255, 255,
            ]
        );
    }
}
