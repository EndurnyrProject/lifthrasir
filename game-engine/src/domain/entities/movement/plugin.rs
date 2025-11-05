use super::events::{MovementConfirmed, MovementRequested, MovementStopped};
use super::systems::{
    handle_movement_confirmed_system, handle_movement_stopped_system, handle_server_stop_system,
    interpolate_movement_system, send_movement_requests_system, update_entity_altitude_system,
};
use crate::infrastructure::networking::protocol::zone::handlers::movement_handlers::{
    MovementConfirmedByServer, MovementStoppedByServer,
};
use bevy::prelude::*;

/// Plugin that handles all character movement functionality
///
/// This plugin sets up the complete movement system including:
/// - Event registration (MovementRequested, MovementConfirmed, MovementStopped)
/// - System scheduling with proper ordering
///
/// # System Flow
///
/// 1. `send_movement_requests_system` - Consumes MovementRequested, sends to server
/// 2. Server validates and responds with ZC_NOTIFY_PLAYERMOVE
/// 3. `handle_movement_confirmed_system` - Starts interpolation, updates direction
/// 4. `interpolate_movement_system` - Runs every frame to move character smoothly
/// 5. `handle_movement_stopped_system` - Cleanup when movement completes
///
/// # Integration
///
/// ```ignore
/// app.add_plugins(MovementPlugin);
/// ```
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        // Register events (domain-level)
        app.add_message::<MovementRequested>()
            .add_message::<MovementConfirmed>()
            .add_message::<MovementStopped>();

        // Register network protocol messages (infrastructure-level)
        app.add_message::<MovementConfirmedByServer>()
            .add_message::<MovementStoppedByServer>();

        // Add systems with proper scheduling to avoid event access conflicts
        // Systems that emit events run in Update
        app.add_systems(
            Update,
            (
                send_movement_requests_system,
                handle_movement_confirmed_system,
                interpolate_movement_system,
                handle_server_stop_system,
                update_entity_altitude_system,
            )
                .chain(),
        );

        // Systems that read events run in PostUpdate to avoid conflicts
        // This allows event writers in Update to complete before readers process them
        app.add_systems(PostUpdate, handle_movement_stopped_system);

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
