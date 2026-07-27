//! Map-load progress tracking via `iyes_progress`.
//!
//! One tracked system reports the map-loading pipeline as `Progress`
//! (loader spawned -> gnd/gat/rsw loaded -> mesh built -> textures loaded),
//! and `ProgressPlugin` owns the `Loading -> InGame` transition once
//! everything reports done (i.e. `MapData` exists). The timeout path in
//! `map_loading.rs` still bails to `CharacterSelection` directly.
//!
//! A map served by a converted glb has none of those stages -- no `MapLoader`,
//! no `TerrainTexturesLoading` -- so it is tracked by its own coarse steps
//! instead; see [`GLTF_MAP_STEPS`].

use crate::core::state::GameState;
use crate::domain::world::components::MapLoader;
use crate::domain::world::map::MapData;
use crate::domain::world::terrain::TerrainTexturesLoading;
use bevy::prelude::*;
use iyes_progress::prelude::*;

#[cfg(feature = "map-gltf")]
use crate::domain::world::gltf_map::GltfMapLoader;
#[cfg(feature = "map-gltf")]
use bevy::world_serialization::WorldAssetRoot;

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

/// Steps on the glb path: the glb document itself parsed, its textures (the
/// scene's asset dependencies) loaded, and the scene adapted into `MapData`.
/// bevy exposes no finer granularity than "loaded / loaded with dependencies"
/// for a scene handle, so this is as detailed as the glb path gets.
#[cfg(feature = "map-gltf")]
pub(crate) const GLTF_MAP_STEPS: u32 = 3;

pub(crate) fn track_map_load_progress(
    asset_server: Res<AssetServer>,
    loaders: Query<&MapLoader>,
    textures: Query<&TerrainTexturesLoading>,
    maps: Query<(), With<MapData>>,
    #[cfg(feature = "map-gltf")] gltf_roots: Query<&WorldAssetRoot, With<GltfMapLoader>>,
) -> Progress {
    #[cfg(feature = "map-gltf")]
    if let Some(root) = gltf_roots.iter().next() {
        let scene = root.0.id();
        let done = u32::from(asset_server.is_loaded(scene))
            + u32::from(asset_server.is_loaded_with_dependencies(scene))
            + u32::from(!maps.is_empty());
        return Progress {
            done,
            total: GLTF_MAP_STEPS,
        };
    }

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
    done += asset_done(
        loader
            .altitude
            .as_ref()
            .is_none_or(|h| asset_server.is_loaded_with_dependencies(h.id())),
    );
    done += asset_done(
        loader
            .world
            .as_ref()
            .is_none_or(|h| asset_server.is_loaded_with_dependencies(h.id())),
    );

    if let Some(loading) = textures.iter().next() {
        done += 1;
        let default_handle = Handle::<Image>::default();
        done += loading
            .texture_handles
            .iter()
            .filter(|h| **h == default_handle || asset_server.is_loaded_with_dependencies(h.id()))
            .count() as u32;
    }

    Progress { done, total }
}
