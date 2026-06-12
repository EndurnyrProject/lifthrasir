use bevy_auto_plugin::prelude::AutoPlugin;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct RenderingPlugin;
