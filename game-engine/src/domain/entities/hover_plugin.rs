use super::hover::{EntityHoverEntered, EntityHoverExited, HoverConfig, Hoverable, HoveredEntity};
use super::hover_system::{entity_hover_detection_system, update_entity_bounds_system};
use super::name_request_system::{name_request_system, name_response_handler_system};
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityHoverSystemSet;

/// Plugin that handles entity hovering and name requests
///
/// This plugin sets up the complete entity hover system including:
/// - Event registration (EntityHoverEntered, EntityHoverExited)
/// - Component registration (HoveredEntity, Hoverable)
/// - System scheduling with proper ordering
///
/// # System Flow
///
/// 1. `update_entity_bounds_system` - Calculates screen-space bounds for entities
/// 2. `entity_hover_detection_system` - Detects mouse hover, emits enter/exit events
/// 3. `name_request_system` - Sends CZ_REQNAME2 packets when hovering entities
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
        app.add_message::<EntityHoverEntered>()
            .add_message::<EntityHoverExited>();

        app.register_type::<HoveredEntity>()
            .register_type::<Hoverable>();

        app.init_resource::<HoverConfig>();

        app.add_systems(
            Update,
            (
                update_entity_bounds_system,
                entity_hover_detection_system,
                name_request_system,
                name_response_handler_system,
            )
                .chain()
                .in_set(EntityHoverSystemSet),
        );

        info!("EntityHoverPlugin initialized");
    }
}
