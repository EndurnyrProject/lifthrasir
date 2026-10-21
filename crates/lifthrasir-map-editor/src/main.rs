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
use std::path::PathBuf;

use application::{EditorDocument, EditorSessionPlugin};
use infrastructure::content::{EditorContentPlugin, register_source};
use presentation::{EditorPresentationPlugin, MapAssetPath};

#[derive(Parser)]
#[command(name = "lifthrasir-map-editor")]
struct Args {
    /// Path relative to `assets/`, used by Bevy to render the GLB.
    #[arg(long, default_value = "data/maps/prontera/prontera.glb")]
    map: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let document = MapDocument::open(PathBuf::from("assets").join(&args.map))?;
    let map_path = args
        .map
        .strip_prefix("data/")
        .unwrap_or(&args.map)
        .to_string();

    let mut app = App::new();
    register_source(&mut app)?;
    app.insert_resource(EditorDocument(document))
        .insert_resource(MapAssetPath(map_path))
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
    app.run();
    Ok(())
}
