use super::shader_fx::ShaderFxCatalog;
use bevy::asset::LoadState;
use bevy::prelude::*;
use lifthrasir_data::{EffectData, EffectDescriptor, ShaderFxEntry, Visual};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct EffectDataAsset(pub lifthrasir_data::EffectData);

/// The three descriptor tables of `effects.ron`, resolved and validated once at
/// load: `skills` (rAthena skill id), `special` (the `e_special_effects` EF_*
/// id, driving both RSW map effects and aesir `SpecialEffect` packets), and
/// `efsts` (EFST id). The id namespaces are distinct, so the tables stay
/// separate.
///
/// `Visual::Efst` layers in `skills`/`special` are spliced with the referenced
/// `efsts` recipe's layers here, so lookups return descriptors whose `visuals`
/// carry no composition left to resolve.
#[derive(Resource, Debug)]
pub struct EffectCatalog {
    skills: HashMap<u32, EffectDescriptor>,
    special: HashMap<u32, EffectDescriptor>,
    efsts: HashMap<u32, EffectDescriptor>,
}

impl EffectCatalog {
    /// Splice `Efst` composition and validate every table. `Err` carries a
    /// message naming the offending section and id, ready for `error!`.
    pub fn build(data: &EffectData) -> Result<Self, String> {
        for (id, descriptor) in &data.efsts {
            if let Some(nested) = first_efst_ref(descriptor) {
                return Err(format!(
                    "efsts[{id}] contains Efst({nested}); efst recipes are one level deep and must not compose other recipes"
                ));
            }
        }

        let skills = resolve_section("skills", &data.skills, &data.efsts)?;
        let special = resolve_section("special", &data.special, &data.efsts)?;

        for (section, table) in [
            ("skills", &skills),
            ("special", &special),
            ("efsts", &data.efsts),
        ] {
            for (id, descriptor) in table {
                validate_descriptor(section, *id, descriptor, &data.shader_fx)?;
            }
        }

        Ok(Self {
            skills: skills.into_iter().collect(),
            special: special.into_iter().collect(),
            efsts: data.efsts.clone().into_iter().collect(),
        })
    }

    pub fn skill(&self, skill_id: u32) -> Option<&EffectDescriptor> {
        self.skills.get(&skill_id)
    }

    pub fn special(&self, effect_id: u32) -> Option<&EffectDescriptor> {
        self.special.get(&effect_id)
    }

    pub fn efst(&self, efst_id: u32) -> Option<&EffectDescriptor> {
        self.efsts.get(&efst_id)
    }
}

/// The first `Efst` layer's target id, if the descriptor composes one.
fn first_efst_ref(descriptor: &EffectDescriptor) -> Option<u32> {
    descriptor.visuals.iter().find_map(|visual| match visual {
        Visual::Efst(id) => Some(*id),
        _ => None,
    })
}

/// Replace every `Efst(id)` layer with the referenced recipe's layers, in
/// place. Only `visuals` is touched: the composing entry keeps its own
/// placement, repeating, color and sound.
fn resolve_section(
    section: &str,
    table: &BTreeMap<u32, EffectDescriptor>,
    efsts: &BTreeMap<u32, EffectDescriptor>,
) -> Result<BTreeMap<u32, EffectDescriptor>, String> {
    table
        .iter()
        .map(|(id, descriptor)| {
            let mut resolved = descriptor.clone();
            resolved.visuals = Vec::new();
            for visual in &descriptor.visuals {
                let Visual::Efst(efst_id) = visual else {
                    resolved.visuals.push(visual.clone());
                    continue;
                };
                let recipe = efsts.get(efst_id).ok_or_else(|| {
                    format!("{section}[{id}] composes Efst({efst_id}), which has no efsts entry")
                })?;
                resolved.visuals.extend(recipe.visuals.iter().cloned());
            }
            Ok((*id, resolved))
        })
        .collect()
}

fn kind_name(visual: &Visual) -> &'static str {
    match visual {
        Visual::Str(_) => "Str",
        Visual::Sprite(_) => "Sprite",
        Visual::Model(_) => "Model",
        Visual::Shader(_) => "Shader",
        Visual::Bespoke(_) => "Bespoke",
        Visual::Efst(_) => "Efst",
    }
}

/// Every `Shader` layer must name a real `shader_fx` entry, and a resolved
/// descriptor may carry at most one layer of each kind.
///
/// NOTE: the one-layer-per-kind ceiling is deliberate — the dispatch code reads
/// the first layer of a kind, so a second would render as a silent drop. Lift it
/// (accessors returning iterators, dispatch looping) when a real effect needs
/// two layers of one kind.
fn validate_descriptor(
    section: &str,
    id: u32,
    descriptor: &EffectDescriptor,
    shader_fx: &BTreeMap<String, ShaderFxEntry>,
) -> Result<(), String> {
    let mut seen: Vec<&'static str> = Vec::with_capacity(descriptor.visuals.len());
    for visual in &descriptor.visuals {
        if let Visual::Shader(key) = visual
            && !shader_fx.contains_key(key)
        {
            return Err(format!(
                "{section}[{id}] references Shader(\"{key}\"), which has no shader_fx entry"
            ));
        }

        let kind = kind_name(visual);
        if seen.contains(&kind) {
            return Err(format!(
                "{section}[{id}] carries more than one {kind} layer; at most one layer per kind is supported"
            ));
        }
        seen.push(kind);
    }
    Ok(())
}

#[derive(Resource)]
pub struct EffectDataHandle(Handle<EffectDataAsset>);

pub fn start_loading_effect_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load("ro://ron/effects.ron");
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
            "Failed to load ro://ron/effects.ron: {:?}. It is hand-authored at assets/data/ron/effects.ron.",
            err
        );
        commands.remove_resource::<EffectDataHandle>();
        return;
    }

    let Some(asset) = effect_data_assets.get(&handle.0) else {
        return;
    };

    let catalog = match EffectCatalog::build(&asset.0) {
        Ok(catalog) => catalog,
        Err(message) => {
            error!(
                "Invalid ro://ron/effects.ron: {message}. It is hand-authored at assets/data/ron/effects.ron; no effect catalog is available."
            );
            commands.remove_resource::<EffectDataHandle>();
            return;
        }
    };

    commands.insert_resource(catalog);
    commands.insert_resource(ShaderFxCatalog::from_entries(asset.0.shader_fx.clone()));
    commands.remove_resource::<EffectDataHandle>();
    debug!("Effect catalogs created from RON");
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{EffectPlacement, GroundAnchor};

    fn seed_data() -> EffectData {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        ron::from_str::<EffectDataAsset>(ron)
            .expect("deserialize")
            .0
    }

    fn seeded_catalog() -> EffectCatalog {
        EffectCatalog::build(&seed_data()).expect("seed catalog builds")
    }

    /// A descriptor carrying exactly the given layers, with inert other fields.
    fn descriptor(visuals: Vec<Visual>) -> EffectDescriptor {
        EffectDescriptor {
            visuals,
            ..Default::default()
        }
    }

    #[test]
    fn deserializes_ron_into_effect_data() {
        let data = seed_data();

        assert_eq!(data.skills[&28].str_name(), Some("heal.strfx.ron"));
        assert_eq!(data.skills[&28].placement, EffectPlacement::Target);
        assert_eq!(data.skills[&89].placement, EffectPlacement::Ground);
        assert!(data.skills[&89].repeating);

        // id 18 is MG_FIREWALL: a looping sprite effect, not an STR.
        assert_eq!(data.skills[&18].str_name(), None);
        assert_eq!(data.skills[&18].sprite_stem(), Some("이팩트/firewall"));
        assert_eq!(data.skills[&18].placement, EffectPlacement::Ground);
        assert!(data.skills[&18].repeating);

        // Safety Wall is a persistent single-cell unit and uses the native STR.
        assert_eq!(data.skills[&12].str_name(), Some("safetywall.str"));
        assert_eq!(data.skills[&12].ground_anchor, GroundAnchor::Cell);
        assert!(data.skills[&12].repeating);

        // id 5 is SM_BASH: authored slash effect, no procedural layer.
        assert_eq!(data.skills[&5].str_name(), Some("bash.strfx.ron"));
        assert_eq!(data.skills[&5].shader_key(), None);
        assert_eq!(data.skills[&5].sound.as_deref(), Some("effect/ef_bash.wav"));

        // Knight: native STRs for 56-62, One-Hand Quicken (495) reuses the
        // Two-Hand Quicken STR, Charge Attack (1001) is authored.
        assert_eq!(data.skills[&56].str_name(), Some("pierce.str"));
        assert_eq!(data.skills[&60].str_name(), Some("twohand.str"));
        assert_eq!(data.skills[&495].str_name(), Some("twohand.str"));
        assert_eq!(
            data.skills[&1001].str_name(),
            Some("charge_attack.strfx.ron")
        );

        // ids 7/8 are SM_MAGNUM and SM_ENDURE: authored caster effects.
        assert_eq!(data.skills[&7].str_name(), Some("magnum_break.strfx.ron"));
        assert_eq!(data.skills[&7].placement, EffectPlacement::Caster);
        assert_eq!(data.skills[&8].str_name(), Some("endure.strfx.ron"));
        assert_eq!(data.skills[&8].placement, EffectPlacement::Caster);

        // id 28 is AL_HEAL: a single STR layer, nothing procedural.
        assert_eq!(data.skills[&28].shader_key(), None);
        assert_eq!(data.skills[&28].bespoke_key(), None);

        // ids 14/19/20 are the authored bolt effects: MG_COLDBOLT, MG_FIREBOLT,
        // MG_LIGHTNINGBOLT, each a lone authored strfx layer with no procedural
        // fallback.
        assert_eq!(data.skills[&14].str_name(), Some("cold_bolt.strfx.ron"));
        assert_eq!(data.skills[&14].shader_key(), None);
        assert_eq!(data.skills[&19].str_name(), Some("fire_bolt.strfx.ron"));
        assert_eq!(data.skills[&19].shader_key(), None);
        assert_eq!(
            data.skills[&20].str_name(),
            Some("lightning_bolt.strfx.ron")
        );
        assert_eq!(data.skills[&20].shader_key(), None);

        // Bucket-A samples: one ground field and one caster buff.
        assert_eq!(data.skills[&21].str_name(), Some("thunderstorm.str"));
        assert_eq!(data.skills[&21].placement, EffectPlacement::Ground);
        assert!(data.skills[&21].repeating);
        assert_eq!(data.skills[&33].str_name(), Some("angelus.str"));
        assert_eq!(data.skills[&33].placement, EffectPlacement::Caster);
        assert!(!data.skills[&33].repeating);

        // Sound paths are relative to `data/wav/` (see `mob_sfx_path`). These two
        // were broken: `_heal_effect.wav` lives at the wav root (no `effect/`
        // prefix), and Storm Gust's only sound is `wizard_stormgust.wav` —
        // `effect/stormgust.wav` does not exist in the GRF.
        assert_eq!(data.skills[&28].sound.as_deref(), Some("_heal_effect.wav"));
        assert_eq!(
            data.skills[&89].sound.as_deref(),
            Some("effect/wizard_stormgust.wav")
        );

        // id 26 is AL_TELEPORT: authored effect, caster-anchored, non-repeating.
        assert_eq!(data.skills[&26].str_name(), Some("teleport.strfx.ron"));
        assert_eq!(data.skills[&26].placement, EffectPlacement::Caster);
        assert!(!data.skills[&26].repeating);

        // id 68 is PR_ASPERSIO: official STR, target-anchored, non-repeating.
        assert_eq!(data.skills[&68].str_name(), Some("aspersio.str"));
        assert_eq!(data.skills[&68].placement, EffectPlacement::Target);
        assert!(!data.skills[&68].repeating);

        // id 78 is PR_LEXAETERNA: official STR, target-anchored, non-repeating.
        assert_eq!(data.skills[&78].str_name(), Some("lexaeterna.str"));
        assert_eq!(data.skills[&78].placement, EffectPlacement::Target);
        assert!(!data.skills[&78].repeating);

        // id 70 is PR_SANCTUARY: persistent ground field, Group-anchored (default).
        assert_eq!(data.skills[&70].str_name(), Some("sanctuary.str"));
        assert_eq!(data.skills[&70].placement, EffectPlacement::Ground);
        assert!(data.skills[&70].repeating);

        // id 79 is PR_MAGNUS: persistent ground field, Group-anchored (default).
        assert_eq!(data.skills[&79].str_name(), Some("magnus.str"));
        assert_eq!(data.skills[&79].placement, EffectPlacement::Ground);
        assert!(data.skills[&79].repeating);

        // id 27 is AL_WARP: authored open-portal loop, persistent ground
        // field, Group-anchored (default).
        assert_eq!(data.skills[&27].str_name(), Some("warp_portal.strfx.ron"));
        assert_eq!(data.skills[&27].placement, EffectPlacement::Ground);
        assert!(data.skills[&27].repeating);

        // id 271 is MO_EXTREMITYFIST (Asura Strike): authored blast on the
        // victim. id 270 is MO_EXPLOSIONSPIRITS (Fury): caster-anchored cast
        // burst, with the persistent thundershock crackle looping via the
        // `efsts:` entry (EFST 86).
        assert_eq!(data.skills[&271].str_name(), Some("asura_strike.strfx.ron"));
        assert_eq!(data.skills[&271].placement, EffectPlacement::Target);
        assert_eq!(data.skills[&270].placement, EffectPlacement::Caster);
        assert_eq!(data.efsts[&86].str_name(), Some("fury_sparks.strfx.ron"));
        assert!(data.efsts[&86].repeating);
    }

    #[test]
    fn skill_returns_seeded_target_and_ground_descriptors() {
        let catalog = seeded_catalog();

        let target = catalog.skill(28).expect("AL_HEAL target descriptor");
        assert_eq!(target.str_name(), Some("heal.strfx.ron"));
        assert_eq!(target.placement, EffectPlacement::Target);

        let ground = catalog.skill(89).expect("WZ_STORMGUST ground descriptor");
        assert_eq!(ground.str_name(), Some("stormgust.str"));
        assert_eq!(ground.placement, EffectPlacement::Ground);
        assert!(ground.repeating);
    }

    #[test]
    fn lookups_return_none_for_unknown_ids() {
        let catalog = EffectCatalog::build(&EffectData::default()).expect("empty catalog builds");

        assert!(catalog.skill(9999).is_none());
        assert!(catalog.special(9999).is_none());
        assert!(catalog.efst(9999).is_none());
    }

    #[test]
    fn special_effects_ron_deserializes_into_catalog() {
        let catalog = seeded_catalog();

        let stormgust = catalog.special(89).expect("EF_STORMGUST descriptor");
        assert_eq!(stormgust.str_name(), Some("stormgust.str"));
        assert!(stormgust.repeating);

        let magnus = catalog.special(113).expect("EF_MAGNUS descriptor");
        assert_eq!(magnus.str_name(), Some("magnus.str"));

        // The ambient EF_* ids are Bespoke layers dispatched to code spawners,
        // never to `shader_fx`.
        assert_eq!(
            catalog.special(44).expect("EF_SMOKE").bespoke_key(),
            Some("smoke")
        );
        assert_eq!(
            catalog.special(974).expect("EF_EMITTER").bespoke_key(),
            Some("emitter")
        );

        assert!(catalog.special(9999).is_none());
    }

    #[test]
    fn efsts_ron_round_trips_into_catalog() {
        let ron = r#"(
            skills: {},
            special: {},
            efsts: {
                19: (
                    visuals: [Str("kyrie_min.str")],
                    sound: None,
                    placement: Caster,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
                22: (
                    visuals: [Str("lex_mark.strfx.ron")],
                    sound: None,
                    placement: Target,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
                157: (
                    visuals: [Str("energycoat.str")],
                    sound: None,
                    placement: Caster,
                    color: (1.0, 1.0, 1.0, 1.0),
                    repeating: true,
                ),
            },
        )"#;
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("deserialize");
        let catalog = EffectCatalog::build(&asset.0).expect("catalog builds");

        let kyrie = catalog.efst(19).expect("EFST 19 (kyrie) descriptor");
        assert_eq!(kyrie.str_name(), Some("kyrie_min.str"));
        assert!(kyrie.repeating);

        let lex_aeterna = catalog.efst(22).expect("EFST 22 (lex aeterna) descriptor");
        assert_eq!(lex_aeterna.str_name(), Some("lex_mark.strfx.ron"));
        assert!(lex_aeterna.repeating);

        let energy_coat = catalog.efst(157).expect("EFST 157 descriptor");
        assert_eq!(energy_coat.str_name(), Some("energycoat.str"));
        assert!(energy_coat.repeating);

        assert!(catalog.efst(9999).is_none());
    }

    #[test]
    fn efst_catalog_seeded_with_energy_coat() {
        let catalog = seeded_catalog();

        // 31 is EFST_ENERGYCOAT (aesir Efst.id(:energycoat)); 157 is the
        // MG_ENERGYCOAT *skill* id, a different namespace with no EFST entry.
        let energy_coat = catalog.efst(31).expect("EFST_ENERGYCOAT descriptor");
        assert_eq!(energy_coat.str_name(), Some("energycoat.str"));
        assert!(!energy_coat.repeating);
        assert!(catalog.efst(157).is_none());

        // 19 is EFST_KYRIE (aesir Efst.id(:kyrie)) -- Kyrie Eleison's barrier
        // shimmer aura, a leaner GRF variant than the one-shot cast STR.
        let kyrie = catalog.efst(19).expect("EFST_KYRIE descriptor");
        assert_eq!(kyrie.str_name(), Some("kyrie_min.str"));
        assert!(!kyrie.repeating);

        // 22 is EFST_LEX_AETERNA (aesir Efst.id(:lexaeterna)) -- the pulsing
        // mark hovering above the marked unit while the status is active.
        let lex_aeterna = catalog.efst(22).expect("EFST_LEX_AETERNA descriptor");
        assert_eq!(lex_aeterna.str_name(), Some("lex_mark.strfx.ron"));
        assert!(lex_aeterna.repeating);
    }

    #[test]
    fn seeded_energy_coat_cast_splices_the_efst_recipe() {
        let catalog = seeded_catalog();

        // Skill 157 is the shipped composition example: `visuals: [Efst(31)]`
        // resolves to EFST 31's layers while keeping its own local fields.
        let cast = catalog.skill(157).expect("MG_ENERGYCOAT descriptor");
        assert_eq!(cast.visuals, vec![Visual::Str("energycoat.str".into())]);
        assert_eq!(cast.placement, EffectPlacement::Caster);
        assert!(!cast.repeating);
    }

    #[test]
    fn splicing_keeps_local_fields_and_expands_in_place() {
        let mut data = EffectData::default();
        data.efsts.insert(
            31,
            EffectDescriptor {
                visuals: vec![Visual::Str("energycoat.str".into())],
                sound: Some("aura.wav".into()),
                placement: EffectPlacement::Caster,
                repeating: true,
                ..Default::default()
            },
        );
        data.skills.insert(
            157,
            EffectDescriptor {
                visuals: vec![Visual::Sprite("이팩트/firewall".into()), Visual::Efst(31)],
                sound: None,
                placement: EffectPlacement::Target,
                repeating: false,
                ..Default::default()
            },
        );

        let catalog = EffectCatalog::build(&data).expect("catalog builds");
        let spliced = catalog.skill(157).expect("composed descriptor");

        assert_eq!(
            spliced.visuals,
            vec![
                Visual::Sprite("이팩트/firewall".into()),
                Visual::Str("energycoat.str".into()),
            ],
            "the Efst layer expands in place, after the local Sprite layer"
        );
        assert_eq!(spliced.sound, None, "local sound wins over the recipe's");
        assert_eq!(spliced.placement, EffectPlacement::Target);
        assert!(!spliced.repeating);

        // The recipe itself stays untouched and still serves StatusEffectChanged.
        assert!(catalog.efst(31).expect("recipe").repeating);
    }

    #[test]
    fn dangling_efst_reference_is_rejected() {
        let mut data = EffectData::default();
        data.skills.insert(157, descriptor(vec![Visual::Efst(31)]));

        let error = EffectCatalog::build(&data).expect_err("dangling Efst ref must fail");
        assert!(error.contains("skills[157]"), "{error}");
        assert!(error.contains("Efst(31)"), "{error}");
    }

    #[test]
    fn nested_efst_recipe_is_rejected() {
        let mut data = EffectData::default();
        data.efsts.insert(31, descriptor(vec![Visual::Efst(19)]));
        data.efsts
            .insert(19, descriptor(vec![Visual::Str("kyrie_min.str".into())]));

        let error = EffectCatalog::build(&data).expect_err("nested recipe must fail");
        assert!(error.contains("efsts[31]"), "{error}");
        assert!(error.contains("one level deep"), "{error}");
    }

    #[test]
    fn unknown_shader_key_is_rejected() {
        let mut data = EffectData::default();
        data.skills
            .insert(11, descriptor(vec![Visual::Shader("no_such_fx".into())]));

        let error = EffectCatalog::build(&data).expect_err("unknown shader key must fail");
        assert!(error.contains("skills[11]"), "{error}");
        assert!(error.contains("no_such_fx"), "{error}");
    }

    #[test]
    fn bespoke_key_is_never_checked_against_shader_fx() {
        let mut data = EffectData::default();
        data.skills
            .insert(87, descriptor(vec![Visual::Bespoke("ice_wall".into())]));

        let catalog = EffectCatalog::build(&data).expect("bespoke keys need no shader_fx entry");
        assert_eq!(
            catalog.skill(87).expect("descriptor").bespoke_key(),
            Some("ice_wall")
        );
    }

    #[test]
    fn duplicate_layer_kind_is_rejected() {
        let mut data = EffectData::default();
        data.skills.insert(
            5,
            descriptor(vec![
                Visual::Str("bash.strfx.ron".into()),
                Visual::Str("provoke.str".into()),
            ]),
        );

        let error = EffectCatalog::build(&data).expect_err("duplicate kind must fail");
        assert!(error.contains("skills[5]"), "{error}");
        assert!(error.contains("Str"), "{error}");
    }

    #[test]
    fn duplicate_layer_kind_introduced_by_splicing_is_rejected() {
        let mut data = EffectData::default();
        data.efsts
            .insert(31, descriptor(vec![Visual::Str("energycoat.str".into())]));
        data.skills.insert(
            157,
            descriptor(vec![Visual::Str("kyrie.str".into()), Visual::Efst(31)]),
        );

        let error = EffectCatalog::build(&data).expect_err("post-splice duplicate must fail");
        assert!(error.contains("skills[157]"), "{error}");
        assert!(error.contains("Str"), "{error}");
    }

    #[test]
    fn shipped_effects_ron_builds_the_catalog() {
        let catalog = seeded_catalog();

        // Every resolved descriptor is composition-free and shader-valid; the
        // build above would have failed otherwise. Spot-check one of each table.
        assert!(catalog.skill(5).is_some());
        assert!(catalog.special(89).is_some());
        assert!(catalog.efst(31).is_some());
    }

    #[test]
    fn crusader_skill_effects_ron_deserialize_into_catalog() {
        let catalog = seeded_catalog();

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
                .skill(skill_id)
                .unwrap_or_else(|| panic!("skill {skill_id} descriptor"));
            assert_eq!(descriptor.str_name(), Some(asset_name), "skill {skill_id}");
            assert_eq!(descriptor.placement, placement, "skill {skill_id}");
            assert_eq!(descriptor.sound.as_deref(), sound, "skill {skill_id}");
            assert!(!descriptor.repeating, "skill {skill_id}");
        }

        // CR_FAITH (248) is a passive with no cast event: deliberately absent.
        assert!(catalog.skill(248).is_none());
    }

    #[test]
    fn crusader_special_effects_ron_deserialize_into_catalog() {
        let catalog = seeded_catalog();

        // EF_REFLECTSHIELD (252): Reflect Shield's reactive proc flash on the
        // reflecting unit; reuses the authored skill-252 asset.
        let reflect = catalog.special(252).expect("EF_REFLECTSHIELD descriptor");
        assert_eq!(reflect.str_name(), Some("reflect_shield.strfx.ron"));
        assert_eq!(reflect.sound, None);
        assert_eq!(reflect.placement, EffectPlacement::Ground);
        assert!(!reflect.repeating);

        // EF_GUARD (336): Auto Guard's reactive proc flash on the guarding
        // unit, using the native kyrie STR.
        let guard = catalog.special(336).expect("EF_GUARD descriptor");
        assert_eq!(guard.str_name(), Some("kyrie.str"));
        assert_eq!(guard.sound, None);
        assert_eq!(guard.placement, EffectPlacement::Ground);
        assert!(!guard.repeating);
    }

    #[test]
    fn crusader_efst_effects_ron_deserialize_into_catalog() {
        let catalog = seeded_catalog();

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
                .efst(efst_id)
                .unwrap_or_else(|| panic!("EFST {efst_id} descriptor"));
            assert_eq!(descriptor.str_name(), Some(asset_name), "EFST {efst_id}");
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
        let auto_guard = catalog.efst(58).expect("EFST_AUTOGUARD").color;
        let reflect_shield = catalog.efst(59).expect("EFST_REFLECTSHIELD").color;
        let defender = catalog.efst(62).expect("EFST_DEFENDER").color;
        assert_ne!(auto_guard, reflect_shield);
        assert_ne!(auto_guard, defender);
        assert_ne!(reflect_shield, defender);
        assert_ne!(
            catalog.efst(60).expect("EFST_DEVOTION").color,
            catalog.efst(61).expect("EFST_PROVIDENCE").color
        );
    }
}
