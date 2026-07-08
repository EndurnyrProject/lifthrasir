use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use crate::domain::entities::{
    character::components::status::{CharacterStatus, StatusParameter},
    character::events::StatusParameterChanged,
    markers::LocalPlayer,
    registry::EntityRegistry,
};
use net_contract::events::ParamChanged;

/// Holds parameter changes that arrived before the `LocalPlayer` entity spawned.
///
/// On login the server sends the initial stat params (HP, max HP, ...) while the
/// client is still loading map/ground assets, so the `LocalPlayer` does not exist
/// yet. Bevy messages only live for two frames, so those initial params would be
/// dropped before there is a `CharacterStatus` to apply them to — leaving the
/// default HP of 100 until the next server-sent change. Buffering here keeps them
/// until the entity is ready.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct PendingStatusParams(Vec<ParamChanged>);

#[auto_add_system(
    plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin,
    schedule = Update,
    config(after = crate::domain::entities::spawning::systems::spawn_network_entity_system)
)]
pub fn update_character_status_system(
    mut param_events: MessageReader<ParamChanged>,
    mut status_changed_events: MessageWriter<StatusParameterChanged>,
    entity_registry: Res<EntityRegistry>,
    mut pending: ResMut<PendingStatusParams>,
    mut query: Query<&mut CharacterStatus, With<LocalPlayer>>,
) {
    // Always drain the message buffer so params are never lost to the two-frame
    // window; hold them until the LocalPlayer exists.
    pending.0.extend(param_events.read().cloned());

    if query.single().is_err() {
        if !pending.0.is_empty() {
            debug!(
                "Status update system: buffering {} param change(s) until LocalPlayer spawns",
                pending.0.len()
            );
        }
        return;
    }

    let events: Vec<_> = pending.0.drain(..).collect();
    if !events.is_empty() {
        debug!("Processing {} parameter change events", events.len());
    }

    for event in events {
        let var_id = event.var as u16;
        let value = event.value as u32;

        let Some(param) = StatusParameter::from_var_id(var_id) else {
            warn!(
                "Unknown status parameter ID: 0x{:04X} (value: {})",
                var_id, value
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
        status.update_param(param, value);

        debug!(
            "Status parameter updated: {:?} ({}) = {} (was: {})",
            param,
            param.name(),
            value,
            old_value
        );

        if let Some(entity) = entity_registry.local_player_entity() {
            status_changed_events.write(StatusParameterChanged {
                entity,
                parameter: param,
                new_value: value,
                old_value: Some(old_value),
            });
        }
    }
}
