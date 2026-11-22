//! Draws the companion's sprite on every [`Portrait`] node: the window's parchment
//! and the plate's avatar.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lifthrasir_assets::{
    animation::converters::convert_sprite_frame_to_rgba,
    catalogs::JobSpriteRegistry,
    loaders::{RoActAsset, RoSpriteAsset},
    paths::{homunculus_action_path, homunculus_sprite_path},
};
use lifthrasir_client::domain::homunculus::HomunculusState;

use super::Portrait;

#[derive(Default)]
pub(super) struct Pending {
    id: u32,
    sources: Option<(Handle<RoSpriteAsset>, Handle<RoActAsset>)>,
    image: Option<Handle<Image>>,
}

type Sources<'w> = (
    Res<'w, AssetServer>,
    Res<'w, Assets<RoSpriteAsset>>,
    Res<'w, Assets<RoActAsset>>,
);

pub(super) fn update(
    state: Res<HomunculusState>,
    jobs: Option<Res<JobSpriteRegistry>>,
    sources: Sources,
    mut images: ResMut<Assets<Image>>,
    mut pending: Local<Pending>,
    mut nodes: Query<(&mut ImageNode, &mut Node), With<Portrait>>,
) {
    if nodes.is_empty() {
        return;
    }
    let id = state.snapshot().map_or(0, |s| s.appearance_id);
    if pending.id != id {
        *pending = Pending { id, ..default() };
    }
    if pending.image.is_none() && (6001..=6016).contains(&id) {
        pending.image =
            idle_frame(&mut pending, jobs.as_deref(), &sources).map(|pixels| images.add(pixels));
    }
    let (handle, display) = match &pending.image {
        Some(handle) => (handle.clone(), Display::Flex),
        None => (Handle::default(), Display::None),
    };
    for (mut image, mut node) in &mut nodes {
        if image.image != handle {
            image.image = handle.clone();
        }
        if node.display != display {
            node.display = display;
        }
    }
}

/// The largest layer of the companion's first idle frame, once its sprite is loaded.
fn idle_frame(
    pending: &mut Pending,
    jobs: Option<&JobSpriteRegistry>,
    (server, sprites, acts): &Sources,
) -> Option<Image> {
    if pending.sources.is_none() {
        let name = jobs?.get_sprite_name(pending.id)?;
        pending.sources = Some((
            server.load(homunculus_sprite_path(name)),
            server.load(homunculus_action_path(name)),
        ));
    }
    let (sprite, act) = pending.sources.as_ref()?;
    let (sprite, act) = (sprites.get(sprite)?, acts.get(act)?);
    let frame = act
        .action
        .actions
        .first()
        .and_then(|action| action.animations.first())
        .into_iter()
        .flat_map(|frame| &frame.layers)
        .filter_map(|layer| {
            let index = usize::try_from(layer.sprite_index).ok()?;
            let index = index
                + if layer.sprite_type == 0 {
                    0
                } else {
                    usize::from(sprite.sprite.indexed_count)
                };
            sprite.sprite.frames.get(index)
        })
        .max_by_key(|frame| u32::from(frame.width) * u32::from(frame.height))?;
    let mut pixels = Image::new(
        Extent3d {
            width: u32::from(frame.width),
            height: u32::from(frame.height),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        convert_sprite_frame_to_rgba(frame, sprite.sprite.palette.as_ref(), None),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    pixels.sampler = ImageSampler::nearest();
    Some(pixels)
}
