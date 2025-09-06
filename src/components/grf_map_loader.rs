use crate::assets::loaders::GrfAsset;
use bevy::prelude::*;

#[derive(Component)]
pub struct GrfMapLoader {
    pub grf_handle: Handle<GrfAsset>,
    pub map_name: String,
    pub loaded: bool,
}

impl GrfMapLoader {
    pub fn new(grf_handle: Handle<GrfAsset>, map_name: String) -> Self {
        Self {
            grf_handle,
            map_name,
            loaded: false,
        }
    }
}

#[derive(Component)]
pub struct ExtractedMapFiles {
    pub ground_data: Option<Vec<u8>>,
    pub altitude_data: Option<Vec<u8>>,
    pub world_data: Option<Vec<u8>>,
}

impl ExtractedMapFiles {
    pub fn new() -> Self {
        Self {
            ground_data: None,
            altitude_data: None,
            world_data: None,
        }
    }
}
