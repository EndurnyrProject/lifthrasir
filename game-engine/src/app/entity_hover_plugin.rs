use bevy_auto_plugin::prelude::*;

/// Entity Hover Plugin
///
/// This plugin handles entity hover detection and name requests.
///
/// Registered resource:
/// - HoverConfig
///
/// Registered observer:
/// - name_request_observer
///
/// Registered systems (chained in EntityHoverSystems set):
/// - update_entity_bounds_system
/// - entity_hover_detection_system
/// - name_response_handler_system
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct EntityHoverDomainPlugin;
