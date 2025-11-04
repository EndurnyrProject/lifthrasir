use crate::{
    domain::entities::{
        components::{EntityName, NetworkEntity},
        hover::EntityHoverEntered,
    },
    infrastructure::networking::{
        client::ZoneServerClient,
        protocol::zone::{EntityNameAllReceived, EntityNameReceived},
    },
};
use bevy::prelude::*;

pub fn name_request_system(
    mut client: Option<ResMut<ZoneServerClient>>,
    mut hover_entered_events: MessageReader<EntityHoverEntered>,
    entity_query: Query<&EntityName>,
    network_entity_query: Query<(Entity, &NetworkEntity)>,
) {
    let Some(ref mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    for event in hover_entered_events.read() {
        let Some((entity, _)) = network_entity_query.iter().find(|(_, ne)| ne.aid == event.entity_id) else {
            continue;
        };

        if entity_query.get(entity).is_ok() {
            continue;
        }

        info!("📤 Sending CZ_REQNAME2 packet for entity ID: {}", event.entity_id);

        if let Err(e) = client.request_entity_name(event.entity_id) {
            error!("❌ Failed to send name request for entity {}: {:?}", event.entity_id, e);
        }
    }
}

pub fn name_response_handler_system(
    mut commands: Commands,
    mut basic_name_events: MessageReader<EntityNameReceived>,
    mut full_name_events: MessageReader<EntityNameAllReceived>,
    network_entity_query: Query<(Entity, &NetworkEntity)>,
) {
    for event in basic_name_events.read() {
        let gid_to_find = event.char_id;
        let Some((entity, _)) = network_entity_query
            .iter()
            .find(|(_, ne)| ne.gid == gid_to_find) else {
            warn!(
                "Received name for GID {} but no NetworkEntity found - name: {}",
                gid_to_find, event.name
            );
            continue;
        };

        let entity_name = EntityName::new(event.name.clone());
        commands.entity(entity).insert(entity_name);

        debug!("Added entity name: {} (GID: {})", event.name, gid_to_find);
    }

    for event in full_name_events.read() {
        let gid_to_find = event.gid;
        let Some((entity, _)) = network_entity_query
            .iter()
            .find(|(_, ne)| ne.gid == gid_to_find) else {
            warn!(
                "Received full name for GID {} but no NetworkEntity found - name: {}",
                gid_to_find, event.name
            );
            continue;
        };

        let entity_name = EntityName::with_full_details(
            event.name.clone(),
            event.party_name.clone(),
            event.guild_name.clone(),
            event.position_name.clone(),
        );
        commands.entity(entity).insert(entity_name);

        debug!(
            "Added entity full details: {} (GID: {}), Party: {}, Guild: {}, Position: {}",
            event.name, gid_to_find, event.party_name, event.guild_name, event.position_name
        );
    }
}

