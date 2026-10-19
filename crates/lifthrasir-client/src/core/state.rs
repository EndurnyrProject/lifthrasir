use bevy::prelude::*;
use bevy_auto_plugin::prelude::{AutoPlugin, auto_init_state, auto_register_state_type};

/// Root auto-plugin; the app-wide states below register themselves onto it.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct LifthrasirPlugin;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
#[auto_init_state(plugin = crate::core::state::LifthrasirPlugin)]
#[auto_register_state_type(plugin = crate::core::state::LifthrasirPlugin)]
pub enum GameState {
    #[default]
    Bootstrapping,
    Loading,
    Login,
    Connecting,
    ServerSelection,
    CharacterSelection,
    CharacterCreation,
    InGame,
}
