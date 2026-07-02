use crate::app::entity_hover_plugin::EntityHoverDomainPlugin;
use bevy::prelude::*;

/// Entity Hover Plugin (Wrapper)
///
/// Adds the EntityHoverDomainPlugin (auto-plugin).
///
/// # System Flow
///
/// 1. `entities::picking` observers detect hover via `bevy_picking` and trigger
///    `EntityHoverEntered`
/// 2. `name_request_observer` - Observer that sends CZ_REQNAME2 packets when hovering entities
/// 3. Server responds with ZC_ACK_REQNAME or ZC_ACK_REQNAMEALL
/// 4. `name_response_handler_system` - Adds EntityName component to entities
///
/// # Integration
///
/// ```ignore
/// app.add_plugins(EntityHoverPlugin);
/// ```
pub struct EntityHoverPlugin;

impl Plugin for EntityHoverPlugin {
    fn build(&self, app: &mut App) {
        // Add entity hover domain plugin (auto-plugin with resource, observer, and systems)
        app.add_plugins(EntityHoverDomainPlugin);

        debug!("EntityHoverPlugin initialized");
    }
}
