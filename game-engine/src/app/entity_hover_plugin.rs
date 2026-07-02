use bevy_auto_plugin::prelude::*;

/// Entity Hover Plugin
///
/// Hover detection now runs through the `bevy_picking` observers in
/// `entities::picking`, which set `CurrentlyHoveredEntity` and trigger
/// `EntityHoverEntered`/`EntityHoverExited`. This plugin owns the name-request
/// side of hover.
///
/// Registered resource:
/// - CurrentlyHoveredEntity
///
/// Registered observer:
/// - name_request_observer
///
/// Registered system:
/// - name_response_handler_system
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct EntityHoverDomainPlugin;
