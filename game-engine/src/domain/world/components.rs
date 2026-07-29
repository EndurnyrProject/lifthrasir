use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use bevy::prelude::*;

/// The current map's heightfield, decoded from the map glb and added to
/// `Assets<RoAltitudeAsset>` by the map adoption observer.
///
/// Lifecycle mirrors `CurrentMapPathfindingGrid`: inserted per map load,
/// overwritten on a map switch, never removed.
#[derive(Resource)]
pub struct CurrentMapAltitude(pub Handle<RoAltitudeAsset>);
