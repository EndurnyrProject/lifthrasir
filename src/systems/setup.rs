use bevy::prelude::*;

pub fn setup(mut commands: Commands) {
    info!("🚀 Lifthrasir client ready");
    
    // Spawn camera
    commands.spawn(Camera2d);
}