use bevy::asset::LoadState;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct SkillEffectDataAsset(pub lifthrasir_data::SkillEffectData);

#[derive(Resource)]
pub struct EffectCatalog {
    effects: HashMap<u32, lifthrasir_data::EffectDescriptor>,
}

impl EffectCatalog {
    pub fn from_skill_effect_data(data: lifthrasir_data::SkillEffectData) -> Self {
        Self {
            effects: data.effects.into_iter().collect(),
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
    pub fn from_effect_data(data: lifthrasir_data::SkillEffectData) -> Self {
        Self {
            effects: data.effects.into_iter().collect(),
        }
    }

    pub fn get(&self, effect_type: u32) -> Option<&lifthrasir_data::EffectDescriptor> {
        self.effects.get(&effect_type)
    }
}

#[derive(Resource)]
pub struct SkillEffectDataHandle(Handle<SkillEffectDataAsset>);

#[derive(Resource)]
pub struct MapEffectDataHandle(Handle<SkillEffectDataAsset>);

pub fn start_loading_skill_effect_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/skill_effects.ron");
    commands.insert_resource(SkillEffectDataHandle(handle));
    debug!("Loading skill effect data RON");
}

pub fn start_loading_map_effect_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("data/ron/map_effects.ron");
    commands.insert_resource(MapEffectDataHandle(handle));
    debug!("Loading map effect data RON");
}

pub fn process_loaded_skill_effect_data(
    mut commands: Commands,
    handle: Option<Res<SkillEffectDataHandle>>,
    skill_effect_data_assets: Res<Assets<SkillEffectDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/skill_effects.ron: {:?}. It is hand-authored at assets/data/ron/skill_effects.ron.",
            err
        );
        commands.remove_resource::<SkillEffectDataHandle>();
        return;
    }

    let Some(asset) = skill_effect_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(EffectCatalog::from_skill_effect_data(asset.0.clone()));
    commands.remove_resource::<SkillEffectDataHandle>();
    debug!("Effect catalog created from RON");
}

pub fn process_loaded_map_effect_data(
    mut commands: Commands,
    handle: Option<Res<MapEffectDataHandle>>,
    map_effect_data_assets: Res<Assets<SkillEffectDataAsset>>,
    asset_server: Res<AssetServer>,
) {
    let Some(handle) = handle else { return };

    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        error!(
            "Failed to load data/ron/map_effects.ron: {:?}. It is hand-authored at assets/data/ron/map_effects.ron.",
            err
        );
        commands.remove_resource::<MapEffectDataHandle>();
        return;
    }

    let Some(asset) = map_effect_data_assets.get(&handle.0) else {
        return;
    };

    commands.insert_resource(MapEffectCatalog::from_effect_data(asset.0.clone()));
    commands.remove_resource::<MapEffectDataHandle>();
    debug!("Map effect catalog created from RON");
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::EffectPlacement;

    #[test]
    fn deserializes_ron_into_skill_effect_data() {
        let ron = include_str!("../../../../assets/data/ron/skill_effects.ron");
        let asset = ron::from_str::<SkillEffectDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.effects[&28].str, "heal.str");
        assert_eq!(asset.0.effects[&28].placement, EffectPlacement::Target);
        assert_eq!(asset.0.effects[&89].placement, EffectPlacement::Ground);
        assert!(asset.0.effects[&89].repeating);

        // id 18 is MG_FIREWALL (was a stale magnus.str mapping).
        assert_eq!(asset.0.effects[&18].str, "firewall.str");
        assert_eq!(asset.0.effects[&18].placement, EffectPlacement::Ground);
        assert!(asset.0.effects[&18].repeating);
        assert!(
            asset.0.effects.values().all(|e| e.str != "magnus.str"),
            "magnus.str must not be referenced by the skill catalog"
        );

        // Bucket-A samples: one ground field and one caster buff.
        assert_eq!(asset.0.effects[&21].str, "thunderstorm.str");
        assert_eq!(asset.0.effects[&21].placement, EffectPlacement::Ground);
        assert!(asset.0.effects[&21].repeating);
        assert_eq!(asset.0.effects[&33].str, "angelus.str");
        assert_eq!(asset.0.effects[&33].placement, EffectPlacement::Caster);
        assert!(!asset.0.effects[&33].repeating);
    }

    #[test]
    fn get_returns_seeded_target_and_ground_descriptors() {
        let ron = include_str!("../../../../assets/data/ron/skill_effects.ron");
        let asset = ron::from_str::<SkillEffectDataAsset>(ron).expect("deserialize");
        let catalog = EffectCatalog::from_skill_effect_data(asset.0);

        let target = catalog.get(28).expect("AL_HEAL target descriptor");
        assert_eq!(target.str, "heal.str");
        assert_eq!(target.placement, EffectPlacement::Target);

        let ground = catalog.get(89).expect("WZ_STORMGUST ground descriptor");
        assert_eq!(ground.str, "stormgust.str");
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
        let ron = include_str!("../../../../assets/data/ron/map_effects.ron");
        let asset = ron::from_str::<SkillEffectDataAsset>(ron).expect("deserialize");
        let catalog = MapEffectCatalog::from_effect_data(asset.0);

        let stormgust = catalog.get(89).expect("EF_STORMGUST descriptor");
        assert_eq!(stormgust.str, "stormgust.str");
        assert!(stormgust.repeating);

        assert!(catalog.get(9999).is_none());
    }
}
