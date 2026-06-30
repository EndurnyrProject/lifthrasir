use bevy_auto_plugin::prelude::*;

pub mod commands;
pub mod dto;
pub mod events;
pub mod state;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct NetContractPlugin;
