use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::actors::ActionType;
use crate::actors::sprites::components::{
    BodyAttachPoint, HeadLayer, MobSprite, PlayerAppearance, PlayerSprite, RenderLayer,
    RoSpriteGeneric,
};
use crate::actors::sprites::layout::ActionLayout;
use crate::actors::sprites::systems::set_layer_texture;
use crate::audio::events::PlayMobSfx;
use crate::effects::AnimationPaused;
use crate::rendering::SpriteRenderingSystems;
use crate::rendering::constants::SPRITE_WORLD_SCALE;
use lifthrasir_assets::animation::asset::RoAnimationAsset;
use lifthrasir_assets::catalogs::WeaponDb;

type BodyLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut BodyAttachPoint,
    ),
    Without<HeadLayer>,
>;

/// Retail ACT sound tables also carry authoring placeholders (e.g. `atk`, `a`)
/// that name no audio file, so only `.wav` entries are worth loading.
fn is_playable_sound(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".wav")
}

fn weapon_attack_sound(
    appearance: Option<&PlayerAppearance>,
    weapon_db: Option<&WeaponDb>,
) -> Option<&'static str> {
    let view_id = appearance?.weapon_view_id?;
    weapon_db?.attack_sound(view_id)
}

/// Resolve the sound a frame triggers. A player swinging a weapon plays the
/// retail per-weapon swing wav instead of the class ACT sound, which is generic
/// per job; everything else (steps, cloth rustle) keeps the ACT sound.
fn resolve_sound<'a>(
    action_type: ActionType,
    sound_id: Option<i32>,
    animation: &'a RoAnimationAsset,
    appearance: Option<&PlayerAppearance>,
    weapon_db: Option<&WeaponDb>,
) -> Option<&'a str> {
    let name = animation
        .sounds
        .get(sound_id? as usize)
        .map(String::as_str)?;
    if !is_playable_sound(name) {
        return None;
    }
    if !action_type.is_attack() {
        return Some(name);
    }

    weapon_attack_sound(appearance, weapon_db).or(Some(name))
}

fn sync_body_layer_impl<T: ActionLayout>(
    game_time_ms: u32,
    animations: &Res<Assets<RoAnimationAsset>>,
    materials: &mut Assets<StandardMaterial>,
    parent_query: &Query<(
        &RoSpriteGeneric<T>,
        Option<&AnimationPaused>,
        Option<&PlayerAppearance>,
    )>,
    layer_query: &mut BodyLayerQuery,
    weapon_db: Option<&WeaponDb>,
    mut sfx: Option<&mut MessageWriter<PlayMobSfx>>,
) {
    for (layer, child_of, material_handle, mut transform, mut attach_point) in
        layer_query.iter_mut()
    {
        let Ok((ro_sprite, paused, appearance)) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&layer.animation) else {
            continue;
        };

        // A frozen/petrified unit holds its animation on the frame that was
        // showing when the pause began: feed that captured timestamp instead of
        // the live clock. The head and weapon layers ride the body's published
        // frame index, so freezing the body alone holds the whole character.
        let effective_time = paused.map_or(game_time_ms, |p| p.at_ms);

        let frame_index = ro_sprite.get_frame_index(animation, effective_time);
        let Some(frame) = ro_sprite.get_frame(animation, effective_time) else {
            continue;
        };

        // Fire once when crossing into a new frame that carries a sound id.
        // `as_mut()` reborrows the Option each iteration (MessageWriter is not DerefMut).
        if let Some(writer) = sfx.as_mut()
            && frame_index != attach_point.frame_index
            && let Some(name) = resolve_sound(
                ro_sprite.action_type,
                frame.sound_id,
                animation,
                appearance,
                weapon_db,
            )
        {
            writer.write(PlayMobSfx {
                emitter: child_of.parent(),
                sound: name.to_string(),
            });
        }

        if let Some(part) = frame.parts.first() {
            if let Some(texture) = animation.textures.get(part.texture_index) {
                set_layer_texture(materials, &material_handle.0, texture);
            }

            let sprite_width = part.texture_size.x;
            let sprite_height = part.texture_size.y;

            let mut scale_x = part.scale.x * sprite_width * SPRITE_WORLD_SCALE;
            let scale_y = part.scale.y * sprite_height * SPRITE_WORLD_SCALE;

            if part.mirror {
                scale_x = -scale_x;
            }

            // RO authors each frame so the sprite, drawn centered at its ACT
            // `position`, lands its feet on the ground anchor. `position` was
            // Y-negated on extraction into Y-up space, so it places the center
            // directly: taller sprites carry a larger `position.y` and are
            // lifted more, which is what grounds them.
            let current = *transform;
            transform.set_if_neq(Transform {
                scale: Vec3::new(scale_x, scale_y, 1.0),
                translation: Vec3::new(
                    part.position.x * SPRITE_WORLD_SCALE,
                    part.position.y * SPRITE_WORLD_SCALE,
                    current.translation.z,
                ),
                ..current
            });
        }

        attach_point.set_if_neq(BodyAttachPoint {
            attach_point: frame.attach_point.unwrap_or(attach_point.attach_point),
            frame_index,
            layer_pos: frame
                .parts
                .first()
                .map_or(attach_point.layer_pos, |part| part.position),
        });
    }
}

#[auto_add_system(
    plugin = crate::actors::ActorRenderPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_player_body_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<(
        &PlayerSprite,
        Option<&AnimationPaused>,
        Option<&PlayerAppearance>,
    )>,
    mut layer_query: BodyLayerQuery,
    weapon_db: Option<Res<WeaponDb>>,
    mut sfx_writer: MessageWriter<PlayMobSfx>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
        weapon_db.as_deref(),
        Some(&mut sfx_writer),
    );
}

#[auto_add_system(
    plugin = crate::actors::ActorRenderPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate)
)]
pub fn sync_mob_body_layer(
    time: Res<Time>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parent_query: Query<(
        &MobSprite,
        Option<&AnimationPaused>,
        Option<&PlayerAppearance>,
    )>,
    mut layer_query: BodyLayerQuery,
    mut sfx_writer: MessageWriter<PlayMobSfx>,
) {
    let game_time_ms = (time.elapsed_secs() * 1000.0) as u32;
    sync_body_layer_impl(
        game_time_ms,
        &animations,
        &mut materials,
        &parent_query,
        &mut layer_query,
        None,
        Some(&mut sfx_writer),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::WeaponData;

    fn animation(sound: &str) -> RoAnimationAsset {
        RoAnimationAsset {
            sounds: vec![sound.to_string()],
            ..Default::default()
        }
    }

    fn appearance(weapon_view_id: Option<u16>) -> PlayerAppearance {
        PlayerAppearance {
            weapon_view_id,
            ..Default::default()
        }
    }

    fn weapon_db() -> WeaponDb {
        WeaponDb::from_weapon_data(WeaponData::default())
    }

    #[test]
    fn playable_sound_accepts_wav_names() {
        assert!(is_playable_sound("poring_attack.wav"));
        assert!(is_playable_sound("_attack_sword.WAV"));
    }

    #[test]
    fn playable_sound_rejects_act_placeholders() {
        assert!(!is_playable_sound("atk"));
        assert!(!is_playable_sound("a"));
        assert!(!is_playable_sound(""));
    }

    #[test]
    fn attack_frame_plays_the_equipped_weapons_swing_sound() {
        let act = animation("attack_sword.wav");
        let db = weapon_db();

        let sound = resolve_sound(
            ActionType::Attack,
            Some(0),
            &act,
            Some(&appearance(Some(16))),
            Some(&db),
        );

        assert_eq!(sound, Some("_attack_katar.wav"));
    }

    #[test]
    fn non_attack_frame_keeps_the_act_sound() {
        let act = animation("player_clothes.wav");
        let db = weapon_db();

        let sound = resolve_sound(
            ActionType::Walk,
            Some(0),
            &act,
            Some(&appearance(Some(2))),
            Some(&db),
        );

        assert_eq!(sound, Some("player_clothes.wav"));
    }

    #[test]
    fn attack_frame_falls_back_when_the_weapon_class_has_no_swing_wav() {
        let act = animation("attack_sword.wav");
        let db = weapon_db();

        let sound = resolve_sound(
            ActionType::Attack,
            Some(0),
            &act,
            Some(&appearance(Some(0))),
            Some(&db),
        );

        assert_eq!(sound, Some("attack_sword.wav"));
    }

    #[test]
    fn unarmed_attack_frame_keeps_the_act_sound() {
        let act = animation("attack_short_sword.wav");

        let sound = resolve_sound(ActionType::Attack, Some(0), &act, None, None);

        assert_eq!(sound, Some("attack_short_sword.wav"));
    }

    #[test]
    fn placeholder_sound_ids_play_nothing() {
        let act = animation("atk");
        let sound = resolve_sound(ActionType::Attack, Some(0), &act, None, None);

        assert_eq!(sound, None);
    }
}
