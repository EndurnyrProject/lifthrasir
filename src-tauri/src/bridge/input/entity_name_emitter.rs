use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::domain::entities::{
    components::{EntityName, NetworkEntity},
    hover::HoveredEntity,
    sprite_rendering::components::SpriteObjectTree,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::plugin::TauriSystems;

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

fn emit_entity_tooltip(
    app_handle: &AppHandle,
    network_entity: &NetworkEntity,
    entity_name: &EntityName,
    screen_x: f32,
    screen_y: f32,
) {
    let event = EntityNameEvent {
        entity_id: network_entity.aid,
        name: entity_name.name.clone(),
        party_name: entity_name.party_name.clone(),
        guild_name: entity_name.guild_name.clone(),
        position_name: entity_name.position_name.clone(),
        screen_x,
        screen_y,
    };

    if let Err(e) = app_handle.emit("entity-name-show", event) {
        error!("Failed to emit entity-name-show event: {:?}", e);
    }
}

fn calculate_screen_position(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_pos: Vec3,
    app_handle: &AppHandle,
) -> Option<(f32, f32)> {
    let viewport_size = camera.logical_viewport_size()?;
    let ndc = camera.world_to_ndc(camera_transform, world_pos)?;

    let scale_factor = app_handle
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0) as f32;

    let screen_x = (ndc.x + 1.0) * 0.5 * viewport_size.x / scale_factor;
    let screen_y = (1.0 - ndc.y) * 0.5 * viewport_size.y / scale_factor;

    Some((screen_x, screen_y))
}

pub fn on_hover_started_with_name(
    trigger: On<Add, HoveredEntity>,
    app_handle: NonSend<AppHandle>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    entity_query: Query<(&NetworkEntity, &SpriteObjectTree, &EntityName)>,
    sprite_query: Query<&GlobalTransform>,
) {
    let entity = trigger.entity;

    let Ok((network_entity, sprite_tree, entity_name)) = entity_query.get(entity) else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Ok(sprite_transform) = sprite_query.get(sprite_tree.root) else {
        return;
    };

    let Some((screen_x, screen_y)) =
        calculate_screen_position(camera, camera_transform, sprite_transform.translation(), &app_handle)
    else {
        return;
    };

    emit_entity_tooltip(
        &app_handle,
        network_entity,
        entity_name,
        screen_x,
        screen_y,
    );
}

pub fn on_entity_name_added_to_hovered(
    trigger: On<Insert, EntityName>,
    app_handle: NonSend<AppHandle>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    entity_query: Query<(&NetworkEntity, &SpriteObjectTree), With<HoveredEntity>>,
    name_query: Query<&EntityName>,
    sprite_query: Query<&GlobalTransform>,
) {
    let entity = trigger.entity;

    let Ok((network_entity, sprite_tree)) = entity_query.get(entity) else {
        return;
    };

    let Ok(entity_name) = name_query.get(entity) else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Ok(sprite_transform) = sprite_query.get(sprite_tree.root) else {
        return;
    };

    let Some((screen_x, screen_y)) =
        calculate_screen_position(camera, camera_transform, sprite_transform.translation(), &app_handle)
    else {
        return;
    };

    emit_entity_tooltip(
        &app_handle,
        network_entity,
        entity_name,
        screen_x,
        screen_y,
    );
}

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Emitters)
)]
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
