use crate::{
    core::{GameSettings, GameState, MapState},
    domain::world::systems::{extract_map_from_grf, setup_grf_map_loading},
};
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<MapState>()
            .init_resource::<GameSettings>()
            .add_systems(Startup, setup_grf_map_loading)
            .add_systems(Update, extract_map_from_grf);
    }
}
