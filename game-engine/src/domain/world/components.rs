use crate::infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset, RoWorldAsset};
use bevy::prelude::*;

#[derive(Component)]
pub struct MapLoader {
    pub ground: Handle<RoGroundAsset>,
    pub altitude: Option<Handle<RoAltitudeAsset>>,
    pub world: Option<Handle<RoWorldAsset>>,
}
