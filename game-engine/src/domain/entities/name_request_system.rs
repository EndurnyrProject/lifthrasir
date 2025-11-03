use crate::{
    domain::entities::{
        components::NetworkEntity,
        hover::EntityHoverEntered,
        name_cache::{CachedEntityName, EntityNameCache},
    },
    infrastructure::networking::{
        client::ZoneServerClient,
        protocol::zone::{EntityNameAllReceived, EntityNameReceived},
    },
};
use bevy::prelude::*;

pub fn name_request_system(
    mut cache: ResMut<EntityNameCache>,
    mut client: Option<ResMut<ZoneServerClient>>,
    mut hover_entered_events: MessageReader<EntityHoverEntered>,
) {
    let Some(ref mut client) = client else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    for event in hover_entered_events.read() {
        info!("📨 Received EntityHoverEntered for entity ID: {}", event.entity_id);

        if let Some(cached) = cache.get(event.entity_id) {
            info!("✅ Name already cached: {}", cached.name);
            continue;
        }

        if !cache.can_request(event.entity_id) {
            debug!("⏳ Request throttled for entity ID: {}", event.entity_id);
            continue;
        }

        info!("📤 Sending CZ_REQNAME2 packet for entity ID: {}", event.entity_id);

        if let Err(e) = client.request_entity_name(event.entity_id) {
            error!("❌ Failed to send name request for entity {}: {:?}", event.entity_id, e);
            continue;
        }

        cache.mark_requested(event.entity_id);
    }
}

pub fn name_response_handler_system(
    mut cache: ResMut<EntityNameCache>,
    mut basic_name_events: MessageReader<EntityNameReceived>,
    mut full_name_events: MessageReader<EntityNameAllReceived>,
    network_entity_query: Query<&NetworkEntity>,
) {
    for event in basic_name_events.read() {
        let gid_to_find = event.char_id;
        let aid = network_entity_query
            .iter()
            .find(|ne| ne.gid == gid_to_find)
            .map(|ne| ne.aid);

        let Some(aid) = aid else {
            warn!(
                "Received name for GID {} but no NetworkEntity found - name: {}",
                gid_to_find, event.name
            );
            continue;
        };

        let cached_name = CachedEntityName::new(event.name.clone());
        cache.insert(aid, cached_name);

        debug!("Cached entity name: {} (AID: {}, GID: {})", event.name, aid, gid_to_find);
    }

    for event in full_name_events.read() {
        let gid_to_find = event.gid;
        let aid = network_entity_query
            .iter()
            .find(|ne| ne.gid == gid_to_find)
            .map(|ne| ne.aid);

        let Some(aid) = aid else {
            warn!(
                "Received full name for GID {} but no NetworkEntity found - name: {}",
                gid_to_find, event.name
            );
            continue;
        };

        let cached_name = CachedEntityName::with_full_details(
            event.name.clone(),
            event.party_name.clone(),
            event.guild_name.clone(),
            event.position_name.clone(),
        );
        cache.insert(aid, cached_name);

        debug!(
            "Cached entity full details: {} (AID: {}, GID: {}), Party: {}, Guild: {}, Position: {}",
            event.name, aid, gid_to_find, event.party_name, event.guild_name, event.position_name
        );
    }
}

pub fn cache_cleanup_system(
    mut cache: ResMut<EntityNameCache>,
    mut cleanup_timer: ResMut<CacheCleanupTimer>,
    time: Res<Time>,
) {
    cleanup_timer.0.tick(time.delta());

    if cleanup_timer.0.just_finished() {
        cache.cleanup_expired();
        debug!("Entity name cache cleanup completed");
    }
}

#[derive(Resource)]
pub struct CacheCleanupTimer(pub Timer);
