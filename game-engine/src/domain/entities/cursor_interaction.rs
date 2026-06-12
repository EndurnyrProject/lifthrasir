use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::input::{
    cursor::CursorType, events::CursorChangeRequest, terrain_raycast::TerrainRaycastCache,
};

use super::{
    hover::{EntityHoverEntered, EntityHoverExited},
    markers::{Mob, Npc},
};

#[auto_observer(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
pub fn on_entity_hover_entered(
    trigger: On<EntityHoverEntered>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
    mobs: Query<(), With<Mob>>,
    npcs: Query<(), With<Npc>>,
) {
    let event = trigger.event();

    let cursor_type = if mobs.contains(event.entity) {
        CursorType::Attack
    } else if npcs.contains(event.entity) {
        CursorType::Talk
    } else {
        CursorType::Default
    };

    debug!(
        "Entity hover entered: AID={}, cursor={:?}",
        event.entity_id, cursor_type
    );

    cursor_messages.write(CursorChangeRequest::new(cursor_type));
}

#[auto_observer(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
pub fn on_entity_hover_exited(
    _trigger: On<EntityHoverExited>,
    cache: Res<TerrainRaycastCache>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    let cursor_type = if cache.is_walkable {
        CursorType::Default
    } else {
        CursorType::Impossible
    };

    debug!("Entity hover exited, cursor back to {:?}", cursor_type);
    cursor_messages.write(CursorChangeRequest::new(cursor_type));
}
