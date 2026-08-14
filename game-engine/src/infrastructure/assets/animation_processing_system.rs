use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy::{asset::LoadState, prelude::*};
use moonshine_tag::Tag;

use super::animation_processor::{ProcessedAnimation, RoAnimationProcessor};
use super::loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset};
use super::ro_animation_asset::RoAnimationAsset;
use crate::domain::settings::GraphicsSettings;
use crate::domain::settings::resources::Upscaling;

/// A pending animation request waiting for SPR+ACT and an optional palette to load.
#[derive(Debug, Clone)]
pub struct PendingAnimation {
    pub sprite_handle: Handle<RoSpriteAsset>,
    pub action_handle: Handle<RoActAsset>,
    pub palette_handle: Option<Handle<RoPaletteAsset>>,
    pub layer_tag: Tag,
    pub callback_entity: Option<Entity>,
}

/// A request whose CPU stage is running on [`AsyncComputeTaskPool`].
/// Dropping the entry cancels the task.
struct InFlightAnimation {
    request: PendingAnimation,
    task: Task<ProcessedAnimation>,
}

/// Resource tracking pending animation processing requests.
#[derive(Resource, Default)]
pub struct PendingAnimations {
    pending: Vec<PendingAnimation>,
    in_flight: Vec<InFlightAnimation>,
    completed: Vec<(PendingAnimation, Handle<RoAnimationAsset>)>,
}

impl PendingAnimations {
    /// Request processing of an SPR+ACT pair and optional palette into RoAnimationAsset.
    pub fn request(
        &mut self,
        sprite_handle: Handle<RoSpriteAsset>,
        action_handle: Handle<RoActAsset>,
        palette_handle: Option<Handle<RoPaletteAsset>>,
        layer_tag: Tag,
        callback_entity: Option<Entity>,
    ) {
        self.pending.push(PendingAnimation {
            sprite_handle,
            action_handle,
            palette_handle,
            layer_tag,
            callback_entity,
        });
    }

    /// Drop every queued or completed request for `entity`'s `layer` that hasn't
    /// been claimed by a finalizer yet. Used when a body rebuild supersedes a
    /// still-loading request (e.g. the riding swap arriving while the spawn-time
    /// body is in flight), which would otherwise finalize a second stale layer.
    pub fn discard_for(&mut self, entity: Entity, layer: Tag) {
        let stale =
            |p: &PendingAnimation| p.callback_entity == Some(entity) && p.layer_tag == layer;
        self.pending.retain(|p| !stale(p));
        self.in_flight.retain(|e| !stale(&e.request));
        self.completed.retain(|(p, _)| !stale(p));
    }

    /// Take only the completed animations whose layer satisfies `pred`, leaving
    /// the rest queued. The queue is shared by the body/head, cart, and equipment
    /// finalizers; each MUST claim only its own layers — a finalizer that drains
    /// everything eats (and loses) completions belonging to the others.
    pub fn take_completed_where(
        &mut self,
        mut pred: impl FnMut(Tag) -> bool,
    ) -> Vec<(PendingAnimation, Handle<RoAnimationAsset>)> {
        let (mine, rest) = std::mem::take(&mut self.completed)
            .into_iter()
            .partition(|(pending, _)| pred(pending.layer_tag));
        self.completed = rest;
        mine
    }

    /// Take only the completed animations for a specific layer, leaving the rest
    /// queued.
    pub fn take_completed_for_layer(
        &mut self,
        layer: Tag,
    ) -> Vec<(PendingAnimation, Handle<RoAnimationAsset>)> {
        self.take_completed_where(|tag| tag == layer)
    }

    /// Re-queue completions whose target entity wasn't ready this frame (its
    /// `PendingRenderLayers` hadn't flushed yet), so they're retried next frame
    /// instead of being lost.
    pub fn defer_completed(&mut self, items: Vec<(PendingAnimation, Handle<RoAnimationAsset>)>) {
        self.completed.extend(items);
    }

    /// Check if there are queued requests or in-flight processing tasks.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.in_flight.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaletteReadiness {
    Pending,
    ReadyWithCustomPalette,
    ReadyWithEmbeddedPalette,
}

type AnimationSourceAssets<'w> = (
    Res<'w, Assets<RoSpriteAsset>>,
    Res<'w, Assets<RoActAsset>>,
    Res<'w, Assets<RoPaletteAsset>>,
);

fn palette_readiness(
    palette_requested: bool,
    palette_loaded: bool,
    palette_failed: bool,
) -> PaletteReadiness {
    if !palette_requested || palette_failed {
        PaletteReadiness::ReadyWithEmbeddedPalette
    } else if palette_loaded {
        PaletteReadiness::ReadyWithCustomPalette
    } else {
        PaletteReadiness::Pending
    }
}

/// System that dispatches ready SPR+ACT pairs (and optional palettes) to
/// [`AsyncComputeTaskPool`] and finalizes finished tasks. The heavy pixel work
/// (palette bake, RGBA conversion, xBRZ upscaling) runs off the main thread;
/// only the cheap `Assets<Image>` registration happens here.
pub fn process_pending_animations(
    mut pending: ResMut<PendingAnimations>,
    assets: AnimationSourceAssets,
    asset_server: Res<AssetServer>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<GraphicsSettings>,
) {
    let (sprites, actions, palettes) = assets;
    dispatch_ready_requests(
        &mut pending,
        &sprites,
        &actions,
        &palettes,
        &asset_server,
        settings.upscaling,
    );
    finalize_finished_tasks(&mut pending, &mut animations, &mut images);
}

/// Spawn the CPU stage on the async pool for every request whose source assets
/// finished loading, moving it from `pending` to `in_flight`.
fn dispatch_ready_requests(
    pending: &mut PendingAnimations,
    sprites: &Assets<RoSpriteAsset>,
    actions: &Assets<RoActAsset>,
    palettes: &Assets<RoPaletteAsset>,
    asset_server: &AssetServer,
    upscaling: Upscaling,
) {
    let task_pool = AsyncComputeTaskPool::get();
    let mut still_pending = Vec::new();

    for request in std::mem::take(&mut pending.pending) {
        let sprite_ready = sprites.get(&request.sprite_handle).is_some();
        let action_ready = actions.get(&request.action_handle).is_some();
        let palette_loaded = request
            .palette_handle
            .as_ref()
            .is_some_and(|handle| palettes.get(handle).is_some());
        let palette_failed = request
            .palette_handle
            .as_ref()
            .is_some_and(|handle| matches!(asset_server.load_state(handle), LoadState::Failed(_)));
        let palette_state = palette_readiness(
            request.palette_handle.is_some(),
            palette_loaded,
            palette_failed,
        );

        if !sprite_ready || !action_ready || palette_state == PaletteReadiness::Pending {
            still_pending.push(request);
            continue;
        }

        let custom_palette = match palette_state {
            PaletteReadiness::ReadyWithCustomPalette => {
                let handle = request
                    .palette_handle
                    .as_ref()
                    .expect("custom palette readiness requires a palette handle");
                Some(
                    palettes
                        .get(handle)
                        .expect("ready palette must be present in Assets")
                        .clone(),
                )
            }
            PaletteReadiness::ReadyWithEmbeddedPalette => {
                if palette_failed {
                    let handle = request
                        .palette_handle
                        .as_ref()
                        .expect("failed palette request must carry a palette handle");
                    let path = asset_server
                        .get_path(handle.id())
                        .expect("failed palette handle must have an asset path");
                    warn!("Failed to load custom palette {path}; using embedded sprite palette");
                }
                None
            }
            PaletteReadiness::Pending => unreachable!("pending palette was re-queued"),
        };

        // NOTE: source data is cloned into the task; the clone is trivial next
        // to the pixel work the task exists to offload.
        let sprite = sprites
            .get(&request.sprite_handle)
            .expect("ready sprite must be present in Assets")
            .sprite
            .clone();
        let action = actions
            .get(&request.action_handle)
            .expect("ready action must be present in Assets")
            .action
            .clone();
        let layer_tag = request.layer_tag;

        let task = task_pool.spawn(async move {
            RoAnimationProcessor::process_cpu(
                &sprite,
                &action,
                custom_palette.as_ref(),
                layer_tag,
                upscaling,
            )
        });
        pending.in_flight.push(InFlightAnimation { request, task });
    }

    pending.pending = still_pending;
}

/// Drain finished CPU-stage tasks, register their images, and move them to
/// `completed` for the finalizer systems to claim.
fn finalize_finished_tasks(
    pending: &mut PendingAnimations,
    animations: &mut Assets<RoAnimationAsset>,
    images: &mut Assets<Image>,
) {
    let mut still_in_flight = Vec::new();

    for mut entry in std::mem::take(&mut pending.in_flight) {
        match check_ready(&mut entry.task) {
            Some(processed) => {
                let handle = animations.add(processed.finalize(images));
                pending.completed.push((entry.request, handle));
            }
            None => still_in_flight.push(entry),
        }
    }

    pending.in_flight = still_in_flight;
}

/// Plugin that sets up the animation processing system.
pub struct AnimationProcessingPlugin;

impl Plugin for AnimationProcessingPlugin {
    fn build(&self, app: &mut App) {
        // Gated so the Assets<Image> ResMut access doesn't serialize the
        // schedule on every frame where nothing is queued (the steady state).
        app.init_resource::<PendingAnimations>().add_systems(
            Update,
            process_pending_animations
                .run_if(|pending: Res<PendingAnimations>| pending.has_pending()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::sprite::tags::LAYER_BODY;
    use crate::infrastructure::ro_formats::{
        RoAction, RoSprite,
        sprite::{Palette, SpriteFrame},
    };
    use bevy::asset::AssetPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<RoSpriteAsset>()
            .init_asset::<RoActAsset>()
            .init_asset::<RoPaletteAsset>()
            .init_asset::<RoAnimationAsset>()
            .init_asset::<Image>()
            .init_resource::<PendingAnimations>()
            .insert_resource(GraphicsSettings::default())
            .add_systems(Update, process_pending_animations);
        app
    }

    fn sprite(embedded_color: [u8; 4]) -> RoSpriteAsset {
        RoSpriteAsset {
            sprite: RoSprite {
                version: 1.0,
                indexed_count: 1,
                rgba_count: 0,
                frames: vec![SpriteFrame {
                    width: 1,
                    height: 1,
                    data: vec![1],
                    is_rgba: false,
                }],
                palette: Some(Palette {
                    colors: vec![[0, 0, 0, 0], embedded_color],
                }),
            },
        }
    }

    fn action() -> RoActAsset {
        RoActAsset {
            action: RoAction {
                version: 1.0,
                actions: Vec::new(),
                sounds: Vec::new(),
            },
        }
    }

    /// Processing runs on the async task pool, so completion is not guaranteed
    /// within a single update; poll with a bounded retry budget.
    fn wait_for_completed(app: &mut App) -> Vec<(PendingAnimation, Handle<RoAnimationAsset>)> {
        for _ in 0..500 {
            app.update();
            let completed = app
                .world_mut()
                .resource_mut::<PendingAnimations>()
                .take_completed_for_layer(LAYER_BODY);
            if !completed.is_empty() {
                return completed;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("animation processing did not complete within the retry budget");
    }

    fn completed_texture_data(app: &mut App) -> Vec<u8> {
        let completed = wait_for_completed(app);

        let [(_, animation_handle)] = completed.as_slice() else {
            panic!("expected one completed animation");
        };
        let texture_handle = app
            .world()
            .resource::<Assets<RoAnimationAsset>>()
            .get(animation_handle)
            .unwrap()
            .textures[0]
            .clone();
        app.world()
            .resource::<Assets<Image>>()
            .get(&texture_handle)
            .unwrap()
            .data
            .clone()
            .unwrap()
    }

    #[test]
    fn loaded_palette_request_bakes_custom_palette_colors() {
        let mut app = app();
        let sprite = app
            .world_mut()
            .resource_mut::<Assets<RoSpriteAsset>>()
            .add(sprite([1, 2, 3, 255]));
        let action = app
            .world_mut()
            .resource_mut::<Assets<RoActAsset>>()
            .add(action());
        let palette = app
            .world_mut()
            .resource_mut::<Assets<RoPaletteAsset>>()
            .add(RoPaletteAsset {
                colors: vec![[0, 0, 0, 0], [10, 20, 30, 255]],
            });

        app.world_mut().resource_mut::<PendingAnimations>().request(
            sprite,
            action,
            Some(palette),
            LAYER_BODY,
            None,
        );
        app.update();

        assert_eq!(completed_texture_data(&mut app), vec![10, 20, 30, 255]);
    }

    #[test]
    fn paletteless_request_bakes_embedded_palette_colors() {
        let mut app = app();
        let sprite = app
            .world_mut()
            .resource_mut::<Assets<RoSpriteAsset>>()
            .add(sprite([1, 2, 3, 255]));
        let action = app
            .world_mut()
            .resource_mut::<Assets<RoActAsset>>()
            .add(action());

        app.world_mut()
            .resource_mut::<PendingAnimations>()
            .request(sprite, action, None, LAYER_BODY, None);
        app.update();

        assert_eq!(completed_texture_data(&mut app), vec![1, 2, 3, 255]);
    }

    #[test]
    fn failed_palette_load_uses_embedded_palette() {
        assert_eq!(
            palette_readiness(true, false, true),
            PaletteReadiness::ReadyWithEmbeddedPalette
        );
    }

    #[test]
    fn paletteless_request_is_ready_with_embedded_palette() {
        assert_eq!(
            palette_readiness(false, false, false),
            PaletteReadiness::ReadyWithEmbeddedPalette
        );
    }
}
