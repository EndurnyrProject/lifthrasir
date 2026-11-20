use bevy::prelude::*;
use lifthrasir_runtime::audio::events::PlaySkillSfx;
use lifthrasir_runtime::effects::{EffectAnchor, load_str_effect, playback::spawn_effect};

use crate::domain::entities::character::{
    components::status::StatusParameter, events::StatusParameterChanged,
};

/// Plays level-up feedback after the first authoritative value for each level.
pub fn on_level_up(
    mut events: MessageReader<StatusParameterChanged>,
    mut seen: Local<Option<(Entity, bool, bool)>>,
    transforms: Query<&Transform>,
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut sounds: MessageWriter<PlaySkillSfx>,
) {
    for event in events.read() {
        let (effect, sound, is_base) = match event.parameter {
            StatusParameter::BaseLevel => ("angel.str", "levelup.wav", true),
            StatusParameter::JobLevel => ("angel_2nd.str", "effect/st_job_level_up.wav", false),
            _ => continue,
        };
        let state = seen.get_or_insert((event.entity, false, false));
        if state.0 != event.entity {
            *state = (event.entity, false, false);
        }
        let initialized = if is_base { &mut state.1 } else { &mut state.2 };
        let was_initialized = std::mem::replace(initialized, true);
        if !was_initialized || !event.old_value.is_some_and(|old| event.new_value > old) {
            continue;
        }
        let Ok(transform) = transforms.get(event.entity) else {
            continue;
        };
        spawn_effect(
            &mut commands,
            load_str_effect(&assets, effect),
            EffectAnchor::Position(transform.translation + Vec3::new(0.0, 2.0, 0.0)),
            false,
            Color::WHITE,
            None,
        );
        sounds.write(PlaySkillSfx {
            emitter: event.entity,
            sound: sound.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin, io::AssetSourceBuilder};
    use lifthrasir_assets::effects::asset::LoadedEffectAsset;
    use lifthrasir_runtime::effects::ActiveEffect;

    #[test]
    fn level_gains_play_both_cues_but_initial_values_and_decreases_do_not() {
        let mut app = App::new();
        app.register_asset_source(
            "ro",
            AssetSourceBuilder::platform_default("assets/data", None),
        )
        .add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<LoadedEffectAsset>()
        .add_message::<StatusParameterChanged>()
        .add_message::<PlaySkillSfx>()
        .add_systems(Update, on_level_up);
        let player = app.world_mut().spawn(Transform::default()).id();
        for (parameter, old, new) in [
            (StatusParameter::BaseLevel, 1, 50),
            (StatusParameter::JobLevel, 1, 20),
            (StatusParameter::BaseLevel, 50, 51),
            (StatusParameter::JobLevel, 20, 21),
            (StatusParameter::JobLevel, 21, 1),
        ] {
            app.world_mut().write_message(StatusParameterChanged {
                entity: player,
                parameter,
                old_value: Some(old),
                new_value: new,
            });
        }
        app.update();
        let sounds = app.world().resource::<Messages<PlaySkillSfx>>();
        let mut cursor = sounds.get_cursor();
        let sounds: Vec<_> = cursor
            .read(sounds)
            .map(|sound| sound.sound.as_str())
            .collect();
        assert_eq!(sounds, ["levelup.wav", "effect/st_job_level_up.wav"]);
        assert_eq!(
            app.world_mut()
                .query::<&ActiveEffect>()
                .iter(app.world())
                .count(),
            2
        );
    }
}
