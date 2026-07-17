use bevy::asset::LoadState;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct EffectDataAsset(pub lifthrasir_data::EffectData);

#[derive(Resource)]
pub struct EffectCatalog {
    effects: HashMap<u32, lifthrasir_data::EffectDescriptor>,
}

impl EffectCatalog {
    pub fn from_skill_effect_data(data: BTreeMap<u32, lifthrasir_data::EffectDescriptor>) -> Self {
        Self {
            effects: data.into_iter().collect(),
        }
    }

    pub fn get(&self, skill_id: u32) -> Option<&lifthrasir_data::EffectDescriptor> {
        self.effects.get(&skill_id)
    }
}

/// Map-placed effects, keyed by RSW `effect_type` (the rAthena
/// `e_special_effects` EF_* id, the same namespace aesir's `SpecialEffect`
/// packet uses). Reuses `EffectDescriptor`; `placement` is ignored since map
/// effects always anchor at their RSW position.
#[derive(Resource)]
pub struct MapEffectCatalog {
    effects: HashMap<u32, lifthrasir_data::EffectDescriptor>,
}

impl MapEffectCatalog {
    pub fn from_effect_data(data: BTreeMap<u32, lifthrasir_data::EffectDescriptor>) -> Self {
        Self {
            effects: data.into_iter().collect(),
        }
    }

    pub fn get(&self, effect_type: u32) -> Option<&lifthrasir_data::EffectDescriptor> {
        self.effects.get(&effect_type)
    }
}

/// Persistent status-aura descriptors, keyed by EFST id (e.g. Energy Coat).
/// Reuses `EffectDescriptor`; `placement`/`ground_anchor` are ignored.
#[derive(Resource)]
pub struct StatusEffectCatalog {
    effects: HashMap<u32, lifthrasir_data::EffectDescriptor>,
}

impl StatusEffectCatalog {
    pub fn from_status_effect_data(data: BTreeMap<u32, lifthrasir_data::EffectDescriptor>) -> Self {
        Self {
            effects: data.into_iter().collect(),
        }
    }

    pub fn get(&self, efst_id: u32) -> Option<&lifthrasir_data::EffectDescriptor> {
        self.effects.get(&efst_id)
    }
}

#[derive(Resource)]
pub struct EffectDataHandle(Handle<EffectDataAsset>);

pub fn start_loading_effect_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/effects.ron");
    commands.insert_resource(EffectDataHandle(handle));
    debug!("Loading effect data RON");
}

pub fn process_loaded_effect_data(
    mut commands: Commands,
    handle: Option<Res<EffectDataHandle>>,
    effect_data_assets: Res<Assets<EffectDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/effects.ron: {:?}. It is hand-authored at assets/data/ron/effects.ron.",
            err
        );
        commands.remove_resource::<EffectDataHandle>();
        return;
    }

    let Some(asset) = effect_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(EffectCatalog::from_skill_effect_data(
        asset.0.skills.clone(),
    ));
    commands.insert_resource(MapEffectCatalog::from_effect_data(asset.0.map.clone()));
    commands.insert_resource(StatusEffectCatalog::from_status_effect_data(
        asset.0.statuses.clone(),
    ));
    commands.remove_resource::<EffectDataHandle>();
    debug!("Effect catalogs created from RON");
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::EffectPlacement;

    #[test]
    fn deserializes_ron_into_effect_data() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.skills[&28].str.as_deref(), Some("heal.str"));
        assert_eq!(asset.0.skills[&28].placement, EffectPlacement::Target);
        assert_eq!(asset.0.skills[&89].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&89].repeating);

        // id 18 is MG_FIREWALL (was a stale magnus.str mapping).
        assert_eq!(asset.0.skills[&18].str.as_deref(), Some("firewall.str"));
        assert_eq!(asset.0.skills[&18].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&18].repeating);
        assert!(
            asset
                .0
                .skills
                .values()
                .all(|e| e.str.as_deref() != Some("magnus.str")),
            "magnus.str must not be referenced by the skill catalog"
        );

        // id 5 is SM_BASH: sound-only, no STR effect, procedural vfx key "bash".
        assert_eq!(asset.0.skills[&5].str, None);
        assert_eq!(asset.0.skills[&5].vfx.as_deref(), Some("bash"));
        assert_eq!(
            asset.0.skills[&5].sound.as_deref(),
            Some("effect/ef_bash.wav")
        );

        // id 28 is AL_HEAL: omits `vfx`, must default to None.
        assert_eq!(asset.0.skills[&28].vfx, None);

        // Bucket-A samples: one ground field and one caster buff.
        assert_eq!(asset.0.skills[&21].str.as_deref(), Some("thunderstorm.str"));
        assert_eq!(asset.0.skills[&21].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&21].repeating);
        assert_eq!(asset.0.skills[&33].str.as_deref(), Some("angelus.str"));
        assert_eq!(asset.0.skills[&33].placement, EffectPlacement::Caster);
        assert!(!asset.0.skills[&33].repeating);

        // Sound paths are relative to `data/wav/` (see `mob_sfx_path`). These two
        // were broken: `_heal_effect.wav` lives at the wav root (no `effect/`
        // prefix), and Storm Gust's only sound is `wizard_stormgust.wav` —
        // `effect/stormgust.wav` does not exist in the GRF.
        assert_eq!(
            asset.0.skills[&28].sound.as_deref(),
            Some("_heal_effect.wav")
        );
        assert_eq!(
            asset.0.skills[&89].sound.as_deref(),
            Some("effect/wizard_stormgust.wav")
        );
    }

    #[test]
    fn get_returns_seeded_target_and_ground_descriptors() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = EffectCatalog::from_skill_effect_data(asset.0.skills);

        let target = catalog.get(28).expect("AL_HEAL target descriptor");
        assert_eq!(target.str.as_deref(), Some("heal.str"));
        assert_eq!(target.placement, EffectPlacement::Target);

        let ground = catalog.get(89).expect("WZ_STORMGUST ground descriptor");
        assert_eq!(ground.str.as_deref(), Some("stormgust.str"));
        assert_eq!(ground.placement, EffectPlacement::Ground);
        assert!(ground.repeating);
    }

    #[test]
    fn get_returns_none_for_unknown_skill_id() {
        let catalog = EffectCatalog::from_skill_effect_data(Default::default());

        assert!(catalog.get(9999).is_none());
    }

    #[test]
    fn map_effects_ron_deserializes_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = MapEffectCatalog::from_effect_data(asset.0.map);

        let stormgust = catalog.get(89).expect("EF_STORMGUST descriptor");
        assert_eq!(stormgust.str.as_deref(), Some("stormgust.str"));
        assert!(stormgust.repeating);

        let magnus = catalog.get(113).expect("EF_MAGNUS descriptor");
        assert_eq!(magnus.str.as_deref(), Some("magnus.str"));

        assert!(catalog.get(9999).is_none());
    }

    #[test]
    fn status_effects_ron_round_trips_into_catalog() {
        let ron = r#"(
            skills: {},
            map: {},
            statuses: {
                157: (
                    str: Some("energycoat.str"),
                    vfx: None,
                    sound: None,
                    placement: Caster,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
            },
        )"#;
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = StatusEffectCatalog::from_status_effect_data(asset.0.statuses);

        let energy_coat = catalog.get(157).expect("EFST 157 descriptor");
        assert_eq!(energy_coat.str.as_deref(), Some("energycoat.str"));
        assert!(energy_coat.repeating);

        assert!(catalog.get(9999).is_none());
    }

    #[test]
    fn status_effect_catalog_empty_for_seeded_ron() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = StatusEffectCatalog::from_status_effect_data(asset.0.statuses);

        assert!(catalog.get(157).is_none());
    }
}
