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

use super::{OwnerUi, Portrait};

#[derive(Default)]
pub(super) struct Pending {
    id: u32,
    sources: Option<(Handle<RoSpriteAsset>, Handle<RoActAsset>)>,
    image: Handle<Image>,
}

pub(super) fn update(
    ui: Res<OwnerUi>,
    state: Res<HomunculusState>,
    jobs: Option<Res<JobSpriteRegistry>>,
    (server, sprites, acts): (
        Res<AssetServer>,
        Res<Assets<RoSpriteAsset>>,
        Res<Assets<RoActAsset>>,
    ),
    mut images: ResMut<Assets<Image>>,
    mut pending: Local<Pending>,
    mut nodes: Query<(&mut ImageNode, &mut Node), With<Portrait>>,
) {
    if !ui.open {
        return;
    }
    let Ok((mut image, mut node)) = nodes.single_mut() else {
        return;
    };
    let id = state.snapshot().map_or(0, |s| s.appearance_id);
    if pending.id != id {
        *pending = Pending { id, ..default() };
    }
    if !(6001..=6016).contains(&id) {
        node.display = Display::None;
        image.image = Handle::default();
        return;
    }
    if pending.sources.is_none() {
        let Some(name) = jobs.as_ref().and_then(|jobs| jobs.get_sprite_name(id)) else {
            return;
        };
        pending.sources = Some((
            server.load(homunculus_sprite_path(name)),
            server.load(homunculus_action_path(name)),
        ));
    }
    if pending.image == Handle::default() {
        let (sprite, act) = pending.sources.as_ref().unwrap();
        let (Some(sprite), Some(act)) = (sprites.get(sprite), acts.get(act)) else {
            node.display = Display::None;
            image.image = Handle::default();
            return;
        };
        let Some(frame) = act
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
            .max_by_key(|frame| u32::from(frame.width) * u32::from(frame.height))
        else {
            return;
        };
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
        pending.image = images.add(pixels);
    }
    if image.image != pending.image {
        image.image = pending.image.clone();
    }
    node.display = Display::Flex;
}
