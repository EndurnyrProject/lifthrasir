use bevy_auto_plugin::prelude::*;

/// Client track selection and network sound adaptation. Shared playback is composed by the binary.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AudioPlugin;
