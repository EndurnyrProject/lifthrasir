use crate::app::movement_plugin::MovementDomainPlugin;
use crate::infrastructure::networking::protocol::zone::handlers::movement_handlers::{
    MovementConfirmedByServer, MovementStoppedByServer,
};
use bevy::prelude::*;

/// Movement Plugin (Wrapper)
///
/// Composes movement functionality with proper dependency order:
/// 1. Network protocol messages (infrastructure-level)
/// 2. MovementDomainPlugin (auto-plugin with observers and systems)
///
/// # System Flow
///
/// 1. `send_movement_requests_observer` - Consumes MovementRequested, sends to server
/// 2. Server validates and responds with ZC_NOTIFY_PLAYERMOVE
/// 3. `handle_movement_confirmed_system` - Starts interpolation, updates direction
/// 4. `interpolate_movement_system` - Runs every frame to move character smoothly
/// 5. `handle_server_stop_system` - Cleanup when movement completes
/// 6. `update_entity_altitude_system` - Updates entity height based on terrain
///
/// # Integration
///
/// ```ignore
/// app.add_plugins(MovementPlugin);
/// ```
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        // Register network protocol messages (infrastructure-level)
        app.add_message::<MovementConfirmedByServer>()
            .add_message::<MovementStoppedByServer>();

        // Add movement domain plugin (auto-plugin with observers and systems)
        app.add_plugins(MovementDomainPlugin);

        info!("MovementPlugin initialized");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_plugin_builds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(MovementPlugin);

        // Add required EntityRegistry resource for movement systems
        app.init_resource::<crate::domain::entities::registry::EntityRegistry>();

        // Plugin builds successfully - actual message registration would require more setup
        // This is a smoke test to ensure the plugin can be added without panic
        app.update();
    }
}
