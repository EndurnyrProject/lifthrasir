use bevy::prelude::*;
use crate::assets::RoAssetsPlugin;
use crate::systems::setup;
// Animation system available: use crate::systems::animate_sprites;

pub struct LifthrasirPlugin;

impl Plugin for LifthrasirPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoAssetsPlugin)
            .add_systems(Startup, setup);
            // .add_systems(Update, animate_sprites); // Ready for map entities
    }
}