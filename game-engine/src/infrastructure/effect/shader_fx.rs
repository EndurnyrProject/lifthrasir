use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap};

pub use lifthrasir_data::{
    ShaderFxEntry, ShaderFxGarnish, ShaderFxLight, ShaderFxTravel, TextureFrames,
};

/// Name-keyed procedural burst table, built from the `shader_fx` section of
/// `effects.ron` by `process_loaded_effect_data`.
#[derive(Resource)]
pub struct ShaderFxCatalog {
    entries: HashMap<String, ShaderFxEntry>,
}

impl ShaderFxCatalog {
    pub fn from_entries(data: BTreeMap<String, ShaderFxEntry>) -> Self {
        Self {
            entries: data.into_iter().collect(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&ShaderFxEntry> {
        self.entries.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_fx_section_deserializes_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let data = ron::from_str::<lifthrasir_data::EffectData>(ron).expect("deserialize");
        let catalog = ShaderFxCatalog::from_entries(data.shader_fx);

        let jupitel = catalog
            .get("jupitel_thunder")
            .expect("jupitel_thunder entry");
        assert_eq!(jupitel.kind, 0);
        assert_eq!(jupitel.primary, (3.5, 4.0, 6.0, 1.0));
        assert_eq!(jupitel.secondary, (0.25, 0.55, 3.2, 1.0));
        assert_eq!(jupitel.shape, (2.0, 7.0, 24.0, 0.0));
        assert_eq!(jupitel.duration, 0.7);
        assert_eq!(jupitel.scale, 26.0);

        let light = jupitel.light.as_ref().expect("jupitel_thunder light");
        assert_eq!(light.color, (0.55, 0.65, 1.0));
        assert_eq!(light.intensity_scale, 2.0);
        assert_eq!(light.fade, 0.22);

        let garnish = jupitel.garnish.as_ref().expect("jupitel_thunder garnish");
        assert_eq!(garnish.tint, (1.8, 2.4, 4.5, 1.0));

        assert_eq!(
            jupitel.texture.as_deref(),
            Some("data/texture/effect/thunder_pang.bmp")
        );

        let travel = jupitel.travel.as_ref().expect("jupitel_thunder travel");
        assert_eq!(travel.speed, 100.0);
        assert_eq!(travel.scale, 9.0);
        let frames = travel.frames.as_ref().expect("jupitel projectile frames");
        assert_eq!(frames.paths.len(), 6);
        assert_eq!(frames.fps, 18.0);
        assert_eq!(frames.paths[0], "data/texture/effect/thunder_ball_0001.bmp");

        // A single-orb bolt still travels, with no frame series.
        let fire = catalog.get("fire_bolt").expect("fire_bolt entry");
        let fire_travel = fire.travel.as_ref().expect("fire_bolt travel");
        assert_eq!(
            fire_travel.texture.as_deref(),
            Some("data/texture/effect/fireorb.bmp")
        );
        assert!(fire_travel.frames.is_none());
    }

    #[test]
    fn entry_deserializes_optional_texture() {
        let ron = r#"{
            "textured_fx": (
                kind: 1,
                primary: (1.0, 1.0, 1.0, 1.0),
                secondary: (1.0, 1.0, 1.0, 1.0),
                shape: (0.0, 0.0, 0.0, 0.0),
                duration: 0.5,
                scale: 10.0,
                texture: Some("data/texture/effect/fire_fall_b.bmp"),
            ),
        }"#;
        let entries = ron::from_str::<BTreeMap<String, ShaderFxEntry>>(ron).expect("deserialize");
        let entry = entries.get("textured_fx").expect("textured_fx entry");
        assert_eq!(
            entry.texture.as_deref(),
            Some("data/texture/effect/fire_fall_b.bmp")
        );
    }

    #[test]
    fn get_returns_none_for_unknown_key() {
        let catalog = ShaderFxCatalog::from_entries(Default::default());

        assert!(catalog.get("unknown_key").is_none());
    }
}
