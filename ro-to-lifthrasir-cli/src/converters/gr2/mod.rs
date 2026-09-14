//! Offline conversion of RO GR2 actors and their action clips.

pub(crate) mod animation;
#[cfg(debug_assertions)]
mod validate;
mod writer;

use crate::grf_vfs::{AssetRead, GrfVfs, effective_entries};
use anyhow::{Context, ensure};
use lifthrasir_data::gr2::{self, ATTACK, DEAD, HIT, IDLE, WALK};
use ro_formats::gr2::Gr2File;
use std::{io::Write, path::Path};

const ACTIONS: [(&str, &str); 4] = [
    ("move", WALK),
    ("attack", ATTACK),
    ("damage", HIT),
    ("dead", DEAD),
];

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

pub(crate) fn convert_model(vfs: &impl AssetRead, name: &str, out: &Path) -> anyhow::Result<()> {
    gr2::model_asset_path(name).context("expected a GR2 model basename")?;
    let name = name.to_lowercase();
    let source_path = format!("data/model/3dmob/{name}");
    let bytes = vfs
        .read_asset(&source_path)
        .with_context(|| format!("missing model {source_path}"))?;
    let model = Gr2File::from_bytes(&bytes).with_context(|| format!("decode {source_path}"))?;
    let bone = bone_type(&name)?;
    let mut external = Vec::new();
    for (suffix, action) in ACTIONS {
        let path = format!("data/model/3dmob_bone/{bone}_{suffix}.gr2");
        if let Some(bytes) = vfs.read_asset(&path) {
            external.push((
                action,
                Gr2File::from_bytes(&bytes).with_context(|| format!("decode {path}"))?,
            ));
        }
    }
    let clips: Vec<_> = std::iter::once((IDLE, &model))
        .chain(external.iter().map(|(name, file)| (*name, file)))
        .collect();
    let bytes = writer::build(&model, &clips).with_context(|| format!("export {name}"))?;
    #[cfg(debug_assertions)]
    validate::validate(&bytes, &model, &clips).with_context(|| format!("validate {name}"))?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(out)
        .with_context(|| format!("replace {}", out.display()))?;
    Ok(())
}

fn bone_type(name: &str) -> anyhow::Result<u32> {
    name.strip_suffix(".gr2")
        .and_then(|stem| stem.rsplit_once('_'))
        .context("GR2 model name has no bone type suffix")?
        .1
        .parse()
        .context("invalid GR2 bone type")
}

pub(crate) fn run(vfs: &GrfVfs, out: &Path, selected: Option<&str>) -> anyhow::Result<()> {
    let models = match selected {
        Some(name) => {
            gr2::model_asset_path(name).context("expected a GR2 basename for --model")?;
            vec![name.to_lowercase()]
        }
        None => source_models(vfs),
    };
    ensure!(!models.is_empty(), "no GR2 models in configured GRFs");
    let mut failed = Vec::new();
    for name in &models {
        println!("Converting {name}");
        let path = out.join(name.replace(".gr2", ".glb"));
        if let Err(error) = convert_model(vfs, name, &path) {
            eprintln!("{name}: {error:#}");
            failed.push(name.as_str());
        }
    }
    println!(
        "GR2 models: {} converted, {} failed",
        models.len() - failed.len(),
        failed.len()
    );
    ensure!(
        failed.is_empty(),
        "GR2 conversion failed for {}",
        failed.join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
