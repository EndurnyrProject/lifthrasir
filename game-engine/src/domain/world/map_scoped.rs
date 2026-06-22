use bevy::prelude::Component;

/// Marker for entities whose lifetime is the current map.
///
/// Every map-content entity (terrain, models, lights, water, the map loader,
/// and remote network entities) carries this so a single despawn system can
/// tear them all down on map exit. The local player is NOT map-scoped.
#[derive(Component, Debug)]
pub struct MapScoped;
