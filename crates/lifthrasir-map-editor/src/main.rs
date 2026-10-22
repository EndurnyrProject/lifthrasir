//! Composition root: CLI, source registration, document and feature plugins.

mod application;
mod infrastructure;
mod presentation;

#[cfg(test)]
mod test_support;

use anyhow::Result;
use bevy::{asset::AssetPlugin, prelude::*};
use clap::Parser;
use lifthrasir_map_editor::MapDocument;
use std::path::{Path, PathBuf};

use application::{EditorDocument, EditorSessionPlugin};
use infrastructure::content::{EditorContentPlugin, register_source};
use presentation::{EditorPresentationPlugin, MapAssetPath};

#[derive(Parser)]
#[command(name = "lifthrasir-map-editor")]
struct Args {
    /// Map GLB to open at launch. If omitted, use File > Open File.
    #[arg(long)]
    map: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let selected_map = args
        .map
        .map(resolve_map_path)
        .map(|path| Ok::<_, anyhow::Error>((MapDocument::open(&path)?, map_asset_path(&path)?)))
        .transpose()?;

    let mut app = App::new();
    register_source(&mut app)?;
    app.insert_resource(MapAssetPath(
        selected_map.as_ref().map(|(_, path)| path.clone()),
    ))
    .add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // Cargo runs the binary from its package directory.
                file_path: "../../assets".into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Lifthrasir Map Editor".into(),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((
        EditorContentPlugin,
        EditorSessionPlugin,
        EditorPresentationPlugin,
    ));
    if let Some((document, _)) = selected_map {
        app.insert_resource(EditorDocument(document));
    }
    app.run();
    Ok(())
}

fn resolve_map_path(path: PathBuf) -> PathBuf {
    if path.exists() {
        path
    } else {
        PathBuf::from("assets").join(path)
    }
}

/// Convert an on-disk map path into the asset-source-relative path used by `ro://`.
pub(crate) fn map_asset_path(path: &Path) -> Result<String> {
    let assets = std::fs::canonicalize("assets")?;
    let path = std::fs::canonicalize(path)?;
    let relative = path.strip_prefix(&assets).map_err(|_| {
        anyhow::anyhow!(
            "map '{}' must be inside the editor asset directory '{}'",
            path.display(),
            assets.display()
        )
    })?;
    if path.extension().is_none_or(|extension| extension != "glb") {
        anyhow::bail!("map '{}' must be a .glb file", path.display());
    }
    let relative = relative.to_string_lossy().replace('\\', "/");
    Ok(relative
        .strip_prefix("data/")
        .unwrap_or(&relative)
        .to_string())
}
