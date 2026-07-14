use crate::{
    core::state::GameState,
    domain::{
        entities::{
            components::{EntityName, GuildIdentity, SpawnGuildIdentityKnown},
            hover::EntityHoverEntered,
            registry::EntityRegistry,
        },
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
    spawn_guild_identities: Query<Option<&GuildIdentity>, With<SpawnGuildIdentityKnown>>,
) {
    for event in name_events.read() {
        let Some(entity) = entity_registry.get_entity(event.gid) else {
            continue;
        };

        if commands.get_entity(entity).is_err() {
            continue;
        }

        let guild_name = match spawn_guild_identities.get(entity) {
            Ok(Some(identity)) => identity.guild_name.clone(),
            Ok(None) => String::new(),
            Err(_) => event.guild_name.clone(),
        };
        let entity_name = EntityName::with_full_details(
            event.name.clone(),
            event.party_name.clone(),
            guild_name,
            event.position_name.clone(),
        );
        commands.entity(entity).insert(entity_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::components::{GuildIdentity, SpawnGuildIdentityKnown};

    #[test]
    fn stale_name_response_keeps_newer_spawn_guild_identity() {
        let mut app = App::new();
        app.add_message::<EntityNamed>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, name_response_handler_system);

        let entity = app
            .world_mut()
            .spawn((
                GuildIdentity {
                    guild_id: 77,
                    guild_name: "New Guild".into(),
                    emblem_id: 10,
                },
                SpawnGuildIdentityKnown,
            ))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(150_001, entity);
        app.world_mut().write_message(EntityNamed {
            gid: 150_001,
            name: "Alice".into(),
            party_name: String::new(),
            guild_name: "Old Guild".into(),
            position_name: "Old Position".into(),
        });

        app.update();

        let entity_ref = app.world().entity(entity);
        let identity = entity_ref.get::<GuildIdentity>().unwrap();
        let name = entity_ref.get::<EntityName>().unwrap();
        assert_eq!(identity.guild_id, 77);
        assert_eq!(identity.guild_name, "New Guild");
        assert_eq!(identity.emblem_id, 10);
        assert_eq!(name.guild_name.as_deref(), Some("New Guild"));
        assert_eq!(name.position_name.as_deref(), Some("Old Position"));
    }
}
