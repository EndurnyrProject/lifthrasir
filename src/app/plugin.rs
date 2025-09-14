use crate::domain::camera::controller::camera_movement_system;
use crate::infrastructure::assets::loaders::RoAssetsPlugin;
use crate::presentation::rendering::terrain::setup;
use bevy::prelude::*;
// Animation system available: use crate::systems::animate_sprites;

pub struct LifthrasirPlugin;

impl Plugin for LifthrasirPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoAssetsPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, camera_movement_system);
        // .add_systems(Update, animate_sprites); // Ready for map entities
    }
}
