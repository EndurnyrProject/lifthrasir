use crate::{
    core::state::GameState,
    domain::{
        entities::{components::EntityName, hover::EntityHoverEntered, registry::EntityRegistry},
        system_sets::EntityInteractionSystems,
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::commands::NameRequested;
use net_contract::events::EntityNamed;

#[auto_observer(plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin)]
pub fn name_request_observer(
    trigger: On<EntityHoverEntered>,
    mut name_requests: MessageWriter<NameRequested>,
) {
    name_requests.write(NameRequested {
        gid: trigger.event().entity_id,
    });
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
