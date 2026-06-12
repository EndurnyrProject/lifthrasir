use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;

/// Resource to store cursor position from native input events
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct ForwardedCursorPosition {
    pub position: Option<Vec2>,
}

/// Resource to store mouse click position for terrain clicks for player movement
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct ForwardedMouseClick {
    pub position: Option<Vec2>,
}
