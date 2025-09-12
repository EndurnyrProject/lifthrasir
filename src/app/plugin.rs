use crate::assets::RoAssetsPlugin;
use crate::systems::{camera_movement_system, setup};
use bevy::prelude::*;
// Animation system available: use crate::systems::animate_sprites;

pub struct LifthrasirPlugin;

impl Plugin for LifthrasirPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoAssetsPlugin)
            // RsmAnimationPlugin removed - now using bevy_tween for animations
            .add_systems(Startup, setup)
            .add_systems(Update, camera_movement_system);
        // .add_systems(Update, animate_sprites); // Ready for map entities
    }
}
