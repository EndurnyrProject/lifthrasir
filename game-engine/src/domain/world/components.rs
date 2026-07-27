use crate::infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
use bevy::prelude::*;

#[derive(Component)]
pub struct MapLoader {
    pub ground: Handle<RoGroundAsset>,
    pub altitude: Option<Handle<RoAltitudeAsset>>,
    pub world: Option<Handle<RoWorldAsset>>,
}

/// The current map's heightfield, published by whichever map path loaded it:
/// the native `.gat` handle from [`extract_map_from_unified_assets`], or the
/// glb's decoded altitude added to `Assets` by the `map-gltf` adopt observer.
/// Every height consumer outside `domain::world` reads this instead of
/// [`MapLoader`], which only the native loading orchestration owns.
///
/// Lifecycle mirrors `CurrentMapPathfindingGrid`: inserted per map load,
/// overwritten on a map switch, never removed. The asset itself may still be
/// loading, so consumers must tolerate a missing `Assets` entry.
///
/// [`extract_map_from_unified_assets`]: crate::domain::world::systems::extract_map_from_unified_assets
#[derive(Resource)]
pub struct CurrentMapAltitude(pub Handle<RoAltitudeAsset>);
