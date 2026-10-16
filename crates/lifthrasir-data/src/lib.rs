pub mod catalogs;
pub mod effects;
pub mod gr2;
pub mod lif;

pub use catalogs::*;
pub use effects::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_data_round_trip() {
        let mut original = JobData::default();
        original.npc_sprites.insert(0, "NOVICE".to_string());
        original.npc_sprites.insert(1, "SWORDMAN".to_string());
        original.display_names.insert(0, "Novice".to_string());
        original.display_names.insert(1, "Swordman".to_string());

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: JobData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn item_data_round_trip() {
        let mut original = ItemData::default();
        original.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                identified_description: vec![
                    "A potion that restores 45 HP.".to_string(),
                    "Brewed from red herbs.".to_string(),
                ],
                unidentified_name: "Unknown Potion".to_string(),
                unidentified_resource: "UNKNOWN_POTION".to_string(),
                unidentified_description: vec!["An unidentified potion.".to_string()],
                slot_count: 0,
            },
        );
        original.items.insert(
            2104,
            ItemInfo {
                identified_name: "Buckler".to_string(),
                identified_resource: "BUCKLER".to_string(),
                identified_description: vec![
                    "A small round shield.".to_string(),
                    "DEF +5.".to_string(),
                ],
                unidentified_name: "Round Shield".to_string(),
                unidentified_resource: "ROUND_SHIELD".to_string(),
                unidentified_description: vec!["An unidentified shield.".to_string()],
                slot_count: 1,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: ItemData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.items, deserialized.items);
    }

    #[test]
    fn accessory_data_round_trip() {
        let mut original = AccessoryData::default();
        original.names.insert(1, "_고글".to_string());
        original.names.insert(2, "_고양이머리띠".to_string());

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: AccessoryData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn weapon_data_round_trip() {
        let mut original = WeaponData::default();
        original.names.insert(2, "_검".to_string());
        original.names.insert(3, "_양손검".to_string());
        original.hit_sounds.insert(2, "_hit_sword.wav".to_string());
        original.bow_types.insert(11);

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: WeaponData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn skill_data_round_trip() {
        let mut original = SkillData::default();
        original.skills.insert(
            5,
            SkillMeta {
                name: "SM_BASH".to_string(),
                display_name: "Bash".to_string(),
                description: vec![
                    "Strike an enemy with your weapon.".to_string(),
                    "ATK +300% at level 10.".to_string(),
                ],
                max_level: 10,
                sp_cost: vec![8, 8, 8, 8, 8, 15, 15, 15, 15, 15],
                attack_range: vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            },
        );
        original.skills.insert(
            8,
            SkillMeta {
                name: "SM_ENDURE".to_string(),
                display_name: "Endure".to_string(),
                description: vec!["Ignore MDEF interruptions.".to_string()],
                max_level: 10,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: SkillData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.skills, deserialized.skills);
    }

    #[test]
    fn effect_data_round_trip() {
        let mut original = EffectData::default();
        original.skills.insert(
            28,
            EffectDescriptor {
                visuals: vec![Visual::Str("heal.str".to_string())],
                sound: Some("_heal_effect.wav".to_string()),
                on_trigger: None,
                placement: EffectPlacement::Target,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: false,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.special.insert(
            89,
            EffectDescriptor {
                visuals: vec![Visual::Str("stormgust.str".to_string())],
                sound: None,
                on_trigger: None,
                placement: EffectPlacement::Ground,
                color: [0.6, 0.7, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );
        original.efsts.insert(
            157,
            EffectDescriptor {
                visuals: vec![Visual::Str("energycoat.str".to_string())],
                sound: None,
                on_trigger: None,
                placement: EffectPlacement::Caster,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Group,
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectData = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original.skills, deserialized.skills);
        assert_eq!(original.special, deserialized.special);
        assert_eq!(original.efsts, deserialized.efsts);

        let energy_coat = deserialized.efsts.get(&157).expect("EFST 157 entry");
        assert_eq!(energy_coat.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn every_visual_kind_round_trips() {
        let original = EffectDescriptor {
            visuals: vec![
                Visual::Sprite("이팩트/firewall".to_string()),
                Visual::Str("firewall.str".to_string()),
                Visual::Shader("fire_bolt".to_string()),
                Visual::Bespoke("ice_wall".to_string()),
                Visual::Efst(31),
            ],
            ..Default::default()
        };

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectDescriptor = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn model_visual_and_trigger_fx_round_trip() {
        let original = EffectDescriptor {
            visuals: vec![Visual::Model("외부소품/트랩02".to_string())],
            on_trigger: Some(TriggerFx {
                visual: Visual::Str("skidtrap.str".to_string()),
                sound: Some("effect/hunter_skidtrap.wav".to_string()),
            }),
            ..Default::default()
        };

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: EffectDescriptor = ron::from_str(&serialized).expect("deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn descriptor_without_on_trigger_deserializes() {
        let ron = r#"(
            visuals: [Str("firewall.str")],
            sound: None,
            placement: Ground,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: true,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert_eq!(descriptor.on_trigger, None);
    }

    #[test]
    fn model_path_only_reads_model_layers() {
        let model_only = EffectDescriptor {
            visuals: vec![Visual::Model("외부소품/트랩02".to_string())],
            ..Default::default()
        };
        let sprite_only = EffectDescriptor {
            visuals: vec![Visual::Sprite("이팩트/firewall".to_string())],
            ..Default::default()
        };

        assert_eq!(model_only.model_path(), Some("외부소품/트랩02"));
        assert_eq!(model_only.sprite_stem(), None);
        assert_eq!(sprite_only.model_path(), None);
    }

    #[test]
    fn accessors_return_the_first_layer_of_their_kind() {
        let descriptor = EffectDescriptor {
            visuals: vec![
                Visual::Sprite("이팩트/firewall".to_string()),
                Visual::Bespoke("ice_wall".to_string()),
            ],
            ..Default::default()
        };

        assert_eq!(descriptor.sprite_stem(), Some("이팩트/firewall"));
        assert_eq!(descriptor.bespoke_key(), Some("ice_wall"));
        assert_eq!(descriptor.str_name(), None);
        assert_eq!(descriptor.shader_key(), None);
    }

    #[test]
    fn ground_anchor_defaults_to_group_when_absent() {
        let ron = r#"(
            visuals: [Str("firewall.str")],
            sound: None,
            placement: Ground,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: true,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert_eq!(descriptor.ground_anchor, GroundAnchor::Group);
    }

    #[test]
    fn effect_data_without_efsts_section_deserializes() {
        let ron = r#"(
            skills: {},
            special: {},
        )"#;

        let data: EffectData = ron::from_str(ron).expect("deserialize");

        assert!(data.efsts.is_empty());
    }

    #[test]
    fn descriptor_without_visuals_deserializes_as_sound_only() {
        let ron = r#"(
            sound: Some("effect/ef_bash.wav"),
            placement: Target,
            color: (1.0, 1.0, 1.0, 1.0),
            repeating: false,
        )"#;

        let descriptor: EffectDescriptor = ron::from_str(ron).expect("deserialize");

        assert!(descriptor.visuals.is_empty());
        assert_eq!(descriptor.sound.as_deref(), Some("effect/ef_bash.wav"));
    }

    #[test]
    fn status_icon_data_round_trip() {
        let mut original = StatusIconData::default();
        original.icons.insert(
            10,
            StatusIconEntry {
                image: "BLESSING.TGA".to_string(),
                name: "Blessing".to_string(),
            },
        );

        let serialized = ron::to_string(&original).expect("serialize");
        let deserialized: StatusIconData = ron::from_str(&serialized).expect("deserialize");

        let entry = deserialized.icons.get(&10).expect("efst 10 entry");
        assert_eq!(entry.image, "BLESSING.TGA");
        assert_eq!(entry.name, "Blessing");
    }

    #[test]
    fn effects_ron_seed_deserializes() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/ron/effects.ron"
        );
        let contents = std::fs::read_to_string(path).expect("read effects.ron");
        let data: EffectData = ron::from_str(&contents).expect("deserialize seed");

        let heal = data.skills.get(&28).expect("AL_HEAL entry");
        assert_eq!(heal.placement, EffectPlacement::Target);
        assert_eq!(heal.str_name(), Some("heal.strfx.ron"));
        // Sound is relative to `data/wav/`; `_heal_effect.wav` lives at the root
        // (no `effect/` prefix), so the old `effect/_heal_effect.wav` was broken.
        assert_eq!(heal.sound.as_deref(), Some("_heal_effect.wav"));
        assert!(!heal.repeating);

        let stormgust = data.skills.get(&89).expect("WZ_STORMGUST entry");
        assert_eq!(stormgust.placement, EffectPlacement::Ground);
        assert_eq!(stormgust.str_name(), Some("stormgust.str"));
        // `effect/stormgust.wav` does not exist in the GRF; the real sound is
        // `effect/wizard_stormgust.wav`.
        assert_eq!(
            stormgust.sound.as_deref(),
            Some("effect/wizard_stormgust.wav")
        );
        assert!(stormgust.repeating);

        // SM_BASH uses an authored slash because the official effect is hardcoded.
        let bash = data.skills.get(&5).expect("SM_BASH entry");
        assert_eq!(bash.str_name(), Some("bash.strfx.ron"));
        assert_eq!(bash.sound.as_deref(), Some("effect/ef_bash.wav"));

        let special_stormgust = data.special.get(&89).expect("EF_STORMGUST entry");
        assert_eq!(special_stormgust.str_name(), Some("stormgust.str"));
        assert!(special_stormgust.repeating);

        let magnus = data.special.get(&113).expect("EF_MAGNUS entry");
        assert_eq!(magnus.str_name(), Some("magnus.str"));

        assert_eq!(heal.ground_anchor, GroundAnchor::Group);
        assert_eq!(stormgust.ground_anchor, GroundAnchor::Group);

        // 31 EFST_ENERGYCOAT: the persistent Energy Coat aura, migrated out of
        // `skills:` (157 there stays as the one-shot cast flash).
        let energy_coat_aura = data.efsts.get(&31).expect("EFST_ENERGYCOAT entry");
        assert_eq!(energy_coat_aura.str_name(), Some("energycoat.str"));
        assert!(!energy_coat_aura.repeating);

        // The shipped composition example: the cast flash composes the EFST 31
        // recipe unresolved at this layer (the catalog splices it at load).
        let energy_coat_cast = data.skills.get(&157).expect("MG_ENERGYCOAT entry");
        assert_eq!(energy_coat_cast.visuals, vec![Visual::Efst(31)]);
        assert!(!energy_coat_cast.repeating);
    }
}
