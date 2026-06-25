//! Debug-only keybindings that trigger skill effects on the local player without
//! a server packet or an actual skill cast, so the effect playback pipeline
//! (catalog lookup, spawn, caster motion, sound, render) can be validated
//! in-game. Compiled out of release builds via `#[cfg(debug_assertions)]` at the
//! registration site.

use bevy::prelude::*;

use crate::domain::entities::components::NetworkEntity;
use crate::domain::entities::markers::LocalPlayer;
use crate::infrastructure::networking::zone_messages::{GroundSkillPlaced, SkillEffectShown};
use crate::utils::coordinates::world_position_to_spawn_coords;

/// Emit a mocked skill event on key press, keyed to the local player. The ids
/// match `assets/data/ron/skill_effects.ron`:
/// - F6 -> 18 magnus  (Caster placement, anchored on the player)
/// - F7 -> 28 heal    (Target placement, anchored on the player)
/// - F8 -> 89 stormgust (Ground placement, at the player's current cell)
pub fn debug_trigger_effect_on_keypress(
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(&NetworkEntity, &GlobalTransform), With<LocalPlayer>>,
    mut skill_effects: MessageWriter<SkillEffectShown>,
    mut ground_skills: MessageWriter<GroundSkillPlaced>,
) {
    let Ok((network, transform)) = player.single() else {
        return;
    };
    let gid = network.gid;

    if keys.just_pressed(KeyCode::F6) {
        skill_effects.write(SkillEffectShown {
            skill_id: 18,
            level: 1,
            src_id: gid,
            target_id: gid,
            result: 0,
        });
    }

    if keys.just_pressed(KeyCode::F7) {
        skill_effects.write(SkillEffectShown {
            skill_id: 28,
            level: 1,
            src_id: gid,
            target_id: gid,
            result: 0,
        });
    }

    if keys.just_pressed(KeyCode::F8) {
        let (x, y) = world_position_to_spawn_coords(transform.translation(), 0, 0);
        ground_skills.write(GroundSkillPlaced {
            skill_id: 89,
            src_id: gid,
            level: 10,
            x: x as u32,
            y: y as u32,
            server_tick: 0,
        });
    }
}
