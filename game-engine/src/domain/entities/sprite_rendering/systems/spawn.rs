use super::super::components::{
    BodyAttachPoint, EntitySpriteData, HeadAttachment, HeadLayer, MobSprite, PendingRenderLayers,
    PlayerAppearance, PlayerSprite, RenderLayer, SpriteHierarchyConfig,
};
use super::super::events::{RequestSpriteSpawn, SpawnSpriteEvent};
use crate::domain::assets::patterns;
use crate::domain::entities::billboard::{Billboard, SharedSpriteQuad};
use crate::domain::sprite::tags::{
    layer_order, LAYER_BODY, LAYER_HEAD, LAYER_SHADOW, SPRITE_BASE_Y_OFFSET, Z_OFFSET_PER_LAYER,
};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::animation_processing_system::PendingAnimations;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::infrastructure::job::registry::JobSpriteRegistry;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use moonshine_tag::Tag;

/// Spawn system that handles sprite spawn events.
/// Adds PlayerSprite/MobSprite and optional PlayerAppearance to entities,
/// then requests animation asset loading.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::HierarchySpawn)
)]
pub fn spawn_sprite_hierarchy(
    mut commands: Commands,
    mut spawn_events: MessageReader<SpawnSpriteEvent>,
    _config: Res<SpriteHierarchyConfig>,
    asset_server: Res<AssetServer>,
    mut pending_animations: ResMut<PendingAnimations>,
    job_registry: Option<Res<JobSpriteRegistry>>,
) {
    for event in spawn_events.read() {
        let entity = event.entity;
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            warn!(
                "spawn_sprite_hierarchy: Entity {:?} no longer exists",
                entity
            );
            continue;
        };

        match &event.sprite_info.sprite_data {
            EntitySpriteData::Character {
                job_id,
                gender,
                head,
            } => {
                spawn_character_components(
                    &mut entity_commands,
                    *job_id,
                    *gender,
                    *head,
                    &asset_server,
                    &mut pending_animations,
                    job_registry.as_deref(),
                );
            }
            EntitySpriteData::Mob { sprite_name } => {
                spawn_mob_components(
                    &mut entity_commands,
                    sprite_name,
                    &asset_server,
                    &mut pending_animations,
                );
            }
            EntitySpriteData::Npc { sprite_name } => {
                spawn_npc_components(
                    &mut entity_commands,
                    sprite_name,
                    &asset_server,
                    &mut pending_animations,
                );
            }
        }

        debug!(
            "spawn_sprite_hierarchy: Processing SpawnSpriteEvent for entity {:?}",
            entity
        );
    }
}

fn spawn_character_components(
    entity_commands: &mut EntityCommands,
    job_id: u16,
    gender: crate::domain::entities::character::components::Gender,
    head_id: u16,
    asset_server: &AssetServer,
    pending_animations: &mut PendingAnimations,
    job_registry: Option<&JobSpriteRegistry>,
) {
    let entity = entity_commands.id();
    let gender_byte = match gender {
        crate::domain::entities::character::components::Gender::Male => 1u8,
        crate::domain::entities::character::components::Gender::Female => 0u8,
    };

    let Some(registry) = job_registry else {
        warn!(
            "spawn_character_components: JobSpriteRegistry not available for entity {:?}",
            entity
        );
        return;
    };

    let Some(body_spr_path) = registry.get_body_sprite_path(job_id as u32, gender_byte) else {
        warn!(
            "spawn_character_components: Unknown job_id {} for entity {:?}",
            job_id, entity
        );
        return;
    };
    let body_act_path = body_spr_path.replace(".spr", ".act");

    let head_spr_path = patterns::head_sprite_path(gender, head_id);
    let head_act_path = patterns::head_action_path(gender, head_id);

    let body_spr = asset_server.load(&body_spr_path);
    let body_act = asset_server.load(&body_act_path);
    let head_spr = asset_server.load(&head_spr_path);
    let head_act = asset_server.load(&head_act_path);

    pending_animations.request(body_spr.clone(), body_act.clone(), LAYER_BODY, Some(entity));
    pending_animations.request(head_spr.clone(), head_act.clone(), LAYER_HEAD, Some(entity));

    entity_commands.insert((
        PlayerSprite::default(),
        PlayerAppearance::default(),
        PendingRenderLayers,
    ));

    debug!(
        "spawn_character_components: Requested body ({}) and head animations for entity {:?}",
        body_spr_path, entity
    );
}

fn spawn_mob_components(
    entity_commands: &mut EntityCommands,
    sprite_name: &str,
    asset_server: &AssetServer,
    pending_animations: &mut PendingAnimations,
) {
    let entity = entity_commands.id();

    let spr_path = patterns::mob_sprite_path(sprite_name);
    let act_path = patterns::mob_action_path(sprite_name);

    let spr = asset_server.load(&spr_path);
    let act = asset_server.load(&act_path);

    pending_animations.request(spr, act, LAYER_BODY, Some(entity));

    entity_commands.insert((MobSprite::default(), PendingRenderLayers));

    debug!(
        "spawn_mob_components: Requested animation for entity {:?} ({})",
        entity, sprite_name
    );
}

fn spawn_npc_components(
    entity_commands: &mut EntityCommands,
    sprite_name: &str,
    asset_server: &AssetServer,
    pending_animations: &mut PendingAnimations,
) {
    let entity = entity_commands.id();

    let spr_path = patterns::npc_sprite_path(sprite_name);
    let act_path = patterns::npc_action_path(sprite_name);

    let spr = asset_server.load(&spr_path);
    let act = asset_server.load(&act_path);

    pending_animations.request(spr, act, LAYER_BODY, Some(entity));

    // NPCs are act-driven 8-direction sprites, identical in format to mobs, so they
    // ride the mob render path (`sync_mob_body_layer`). Without a `MobSprite` no
    // sync system would ever advance the layer past the raw first texture.
    entity_commands.insert((MobSprite::default(), PendingRenderLayers));

    debug!(
        "spawn_npc_components: Requested animation for entity {:?} ({})",
        entity, sprite_name
    );
}

type PendingRenderLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static mut PlayerAppearance>,
        Option<&'static mut PlayerSprite>,
        Option<&'static mut MobSprite>,
    ),
    With<PendingRenderLayers>,
>;

/// System that finalizes render layers when animation assets are loaded.
/// Spawns child entities with Mesh3d + MeshMaterial3d + RenderLayer components.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::HierarchySpawn, after = crate::infrastructure::assets::animation_processing_system::process_pending_animations)
)]
#[allow(clippy::too_many_arguments)]
pub fn finalize_render_layers(
    mut commands: Commands,
    mut pending_animations: ResMut<PendingAnimations>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut pending_entities: PendingRenderLayerQuery,
    alive: Query<Entity>,
    config: Res<SpriteHierarchyConfig>,
    shared_quad: Res<SharedSpriteQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let completed = pending_animations.take_completed();
    if completed.is_empty() {
        return;
    }

    debug!(
        "finalize_render_layers: Processing {} completed animations",
        completed.len()
    );

    // Completions whose target entity is alive but hasn't had `PendingRenderLayers`
    // flushed onto it yet (cached-asset case: the animation completes the same frame
    // the layer is requested, before that deferred component lands). Re-queued for the
    // next frame rather than dropped — otherwise the sprite would never build.
    let mut deferred = Vec::new();

    for (pending, animation_handle) in completed {
        let Some(callback_entity) = pending.callback_entity else {
            debug!("finalize_render_layers: No callback_entity for pending animation");
            continue;
        };

        debug!(
            "finalize_render_layers: Trying to get entity {:?}",
            callback_entity
        );

        let Ok((entity, maybe_appearance, maybe_player, maybe_mob)) =
            pending_entities.get_mut(callback_entity)
        else {
            // Alive but no `PendingRenderLayers` yet -> its components haven't flushed;
            // retry next frame. Gone (e.g. a rebuilt diorama preview) -> drop.
            if alive.contains(callback_entity) {
                deferred.push((pending, animation_handle));
            } else {
                warn!(
                    "finalize_render_layers: Entity {:?} no longer exists, dropping completion",
                    callback_entity
                );
            }
            continue;
        };

        let Some(animation) = animations.get(&animation_handle) else {
            warn!(
                "finalize_render_layers: Animation asset not found for entity {:?}",
                entity
            );
            continue;
        };

        if let Some(mut appearance) = maybe_appearance {
            if pending.layer_tag == LAYER_BODY {
                appearance.body = animation_handle.clone();
                if let Some(mut player) = maybe_player {
                    player.animation = animation_handle.clone();
                }
            } else if pending.layer_tag == LAYER_HEAD {
                appearance.head = animation_handle.clone();
            }
        } else if let Some(mut mob) = maybe_mob {
            mob.animation = animation_handle.clone();
        }

        let z_offset = layer_z_offset(pending.layer_tag, &config);

        let first_texture = animation.textures.first().cloned();
        if first_texture.is_none() {
            warn!(
                "finalize_render_layers: No textures available for entity {:?}, layer {:?}. Animation has {} textures.",
                entity, pending.layer_tag, animation.textures.len()
            );
        }
        let first_texture = first_texture.unwrap_or_default();

        debug!(
            "finalize_render_layers: Using texture handle {:?} for entity {:?}, animation has {} textures, first in animation: {:?}",
            first_texture, entity, animation.textures.len(), animation.textures.first()
        );

        let _layer_entity = spawn_render_layer_child(
            &mut commands,
            entity,
            animation_handle,
            pending.layer_tag,
            z_offset,
            first_texture,
            animation.textures.clone(),
            &shared_quad,
            &mut materials,
        );

        debug!(
            "finalize_render_layers: Spawned render layer child for entity {:?}, layer {:?}",
            entity, pending.layer_tag
        );
    }

    // Retry next frame for entities that weren't flushed yet.
    let has_deferred = !deferred.is_empty();
    pending_animations.defer_completed(deferred);

    // Only clear the pending marker once there's no outstanding work, so an entity
    // with a re-queued completion keeps `PendingRenderLayers` until it's finalized.
    if !pending_animations.has_pending() && !has_deferred {
        for (entity, _, _, _) in pending_entities.iter() {
            commands.entity(entity).remove::<PendingRenderLayers>();
        }
    }
}

fn layer_z_offset(layer: Tag, config: &SpriteHierarchyConfig) -> f32 {
    if layer == LAYER_SHADOW {
        return config.shadow_z_offset;
    }

    let order = layer_order(layer) as f32;
    order * Z_OFFSET_PER_LAYER
}

#[allow(clippy::too_many_arguments)]
fn spawn_render_layer_child(
    commands: &mut Commands,
    parent: Entity,
    animation: Handle<RoAnimationAsset>,
    layer: Tag,
    z_offset: f32,
    initial_texture: Handle<Image>,
    textures: Vec<Handle<Image>>,
    shared_quad: &SharedSpriteQuad,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let render_layer = RenderLayer::body(animation, layer, textures);
    let is_head = layer == LAYER_HEAD;
    let is_body = layer == LAYER_BODY;

    let local_offset = Vec3::new(0.0, SPRITE_BASE_Y_OFFSET, z_offset);

    debug!(
        "spawn_render_layer_child: Spawning with local offset {:?} for parent {:?}",
        local_offset, parent
    );

    let sprite_transform = Transform::from_translation(local_offset);

    let material = materials.add(StandardMaterial {
        base_color_texture: Some(initial_texture),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    let mut entity_commands = commands.spawn((
        Mesh3d(shared_quad.mesh.clone()),
        MeshMaterial3d(material),
        Billboard,
        sprite_transform,
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
        render_layer,
        ChildOf(parent),
    ));

    if is_body {
        entity_commands.insert(BodyAttachPoint::default());
    }

    if is_head {
        entity_commands.insert(HeadLayer);
    }

    let sprite_entity = entity_commands.id();

    debug!(
        "spawn_render_layer_child: Spawned sprite entity {:?} as child of {:?} with local offset {:?}",
        sprite_entity, parent, local_offset
    );

    sprite_entity
}

type UnlinkedHeadQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static ChildOf), (With<HeadLayer>, Without<HeadAttachment>)>;

/// System that links HeadLayer entities to their body siblings via HeadAttachment.
/// Runs after render layers are spawned and finds unlinked heads.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::HierarchySpawn, after = finalize_render_layers)
)]
pub fn link_head_to_body(
    mut commands: Commands,
    unlinked_heads: UnlinkedHeadQuery,
    body_layers: Query<Entity, With<BodyAttachPoint>>,
    children_query: Query<&Children>,
) {
    for (head_entity, child_of) in unlinked_heads.iter() {
        let parent = child_of.parent();

        let Ok(children) = children_query.get(parent) else {
            continue;
        };

        let body_entity = children.iter().find(|child| body_layers.contains(*child));

        let Some(body_entity) = body_entity else {
            continue;
        };

        commands
            .entity(head_entity)
            .insert(HeadAttachment { body_entity });

        debug!(
            "link_head_to_body: Linked head {:?} to body {:?}",
            head_entity, body_entity
        );
    }
}

/// Observer for sprite spawn requests
#[auto_observer(plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin)]
pub fn on_request_sprite_spawn(
    trigger: On<RequestSpriteSpawn>,
    mut sprite_spawn_writer: MessageWriter<SpawnSpriteEvent>,
) {
    let event = trigger.event();
    let entity = trigger.entity;

    debug!(
        "RequestSpriteSpawn RECEIVED for entity {:?} at position ({:.2}, {:.2}, {:.2})",
        entity, event.position.x, event.position.y, event.position.z
    );

    sprite_spawn_writer.write(SpawnSpriteEvent {
        entity,
        position: event.position,
        sprite_info: event.sprite_info.clone(),
    });
}
