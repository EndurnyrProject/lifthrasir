use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::{auto_init_state, auto_register_state_type};

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

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::app::plugin::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::app::plugin::LifthrasirPlugin)]
pub enum LoginState {
    #[default]
    LoginForm,
    Connecting,
    Authenticating,
    Failed,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::app::plugin::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::app::plugin::LifthrasirPlugin)]
pub enum NetworkState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::app::plugin::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::app::plugin::LifthrasirPlugin)]
pub enum CharacterScreenState {
    #[default]
    Connecting, // Connecting to character server
    CharacterList,       // Displaying character list
    CharacterDetails,    // Viewing selected character details
    CharacterCreation,   // Character creation form
    DeletionConfirm,     // Confirming character deletion
    TransitioningToGame, // Loading into game world
}
