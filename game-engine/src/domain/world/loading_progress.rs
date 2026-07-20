//! Map-load progress tracking via `iyes_progress`.
//!
//! One tracked system reports the map-loading pipeline as `Progress`
//! (loader spawned -> gnd/gat/rsw loaded -> mesh built -> textures loaded),
//! and `ProgressPlugin` owns the `Loading -> InGame` transition once
//! everything reports done (i.e. `MapData` exists). The timeout path in
//! `map_loading.rs` still bails to `CharacterSelection` directly.

use crate::core::state::GameState;
use crate::domain::world::components::MapLoader;
use crate::domain::world::map::MapData;
use crate::domain::world::terrain::TerrainTexturesLoading;
use bevy::prelude::*;
use iyes_progress::prelude::*;

pub struct MapLoadProgressPlugin;

impl Plugin for MapLoadProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            ProgressPlugin::<GameState>::new()
                .with_state_transition(GameState::Loading, GameState::InGame),
        )
        .add_systems(
            Update,
            track_map_load_progress
                .track_progress::<GameState>()
                .run_if(in_state(GameState::Loading)),
        );
    }
}

/// Base steps before texture loading: loader spawned, gnd/gat/rsw loaded,
/// mesh stage reached.
const BASE_STEPS: u32 = 5;

fn track_map_load_progress(
    asset_server: Res<AssetServer>,
    loaders: Query<&MapLoader>,
    textures: Query<&TerrainTexturesLoading>,
    maps: Query<(), With<MapData>>,
) -> Progress {
    let texture_total = textures
        .iter()
        .next()
        .map(|t| t.texture_handles.len() as u32)
        .unwrap_or(0);
    let total = BASE_STEPS + texture_total;

    if !maps.is_empty() {
        return Progress { done: total, total };
    }

    let Some(loader) = loaders.iter().next() else {
        return Progress { done: 0, total };
    };

    let asset_done = |loaded: bool| loaded as u32;
    let mut done = 1;
    done += asset_done(asset_server.is_loaded_with_dependencies(loader.ground.id()));
    done += asset_done(loader.altitude.as_ref().is_none_or(|h| {
        asset_server.is_loaded_with_dependencies(h.id())
    }));
    done += asset_done(loader.world.as_ref().is_none_or(|h| {
        asset_server.is_loaded_with_dependencies(h.id())
    }));

    if let Some(loading) = textures.iter().next() {
        done += 1;
        let default_handle = Handle::<Image>::default();
        done += loading
            .texture_handles
            .iter()
            .filter(|h| {
                **h == default_handle || asset_server.is_loaded_with_dependencies(h.id())
            })
            .count() as u32;
    }

    Progress { done, total }
}
