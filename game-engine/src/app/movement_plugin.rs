use bevy_auto_plugin::prelude::*;

/// Movement Plugin
///
/// This auto-plugin handles movement observers and systems.
/// Network message registration is handled by the MovementPlugin wrapper
/// in domain/entities/movement/plugin.rs.
///
/// Registered observers:
/// - send_movement_requests_observer
/// - handle_movement_stopped_observer
///
/// Registered systems (chained in Update):
/// - handle_movement_confirmed_system
/// - interpolate_movement_system
/// - handle_server_stop_system
/// - update_entity_altitude_system
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct MovementDomainPlugin;
