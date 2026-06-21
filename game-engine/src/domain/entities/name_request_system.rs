use crate::{
    core::state::GameState,
    domain::{
        entities::{components::EntityName, hover::EntityHoverEntered, registry::EntityRegistry},
        system_sets::EntityInteractionSystems,
    },
    infrastructure::networking::{client::ZoneServerClient, zone_messages::EntityNamed},
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
    config(
        in_set = EntityInteractionSystems::Naming,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn name_response_handler_system(
    mut commands: Commands,
    mut name_events: MessageReader<EntityNamed>,
    entity_registry: Res<EntityRegistry>,
) {
    for event in name_events.read() {
        let Some(entity) = entity_registry.get_entity(event.gid) else {
            continue;
        };

        if commands.get_entity(entity).is_err() {
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
