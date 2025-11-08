use crate::{
    domain::entities::{
        components::EntityName, hover::EntityHoverEntered, registry::EntityRegistry,
    },
    infrastructure::networking::{
        client::ZoneServerClient,
        protocol::zone::{EntityNameAllReceived, EntityNameReceived},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[auto_observer(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
pub fn name_request_observer(
    trigger: On<EntityHoverEntered>,
    mut client: Option<ResMut<ZoneServerClient>>,
) {
    let Some(ref mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    };

    let event = trigger.event();

    info!(
        "Sending name request for entity ID {} (AID from event)",
        event.entity_id
    );

    if let Err(e) = client.request_entity_name(event.entity_id) {
        error!(
            "❌ Failed to send name request for entity {}: {:?}",
            event.entity_id, e
        );
    }
}

#[auto_add_system(
    plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin,
    schedule = Update,
    config(after = crate::domain::entities::hover_system::entity_hover_detection_system)
)]
pub fn name_response_handler_system(
    mut commands: Commands,
    mut basic_name_events: MessageReader<EntityNameReceived>,
    mut full_name_events: MessageReader<EntityNameAllReceived>,
    entity_registry: Res<EntityRegistry>,
) {
    for event in basic_name_events.read() {
        let aid = event.char_id;

        let Some(entity) = entity_registry.get_entity(aid) else {
            debug!(
                "Received name for AID {} but entity no longer exists (may have despawned) - name: {}",
                aid, event.name
            );
            continue;
        };

        if commands.get_entity(entity).is_err() {
            debug!(
                "Received name for AID {} but entity {:?} was already despawned - name: {}",
                aid, entity, event.name
            );
            continue;
        }

        let entity_name = EntityName::new(event.name.clone());
        commands.entity(entity).insert(entity_name);
    }

    for event in full_name_events.read() {
        // Note: event.gid is misleadingly named - it contains AID
        let aid = event.gid;

        let Some(entity) = entity_registry.get_entity(aid) else {
            debug!(
                "Received full name for AID {} but entity no longer exists (may have despawned) - name: {}",
                aid, event.name
            );
            continue;
        };

        if commands.get_entity(entity).is_err() {
            debug!(
                "Received full name for AID {} but entity {:?} was already despawned - name: {}",
                aid, entity, event.name
            );
            continue;
        }

        let entity_name = EntityName::with_full_details(
            event.name.clone(),
            event.party_name.clone(),
            event.guild_name.clone(),
            event.position_name.clone(),
        );
        commands.entity(entity).insert(entity_name);
    }
}
