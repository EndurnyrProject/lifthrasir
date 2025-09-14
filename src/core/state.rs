use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    InGame,
    Paused,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum MapState {
    #[default]
    NotLoaded,
    Loading,
    Loaded,
}
