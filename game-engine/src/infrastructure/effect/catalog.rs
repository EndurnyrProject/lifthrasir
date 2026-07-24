use super::shader_fx::ShaderFxCatalog;
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
    commands.insert_resource(ShaderFxCatalog::from_entries(asset.0.shader_fx.clone()));
    commands.remove_resource::<EffectDataHandle>();
    debug!("Effect catalogs created from RON");
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{EffectPlacement, GroundAnchor};

    #[test]
    fn deserializes_ron_into_effect_data() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.skills[&28].str.as_deref(), Some("heal.strfx.ron"));
        assert_eq!(asset.0.skills[&28].placement, EffectPlacement::Target);
        assert_eq!(asset.0.skills[&89].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&89].repeating);

        // id 18 is MG_FIREWALL: a looping sprite effect, not an STR.
        assert_eq!(asset.0.skills[&18].str, None);
        assert_eq!(
            asset.0.skills[&18].sprite.as_deref(),
            Some("이팩트/firewall")
        );
        assert_eq!(asset.0.skills[&18].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&18].repeating);

        // Safety Wall is a persistent single-cell unit and uses the native STR.
        assert_eq!(asset.0.skills[&12].str.as_deref(), Some("safetywall.str"));
        assert_eq!(asset.0.skills[&12].ground_anchor, GroundAnchor::Cell);
        assert!(asset.0.skills[&12].repeating);

        // id 5 is SM_BASH: authored slash effect, no procedural vfx.
        assert_eq!(asset.0.skills[&5].str.as_deref(), Some("bash.strfx.ron"));
        assert_eq!(asset.0.skills[&5].vfx, None);
        assert_eq!(
            asset.0.skills[&5].sound.as_deref(),
            Some("effect/ef_bash.wav")
        );

        // Knight: native STRs for 56-62, One-Hand Quicken (495) reuses the
        // Two-Hand Quicken STR, Charge Attack (1001) is authored.
        assert_eq!(asset.0.skills[&56].str.as_deref(), Some("pierce.str"));
        assert_eq!(asset.0.skills[&60].str.as_deref(), Some("twohand.str"));
        assert_eq!(asset.0.skills[&495].str.as_deref(), Some("twohand.str"));
        assert_eq!(
            asset.0.skills[&1001].str.as_deref(),
            Some("charge_attack.strfx.ron")
        );

        // ids 7/8 are SM_MAGNUM and SM_ENDURE: authored caster effects.
        assert_eq!(
            asset.0.skills[&7].str.as_deref(),
            Some("magnum_break.strfx.ron")
        );
        assert_eq!(asset.0.skills[&7].placement, EffectPlacement::Caster);
        assert_eq!(asset.0.skills[&8].str.as_deref(), Some("endure.strfx.ron"));
        assert_eq!(asset.0.skills[&8].placement, EffectPlacement::Caster);

        // id 28 is AL_HEAL: omits `vfx`, must default to None.
        assert_eq!(asset.0.skills[&28].vfx, None);

        // ids 14/19/20 are the authored bolt effects: MG_COLDBOLT, MG_FIREBOLT,
        // MG_LIGHTNINGBOLT, each pointing at its strfx.ron asset with no
        // procedural vfx fallback.
        assert_eq!(
            asset.0.skills[&14].str.as_deref(),
            Some("cold_bolt.strfx.ron")
        );
        assert_eq!(asset.0.skills[&14].vfx, None);
        assert_eq!(
            asset.0.skills[&19].str.as_deref(),
            Some("fire_bolt.strfx.ron")
        );
        assert_eq!(asset.0.skills[&19].vfx, None);
        assert_eq!(
            asset.0.skills[&20].str.as_deref(),
            Some("lightning_bolt.strfx.ron")
        );
        assert_eq!(asset.0.skills[&20].vfx, None);

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

        // id 26 is AL_TELEPORT: authored effect, caster-anchored, non-repeating.
        assert_eq!(
            asset.0.skills[&26].str.as_deref(),
            Some("teleport.strfx.ron")
        );
        assert_eq!(asset.0.skills[&26].placement, EffectPlacement::Caster);
        assert!(!asset.0.skills[&26].repeating);

        // id 68 is PR_ASPERSIO: official STR, target-anchored, non-repeating.
        assert_eq!(asset.0.skills[&68].str.as_deref(), Some("aspersio.str"));
        assert_eq!(asset.0.skills[&68].placement, EffectPlacement::Target);
        assert!(!asset.0.skills[&68].repeating);

        // id 78 is PR_LEXAETERNA: official STR, target-anchored, non-repeating.
        assert_eq!(asset.0.skills[&78].str.as_deref(), Some("lexaeterna.str"));
        assert_eq!(asset.0.skills[&78].placement, EffectPlacement::Target);
        assert!(!asset.0.skills[&78].repeating);

        // id 70 is PR_SANCTUARY: persistent ground field, Group-anchored (default).
        assert_eq!(asset.0.skills[&70].str.as_deref(), Some("sanctuary.str"));
        assert_eq!(asset.0.skills[&70].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&70].repeating);

        // id 79 is PR_MAGNUS: persistent ground field, Group-anchored (default).
        assert_eq!(asset.0.skills[&79].str.as_deref(), Some("magnus.str"));
        assert_eq!(asset.0.skills[&79].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&79].repeating);

        // id 27 is AL_WARP: authored open-portal loop, persistent ground
        // field, Group-anchored (default).
        assert_eq!(
            asset.0.skills[&27].str.as_deref(),
            Some("warp_portal.strfx.ron")
        );
        assert_eq!(asset.0.skills[&27].placement, EffectPlacement::Ground);
        assert!(asset.0.skills[&27].repeating);

        // id 271 is MO_EXTREMITYFIST (Asura Strike): authored blast on the
        // victim. id 270 is MO_EXPLOSIONSPIRITS (Fury): caster-anchored cast
        // burst, with the persistent thundershock crackle looping via the
        // `statuses:` entry (EFST 86).
        assert_eq!(
            asset.0.skills[&271].str.as_deref(),
            Some("asura_strike.strfx.ron")
        );
        assert_eq!(asset.0.skills[&271].placement, EffectPlacement::Target);
        assert_eq!(asset.0.skills[&270].placement, EffectPlacement::Caster);
        assert_eq!(
            asset.0.statuses[&86].str.as_deref(),
            Some("fury_sparks.strfx.ron")
        );
        assert!(asset.0.statuses[&86].repeating);
    }

    #[test]
    fn get_returns_seeded_target_and_ground_descriptors() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = EffectCatalog::from_skill_effect_data(asset.0.skills);

        let target = catalog.get(28).expect("AL_HEAL target descriptor");
        assert_eq!(target.str.as_deref(), Some("heal.strfx.ron"));
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
                19: (
                    str: Some("kyrie_min.str"),
                    vfx: None,
                    sound: None,
                    placement: Caster,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
                22: (
                    str: Some("lex_mark.strfx.ron"),
                    vfx: None,
                    sound: None,
                    placement: Target,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
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

        let kyrie = catalog.get(19).expect("EFST 19 (kyrie) descriptor");
        assert_eq!(kyrie.str.as_deref(), Some("kyrie_min.str"));
        assert!(kyrie.repeating);

        let lex_aeterna = catalog.get(22).expect("EFST 22 (lex aeterna) descriptor");
        assert_eq!(lex_aeterna.str.as_deref(), Some("lex_mark.strfx.ron"));
        assert!(lex_aeterna.repeating);

        let energy_coat = catalog.get(157).expect("EFST 157 descriptor");
        assert_eq!(energy_coat.str.as_deref(), Some("energycoat.str"));
        assert!(energy_coat.repeating);

        assert!(catalog.get(9999).is_none());
    }

    #[test]
    fn status_effect_catalog_seeded_with_energy_coat() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = StatusEffectCatalog::from_status_effect_data(asset.0.statuses);

        // 31 is EFST_ENERGYCOAT (aesir Efst.id(:energycoat)); 157 is the
        // MG_ENERGYCOAT *skill* id, a different namespace with no status entry.
        let energy_coat = catalog.get(31).expect("EFST_ENERGYCOAT descriptor");
        assert_eq!(energy_coat.str.as_deref(), Some("energycoat.str"));
        assert!(!energy_coat.repeating);
        assert!(catalog.get(157).is_none());

        // 19 is EFST_KYRIE (aesir Efst.id(:kyrie)) -- Kyrie Eleison's barrier
        // shimmer aura, a leaner GRF variant than the one-shot cast STR.
        let kyrie = catalog.get(19).expect("EFST_KYRIE descriptor");
        assert_eq!(kyrie.str.as_deref(), Some("kyrie_min.str"));
        assert!(!kyrie.repeating);

        // 22 is EFST_LEX_AETERNA (aesir Efst.id(:lexaeterna)) -- the pulsing
        // mark hovering above the marked unit while the status is active.
        let lex_aeterna = catalog.get(22).expect("EFST_LEX_AETERNA descriptor");
        assert_eq!(lex_aeterna.str.as_deref(), Some("lex_mark.strfx.ron"));
        assert!(lex_aeterna.repeating);
    }

    #[test]
    fn crusader_skill_effects_ron_deserialize_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = EffectCatalog::from_skill_effect_data(asset.0.skills);

        // The approved Crusader skill table (249-258 + quest 1002): exact
        // native/authored assets, placements, and the three verified
        // dedicated sounds. Every entry is a non-repeating one-shot.
        let expected: [(u32, &str, EffectPlacement, Option<&str>); 11] = [
            (249, "kyrie.str", EffectPlacement::Caster, None),
            (250, "shield_charge.str", EffectPlacement::Target, None),
            (
                251,
                "shield_boomerang.strfx.ron",
                EffectPlacement::Target,
                Some("effect/cru_shield boomerang.wav"),
            ),
            (
                252,
                "reflect_shield.strfx.ron",
                EffectPlacement::Caster,
                None,
            ),
            (
                253,
                "holy_cross.str",
                EffectPlacement::Target,
                Some("effect/cru_holy cross.wav"),
            ),
            (
                254,
                "grand_cross.strfx.ron",
                EffectPlacement::Ground,
                Some("effect/cru_grand cross.wav"),
            ),
            (255, "devotion.str", EffectPlacement::Target, None),
            (256, "providence.str", EffectPlacement::Target, None),
            (257, "defense.str", EffectPlacement::Caster, None),
            (
                258,
                "spear_quicken.strfx.ron",
                EffectPlacement::Caster,
                None,
            ),
            (1002, "shrink.strfx.ron", EffectPlacement::Caster, None),
        ];
        for (skill_id, asset_name, placement, sound) in expected {
            let descriptor = catalog
                .get(skill_id)
                .unwrap_or_else(|| panic!("skill {skill_id} descriptor"));
            assert_eq!(
                descriptor.str.as_deref(),
                Some(asset_name),
                "skill {skill_id}"
            );
            assert_eq!(descriptor.placement, placement, "skill {skill_id}");
            assert_eq!(descriptor.sound.as_deref(), sound, "skill {skill_id}");
            assert!(!descriptor.repeating, "skill {skill_id}");
        }

        // CR_FAITH (248) is a passive with no cast event: deliberately absent.
        assert!(catalog.get(248).is_none());
    }

    #[test]
    fn crusader_map_effects_ron_deserialize_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = MapEffectCatalog::from_effect_data(asset.0.map);

        // EF_REFLECTSHIELD (252): Reflect Shield's reactive proc flash on the
        // reflecting unit; reuses the authored skill-252 asset.
        let reflect = catalog.get(252).expect("EF_REFLECTSHIELD descriptor");
        assert_eq!(reflect.str.as_deref(), Some("reflect_shield.strfx.ron"));
        assert_eq!(reflect.sound, None);
        assert_eq!(reflect.placement, EffectPlacement::Ground);
        assert!(!reflect.repeating);

        // EF_GUARD (336): Auto Guard's reactive proc flash on the guarding
        // unit, using the native kyrie STR.
        let guard = catalog.get(336).expect("EF_GUARD descriptor");
        assert_eq!(guard.str.as_deref(), Some("kyrie.str"));
        assert_eq!(guard.sound, None);
        assert_eq!(guard.placement, EffectPlacement::Ground);
        assert!(!guard.repeating);
    }

    #[test]
    fn crusader_status_effects_ron_deserialize_into_catalog() {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = StatusEffectCatalog::from_status_effect_data(asset.0.statuses);

        // Persistent auras: shared shield family (58/59/62), shared holy
        // family (60/61), dedicated Spear Quicken (68) and Shrink (197)
        // loops. All repeating, all soundless.
        let expected: [(u32, &str); 7] = [
            (58, "crusader_shield_aura.strfx.ron"),
            (59, "crusader_shield_aura.strfx.ron"),
            (60, "crusader_holy_aura.strfx.ron"),
            (61, "crusader_holy_aura.strfx.ron"),
            (62, "crusader_shield_aura.strfx.ron"),
            (68, "spear_quicken_aura.strfx.ron"),
            (197, "shrink_aura.strfx.ron"),
        ];
        for (efst_id, asset_name) in expected {
            let descriptor = catalog
                .get(efst_id)
                .unwrap_or_else(|| panic!("EFST {efst_id} descriptor"));
            assert_eq!(
                descriptor.str.as_deref(),
                Some(asset_name),
                "EFST {efst_id}"
            );
            assert!(descriptor.repeating, "EFST {efst_id}");
            assert_eq!(descriptor.sound, None, "EFST {efst_id}");
            assert_eq!(
                descriptor.placement,
                EffectPlacement::Caster,
                "EFST {efst_id}"
            );
            assert!(descriptor.color[3] < 1.0, "EFST {efst_id}");
        }

        // Statuses sharing an aura family stay visually distinct through
        // their descriptor tint.
        let auto_guard = catalog.get(58).expect("EFST_AUTOGUARD").color;
        let reflect_shield = catalog.get(59).expect("EFST_REFLECTSHIELD").color;
        let defender = catalog.get(62).expect("EFST_DEFENDER").color;
        assert_ne!(auto_guard, reflect_shield);
        assert_ne!(auto_guard, defender);
        assert_ne!(reflect_shield, defender);
        assert_ne!(
            catalog.get(60).expect("EFST_DEVOTION").color,
            catalog.get(61).expect("EFST_PROVIDENCE").color
        );
    }
}
