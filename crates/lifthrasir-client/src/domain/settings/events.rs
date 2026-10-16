use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Requests that committed settings resources be applied to the live world.
#[derive(Message, Debug, Clone, Copy, Reflect)]
#[reflect(Debug)]
#[auto_add_message(plugin = super::SettingsRuntimePlugin)]
pub struct ApplySettings;
