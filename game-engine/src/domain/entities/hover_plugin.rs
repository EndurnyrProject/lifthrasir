use crate::app::entity_hover_plugin::EntityHoverDomainPlugin;
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityHoverSystems;

/// Entity Hover Plugin (Wrapper)
///
/// Adds the EntityHoverDomainPlugin (auto-plugin).
///
/// # System Flow
///
/// 1. `update_entity_bounds_system` - Calculates screen-space bounds for entities
/// 2. `entity_hover_detection_system` - Detects mouse hover, triggers observer events
/// 3. `name_request_observer` - Observer that sends CZ_REQNAME2 packets when hovering entities
/// 4. Server responds with ZC_ACK_REQNAME or ZC_ACK_REQNAMEALL
/// 5. `name_response_handler_system` - Adds EntityName component to entities
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

        info!("EntityHoverPlugin initialized");
    }
}
