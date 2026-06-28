use bevy::prelude::*;
use bevy_persistent::prelude::Persistent;
use moonshine_tag::Tag;

use super::animation_processor::RoAnimationProcessor;
use super::loaders::{RoActAsset, RoSpriteAsset};
use super::ro_animation_asset::RoAnimationAsset;
use crate::domain::settings::resources::Settings;

/// A pending animation request waiting for SPR+ACT to load.
#[derive(Debug, Clone)]
pub struct PendingAnimation {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub layer_tag: Tag,
    pub callback_entity: Option<Entity>,
}

/// Resource tracking pending animation processing requests.
#[derive(Resource, Default)]
pub struct PendingAnimations {
    pending: Vec<PendingAnimation>,
    completed: Vec<(PendingAnimation, Handle<RoAnimationAsset>)>,
}

impl PendingAnimations {
    /// Request processing of an SPR+ACT pair into RoAnimationAsset.
    pub fn request(
        &mut self,
        sprite_handle: Handle<RoSpriteAsset>,
        action_handle: Handle<RoActAsset>,
        layer_tag: Tag,
        callback_entity: Option<Entity>,
    ) {
        self.pending.push(PendingAnimation {
            sprite_handle,
            action_handle,
            layer_tag,
            callback_entity,
        });
    }

    /// Take all completed animations for processing by other systems.
    pub fn take_completed(&mut self) -> Vec<(PendingAnimation, Handle<RoAnimationAsset>)> {
        std::mem::take(&mut self.completed)
    }

    /// Re-queue completions whose target entity wasn't ready this frame (its
    /// `PendingRenderLayers` hadn't flushed yet), so they're retried next frame
    /// instead of being lost.
    pub fn defer_completed(&mut self, items: Vec<(PendingAnimation, Handle<RoAnimationAsset>)>) {
        self.completed.extend(items);
    }

    /// Check if there are pending requests.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// System that processes pending SPR+ACT pairs when both are loaded.
pub fn process_pending_animations(
    mut pending: ResMut<PendingAnimations>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<Persistent<Settings>>,
) {
    let upscaling = settings.graphics.upscaling;
    let mut still_pending = Vec::new();
    let mut newly_completed = Vec::new();

    for request in std::mem::take(&mut pending.pending) {
        let sprite_ready = sprites.get(&request.sprite_handle).is_some();
        let action_ready = actions.get(&request.action_handle).is_some();

        if sprite_ready && action_ready {
            let sprite = sprites.get(&request.sprite_handle).unwrap();
            let action = actions.get(&request.action_handle).unwrap();

            let animation = RoAnimationProcessor::process(
                &sprite.sprite,
                &action.action,
                request.layer_tag,
                &mut images,
                upscaling,
            );

            let handle = animations.add(animation);
            newly_completed.push((request, handle));
        } else {
            still_pending.push(request);
        }
    }

    pending.pending = still_pending;
    pending.completed.extend(newly_completed);
}

/// Plugin that sets up the animation processing system.
pub struct AnimationProcessingPlugin;

impl Plugin for AnimationProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingAnimations>()
            .add_systems(Update, process_pending_animations);
    }
}
