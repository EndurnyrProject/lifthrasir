use bevy::prelude::*;
use bevy_persistent::prelude::Persistent;

use crate::domain::assets::patterns;
use crate::domain::settings::resources::Settings;
use crate::domain::sprite::tags::LAYER_BODY;
use crate::infrastructure::assets::animation_processor::RoAnimationProcessor;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::{ActionData, RoAnimationAsset};

/// Shared, processed emote animation plus per-action thumbnail images.
///
/// `emotion.act` is direction-less: it carries one action per emote (98 actions
/// covering emote ids `0..MAX_EMOTE_ID` plus newer client emotes), NOT the
/// action-type x 8-direction grid the body uses or the cart's per-facing layout.
/// So an emote id indexes straight into both `actions` and `thumbnails`
/// (`thumbnails[id]` is the representative frame of `actions[id]`). Renderers and
/// the picker must guard `id < animation.actions.len()`.
#[derive(Resource)]
pub struct EmoteAssets {
    pub animation: Handle<RoAnimationAsset>,
    pub thumbnails: Vec<Handle<Image>>,
}

/// SPR/ACT handles still loading for the shared emote sprite. Mirrors the cart's
/// `CartAnimationPending`, but as a resource since there is no owning entity: the
/// emote sprite is processed once into the global [`EmoteAssets`], bypassing the
/// shared `PendingAnimations` queue exactly as the cart does.
#[derive(Resource)]
pub struct EmoteAssetsPending {
    spr: Handle<RoSpriteAsset>,
    act: Handle<RoActAsset>,
}

/// Kicks off the shared `emotion.spr`/`emotion.act` loads on entering gameplay.
pub fn load_emote_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(EmoteAssetsPending {
        spr: asset_server.load(patterns::emotion_sprite_path()),
        act: asset_server.load(patterns::emotion_action_path()),
    });
}

/// Polls the pending emote handles; once both are loaded, processes them through
/// `RoAnimationProcessor` (embedded palette, cart pattern), builds one thumbnail
/// per action, inserts [`EmoteAssets`], and drops the pending marker so this runs
/// exactly once.
pub fn finalize_emote_assets(
    mut commands: Commands,
    pending: Option<Res<EmoteAssetsPending>>,
    sprites: Res<Assets<RoSpriteAsset>>,
    actions: Res<Assets<RoActAsset>>,
    mut animations: ResMut<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<Persistent<Settings>>,
) {
    let Some(pending) = pending else {
        return;
    };
    let (Some(sprite), Some(action)) = (sprites.get(&pending.spr), actions.get(&pending.act))
    else {
        return;
    };

    let animation = RoAnimationProcessor::process(
        &sprite.sprite,
        &action.action,
        LAYER_BODY,
        &mut images,
        settings.graphics.upscaling,
    );

    let thumbnails = action_thumbnails(&animation);
    let animation = animations.add(animation);

    commands.insert_resource(EmoteAssets {
        animation,
        thumbnails,
    });
    commands.remove_resource::<EmoteAssetsPending>();
}

/// One thumbnail per action, aligned so `thumbnails[id]` matches `actions[id]`.
/// Alignment is load-bearing (id == action index == thumbnail index), so an
/// action with no representative part yields a placeholder handle rather than
/// shifting later ids.
fn action_thumbnails(animation: &RoAnimationAsset) -> Vec<Handle<Image>> {
    animation
        .actions
        .iter()
        .map(|action| {
            representative_texture_index(action)
                .and_then(|index| animation.textures.get(index))
                .cloned()
                .unwrap_or_default()
        })
        .collect()
}

/// Texture index of an action's representative (first) frame's first part, or
/// `None` when the action has no drawable frame.
fn representative_texture_index(action: &ActionData) -> Option<usize> {
    action
        .frames
        .first()?
        .parts
        .first()
        .map(|part| part.texture_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::ro_animation_asset::{FrameData, FramePart};

    fn part(texture_index: usize) -> FramePart {
        FramePart {
            texture_index,
            transform: Mat4::IDENTITY,
            position: Vec2::ZERO,
            scale: Vec2::ONE,
            texture_size: Vec2::ONE,
            color: Color::WHITE,
            mirror: false,
        }
    }

    fn action_with_parts(parts: Vec<FramePart>) -> ActionData {
        ActionData {
            frames: vec![FrameData { parts, ..default() }],
            delay_ms: 100.0,
        }
    }

    #[test]
    fn representative_texture_index_is_first_part_of_first_frame() {
        let action = action_with_parts(vec![part(3), part(7)]);
        assert_eq!(representative_texture_index(&action), Some(3));
    }

    #[test]
    fn representative_texture_index_none_without_frames() {
        let action = ActionData {
            frames: Vec::new(),
            delay_ms: 100.0,
        };
        assert_eq!(representative_texture_index(&action), None);
    }

    #[test]
    fn representative_texture_index_none_without_parts() {
        let action = action_with_parts(Vec::new());
        assert_eq!(representative_texture_index(&action), None);
    }
}
