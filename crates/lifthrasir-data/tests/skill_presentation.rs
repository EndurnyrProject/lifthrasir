use lifthrasir_data::{EffectData, SkillMotion, SkillPresentation, resolve_effect_catalog};

#[test]
fn presentation_round_trips_independently_of_artwork() {
    let mut data = EffectData::default();
    let policy = SkillPresentation {
        success_only: true,
        motion: SkillMotion::None,
    };
    data.skill_presentation.insert(777, policy);
    let serialized = ron::to_string(&data).unwrap();
    let decoded: EffectData = ron::from_str(&serialized).unwrap();
    let catalog = resolve_effect_catalog(&decoded).unwrap();
    assert_eq!(catalog.skill(777), None);
    assert_eq!(catalog.skill_presentation(777), policy);
    assert_eq!(
        catalog.skill_presentation(999),
        SkillPresentation::default()
    );
}

#[test]
fn older_catalogs_default_to_ordinary_skill_feedback() {
    let data: EffectData = ron::from_str("(skills: {}, special: {})").unwrap();
    let catalog = resolve_effect_catalog(&data).unwrap();
    let policy = catalog.skill_presentation(150);
    assert!(!policy.success_only);
    assert_eq!(policy.motion, SkillMotion::Cast);
}
