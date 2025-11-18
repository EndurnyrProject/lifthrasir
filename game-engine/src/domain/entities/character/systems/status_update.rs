use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;

use crate::{
    domain::entities::{
        character::components::status::{CharacterStatus, StatusParameter},
        character::events::StatusParameterChanged,
        markers::LocalPlayer,
        registry::EntityRegistry,
    },
    infrastructure::networking::protocol::zone::handlers::ParameterChanged,
};

#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(after = crate::domain::entities::spawning::systems::spawn_network_entity_system)
)]
pub fn update_character_status_system(
    mut param_events: MessageReader<ParameterChanged>,
    mut status_changed_events: MessageWriter<StatusParameterChanged>,
    entity_registry: Res<EntityRegistry>,
    mut query: Query<&mut CharacterStatus, With<LocalPlayer>>,
) {
    if query.single().is_err() {
        debug!("Status update system: waiting for LocalPlayer entity to spawn");
        return;
    }

    let events: Vec<_> = param_events.read().collect();
    if !events.is_empty() {
        info!("Processing {} parameter change events", events.len());
    }

    for event in events {
        let Some(param) = StatusParameter::from_var_id(event.var_id) else {
            warn!(
                "Unknown status parameter ID: 0x{:04X} (value: {})",
                event.var_id, event.value
            );
            continue;
        };

        let Ok(mut status) = query.single_mut() else {
            warn!(
                "No local player entity found to update status parameter: {:?}",
                param
            );
            continue;
        };

        let old_value = status.get_param(param);
        status.update_param(param, event.value);

        debug!(
            "Status parameter updated: {:?} ({}) = {} (was: {})",
            param,
            param.name(),
            event.value,
            old_value
        );

        if let Some(entity) = entity_registry.local_player_entity() {
            status_changed_events.write(StatusParameterChanged {
                entity,
                parameter: param,
                new_value: event.value,
                old_value: Some(old_value),
            });
        }
    }
}
