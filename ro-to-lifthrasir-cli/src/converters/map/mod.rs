#[cfg(test)]
pub mod fixtures;
pub mod terrain;
pub mod textures;
pub mod validate;
pub mod writer;

use crate::grf_vfs::GrfVfs;
use anyhow::Context;
use ro_formats::{RoGround, RoWorld};
use std::path::Path;

/// Convert one map's RSW/GND/GAT into `<out_dir>/<map_name>/<map_name>.glb`
/// plus its `tex/*.png`, then validate the written file against the sources.
pub fn run(vfs: &GrfVfs, map_name: &str, out_dir: &Path) -> anyhow::Result<()> {
    let rsw_bytes = read_source(vfs, map_name, "rsw")?;
    let gnd_bytes = read_source(vfs, map_name, "gnd")?;
    let gat_bytes = read_source(vfs, map_name, "gat")?;

    let world = RoWorld::from_bytes(&rsw_bytes)
        .with_context(|| format!("parsing RSW of map '{map_name}'"))?;
    let ground = RoGround::from_bytes(&gnd_bytes)
        .with_context(|| format!("parsing GND of map '{map_name}'"))?;
    let primitives = terrain::build_terrain(&ground)
        .with_context(|| format!("building terrain of map '{map_name}'"))?;

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
    };
    let glb_path = map_dir.join(format!("{map_name}.glb"));
    writer::write_glb(&glb_path, &inputs)?;

    let counts = validate::validate(&glb_path, &inputs)
        .with_context(|| format!("validating {}", glb_path.display()))?;
    println!(
        "{map_name}: {counts}, {} textures -> {}",
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

        run(&vfs, "prt_fild08", out.path()).expect("convert prt_fild08");

        assert!(out.path().join("prt_fild08/prt_fild08.glb").is_file());
        let textures = std::fs::read_dir(out.path().join("prt_fild08/tex"))
            .expect("tex dir")
            .count();
        assert!(textures > 0, "no textures exported");
    }
}
