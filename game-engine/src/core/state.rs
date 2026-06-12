use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_init_state, auto_register_state_type};

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::app::plugin::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::app::plugin::LifthrasirPlugin)]
pub enum GameState {
    #[default]
    Loading,
    Login,
    Connecting,
    ServerSelection,
    CharacterSelection,
    InGame,
    Paused,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::app::plugin::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::app::plugin::LifthrasirPlugin)]
pub enum MapState {
    #[default]
    NotLoaded,
    Loading,
    Loaded,
}
