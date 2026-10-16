use lifthrasir_data::effects::{
    EffectData, EffectDescriptor, EffectPlacement, TriggerFx, Visual, resolve_effect_catalog,
};

#[test]
fn skill_units_prefer_explicit_entries_and_only_fall_back_to_repeating_casts() {
    let mut data = EffectData::default();
    data.skills.insert(
        1,
        EffectDescriptor {
            visuals: vec![Visual::Str("cast.str".into())],
            repeating: true,
            ..Default::default()
        },
    );
    data.skills.insert(2, EffectDescriptor::default());
    let resolved = resolve_effect_catalog(&data).unwrap();
    assert_eq!(resolved.skill_unit(1), data.skills.get(&1));
    assert_eq!(resolved.skill_unit(2), None);
    data.skill_units.insert(
        1,
        EffectDescriptor {
            visuals: vec![Visual::Str("field.str".into())],
            repeating: true,
            ..Default::default()
        },
    );
    let resolved = resolve_effect_catalog(&data).unwrap();
    assert_eq!(resolved.skill_unit(1), data.skill_units.get(&1));
}

#[test]
fn invalid_catalogs_keep_section_and_id_context() {
    let mut data = EffectData::default();
    data.skills.insert(
        7,
        EffectDescriptor {
            visuals: vec![Visual::Efst(99)],
            ..Default::default()
        },
    );
    let error = resolve_effect_catalog(&data).unwrap_err().to_string();
    assert!(
        error.contains("skills[7]") && error.contains("Efst(99)"),
        "{error}"
    );

    data.efsts.insert(
        99,
        EffectDescriptor {
            visuals: vec![Visual::Efst(99)],
            ..Default::default()
        },
    );
    let error = resolve_effect_catalog(&data).unwrap_err().to_string();
    assert!(
        error.contains("efsts[99]") && error.contains("one level deep"),
        "{error}"
    );

    for (visuals, repeating, placement, trigger, diagnostic) in [
        (
            vec![Visual::Shader("unknown".into())],
            false,
            EffectPlacement::Target,
            None,
            "shader_fx",
        ),
        (
            vec![Visual::Str("a".into()), Visual::Str("b".into())],
            false,
            EffectPlacement::Target,
            None,
            "more than one Str",
        ),
        (
            vec![Visual::Model("trap".into())],
            false,
            EffectPlacement::Ground,
            None,
            "requires repeating",
        ),
        (
            vec![Visual::Model("trap".into())],
            true,
            EffectPlacement::Target,
            None,
            "placement: Ground",
        ),
        (
            vec![],
            false,
            EffectPlacement::Target,
            Some(TriggerFx {
                visual: Visual::Sprite("spark".into()),
                sound: None,
            }),
            "only Str",
        ),
    ] {
        let mut data = EffectData::default();
        data.special.insert(
            8,
            EffectDescriptor {
                visuals,
                repeating,
                placement,
                on_trigger: trigger,
                ..Default::default()
            },
        );
        let error = resolve_effect_catalog(&data).unwrap_err().to_string();
        assert!(
            error.contains("special[8]") && error.contains(diagnostic),
            "{error}"
        );
    }
    let mut data = EffectData::default();
    data.skill_units.insert(9, EffectDescriptor::default());
    let error = resolve_effect_catalog(&data).unwrap_err().to_string();
    assert!(
        error.contains("skill_units[9]") && error.contains("repeating: true"),
        "{error}"
    );
}

#[test]
fn shipped_recipes_resolve_without_bevy() {
    let data: EffectData =
        ron::from_str(include_str!("../../assets/data/ron/effects.ron")).unwrap();
    let resolved = resolve_effect_catalog(&data).unwrap();
    assert_eq!(
        resolved.skill(157).unwrap().visuals,
        [Visual::Str("energycoat.str".into())]
    );
    assert!(resolved.skill_unit(319).unwrap().repeating);
}

#[test]
fn resolves_composition_in_order_without_merging_id_namespaces() {
    let mut data = EffectData::default();
    let recipe = EffectDescriptor {
        visuals: vec![Visual::Str("aura.str".into())],
        repeating: true,
        sound: Some("recipe.wav".into()),
        ..Default::default()
    };
    data.efsts.insert(31, recipe.clone());
    let cast = EffectDescriptor {
        visuals: vec![Visual::Sprite("spark".into()), Visual::Efst(31)],
        color: [0.5; 4],
        ..Default::default()
    };
    data.skills.insert(31, cast.clone());
    data.special.insert(
        31,
        EffectDescriptor {
            visuals: vec![Visual::Bespoke("smoke".into())],
            ..Default::default()
        },
    );
    let resolved = resolve_effect_catalog(&data).unwrap();
    let expected = EffectDescriptor {
        visuals: vec![
            Visual::Sprite("spark".into()),
            Visual::Str("aura.str".into()),
        ],
        ..cast
    };
    assert_eq!(resolved.skill(31), Some(&expected));
    assert_eq!(resolved.efst(31), Some(&recipe));
    assert_eq!(resolved.special(31), data.special.get(&31));
    assert_eq!(resolved.skill_unit(31), None);
    assert_eq!(resolved.skill(999), None);
}
