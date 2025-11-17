use bevy::prelude::*;
use game_engine::domain::entities::{
    components::{EntityName, NetworkEntity},
    hover::HoveredEntity,
    sprite_rendering::components::SpriteObjectTree,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct EntityNameEvent {
    entity_id: u32,
    name: String,
    party_name: Option<String>,
    guild_name: Option<String>,
    position_name: Option<String>,
    screen_x: f32,
    screen_y: f32,
}

#[derive(Serialize, Clone)]
struct EmptyPayload {}

type HoveredEntityQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static NetworkEntity, &'static SpriteObjectTree, &'static EntityName),
    (With<HoveredEntity>, Changed<EntityName>),
>;

pub fn on_entity_name_added_to_hovered(
    trigger: On<Add, EntityName>,
    app_handle: NonSend<AppHandle>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    entity_query: Query<(&NetworkEntity, &SpriteObjectTree), With<HoveredEntity>>,
    name_query: Query<&EntityName>,
    sprite_query: Query<&GlobalTransform>,
) {
    let entity = trigger.entity;

    info!("Observer fired: EntityName added to entity {:?}", entity);

    let Ok((network_entity, sprite_tree)) = entity_query.get(entity) else {
        info!(
            "Entity {:?} received EntityName but is not hovered (no HoveredEntity component)",
            entity
        );
        return;
    };

    let Ok(entity_name) = name_query.get(entity) else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };

    let Ok(sprite_transform) = sprite_query.get(sprite_tree.root) else {
        return;
    };

    let world_pos = sprite_transform.translation();

    let Some(ndc) = camera.world_to_ndc(camera_transform, world_pos) else {
        return;
    };

    let screen_x = (ndc.x + 1.0) * 0.5 * viewport_size.x;
    let screen_y = (1.0 - ndc.y) * 0.5 * viewport_size.y;

    let event = EntityNameEvent {
        entity_id: network_entity.aid,
        name: entity_name.name.clone(),
        party_name: entity_name.party_name.clone(),
        guild_name: entity_name.guild_name.clone(),
        position_name: entity_name.position_name.clone(),
        screen_x,
        screen_y,
    };

    info!(
        "Emitting tooltip for entity {:?}, name: {}, AID: {}",
        entity, entity_name.name, network_entity.aid
    );

    if let Err(e) = app_handle.emit("entity-name-show", event) {
        error!("Failed to emit entity-name-show event: {:?}", e);
    }
}

pub fn emit_entity_unhover(
    app_handle: NonSend<AppHandle>,
    hovered_query: Query<Entity, With<HoveredEntity>>,
    mut previous_hovered: Local<Option<Entity>>,
) {
    let current_hovered = hovered_query.iter().next();

    match (current_hovered, *previous_hovered) {
        (Some(entity), prev) if prev != Some(entity) => {
            *previous_hovered = Some(entity);
        }
        (None, Some(_)) => {
            *previous_hovered = None;

            if let Err(e) = app_handle.emit("entity-name-hide", EmptyPayload {}) {
                error!("Failed to emit entity-name-hide event: {:?}", e);
            }
        }
        _ => {}
    }
}

pub fn emit_hovered_entity_name(
    app_handle: NonSend<AppHandle>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    entity_query: HoveredEntityQuery,
    sprite_query: Query<&GlobalTransform>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };

    for (entity, network_entity, sprite_tree, entity_name) in entity_query.iter() {
        let Ok(sprite_transform) = sprite_query.get(sprite_tree.root) else {
            continue;
        };

        let world_pos = sprite_transform.translation();

        let Some(ndc) = camera.world_to_ndc(camera_transform, world_pos) else {
            continue;
        };

        let screen_x = (ndc.x + 1.0) * 0.5 * viewport_size.x;
        let screen_y = (1.0 - ndc.y) * 0.5 * viewport_size.y;

        let event = EntityNameEvent {
            entity_id: network_entity.aid,
            name: entity_name.name.clone(),
            party_name: entity_name.party_name.clone(),
            guild_name: entity_name.guild_name.clone(),
            position_name: entity_name.position_name.clone(),
            screen_x,
            screen_y,
        };

        info!(
            "Emitting tooltip (name changed): entity {:?}, name: {}, AID: {}",
            entity, entity_name.name, network_entity.aid
        );

        if let Err(e) = app_handle.emit("entity-name-show", event) {
            error!("Failed to emit entity-name-show event: {:?}", e);
        }
    }
}
