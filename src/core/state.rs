use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Login,
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

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum LoginState {
    #[default]
    LoginForm,
    Connecting,
}
