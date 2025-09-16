use crate::presentation::rendering::*;
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::AutoPlugin;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct RenderingPlugin;
