use crate::{
    core::{GameSettings, GameState, MapState},
    domain::world::systems::{
        extract_map_from_hierarchical_assets, setup_hierarchical_map_loading,
    },
    infrastructure::assets::loading_states::AssetLoadingState,
};
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<MapState>()
            .init_resource::<GameSettings>()
            .add_systems(
                OnEnter(AssetLoadingState::Ready),
                setup_hierarchical_map_loading,
            )
            .add_systems(
                Update,
                extract_map_from_hierarchical_assets.run_if(in_state(AssetLoadingState::Ready)),
            );
    }
}
