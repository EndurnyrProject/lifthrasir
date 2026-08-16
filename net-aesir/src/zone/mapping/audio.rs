use bevy::prelude::warn;
use net_contract::events::PlaySoundEffect;

use crate::proto::aesir::net;

pub fn sound_effect(effect: net::SoundEffect) -> Option<PlaySoundEffect> {
    if effect.r#type != 0 {
        warn!(
            "unsupported SoundEffect type {}; skipping '{}'",
            effect.r#type, effect.name
        );
        return None;
    }

    Some(PlaySoundEffect { name: effect.name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_effect_maps_one_shot_filename() {
        let event = sound_effect(net::SoundEffect {
            name: "effect\\door.wav".into(),
            r#type: 0,
        })
        .expect("type 0 should be supported");

        assert_eq!(event.name, "effect\\door.wav");
    }

    #[test]
    fn sound_effect_skips_unsupported_type() {
        let event = sound_effect(net::SoundEffect {
            name: "loop.wav".into(),
            r#type: 1,
        });

        assert!(event.is_none());
    }
}
