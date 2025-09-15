use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Login,
    Connecting, // New state for network connection
    ServerSelection,
    CharacterSelection,
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
    Authenticating,
    Failed,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum NetworkState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}
