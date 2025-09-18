use crate::domain::character::{CharacterData, Gender};

#[derive(Debug, Clone)]
pub struct CharacterSpritePaths {
    pub body_sprite: String,
    pub body_act: String,
    pub head_sprite: String,
    pub head_act: String,
    pub head_palette: Option<String>,
}

/// Generates sprite file paths for a character following RO's Korean GRF structure
pub fn generate_character_sprite_paths(character: &CharacterData) -> CharacterSpritePaths {
    let job_name = character.class.to_sprite_name();
    let sex_suffix = match character.sex {
        Gender::Male => "남",
        Gender::Female => "여",
    };

    // Body sprite path (using backslashes for GRF compatibility)
    // Use proper job class sprite names for all classes, including novice (초보자)
    let body_sprite = format!(
        "data\\sprite\\인간족\\몸통\\{}\\{}_{}.spr",
        sex_suffix, job_name, sex_suffix
    );

    let body_act = format!(
        "data\\sprite\\인간족\\몸통\\{}\\{}_{}.act",
        sex_suffix, job_name, sex_suffix
    );

    // Head sprite path (using backslashes for GRF compatibility)
    let head_sprite = format!(
        "data\\sprite\\인간족\\머리통\\{}\\{}_{}.spr",
        sex_suffix, character.hair_style, sex_suffix
    );
    let head_act = format!(
        "data\\sprite\\인간족\\머리통\\{}\\{}_{}.act",
        sex_suffix, character.hair_style, sex_suffix
    );

    // Head palette path (if custom hair color)
    let head_palette = if character.hair_color > 0 {
        Some(format!(
            "data\\palette\\머리\\{}_{}_{}.pal",
            character.hair_style, sex_suffix, character.hair_color
        ))
    } else {
        None
    };

    CharacterSpritePaths {
        body_sprite,
        body_act,
        head_sprite,
        head_act,
        head_palette,
    }
}

/// Generates palette file path for hair color
pub fn generate_character_palette_path(character: &CharacterData) -> Option<String> {
    if character.hair_color > 0 {
        let sex_suffix = match character.sex {
            Gender::Male => "남",
            Gender::Female => "여",
        };

        Some(format!(
            "data\\palette\\머리\\{}_{}_{}.pal",
            character.hair_style, sex_suffix, character.hair_color
        ))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EquipmentSlot {
    HeadTop,
    HeadMid,
    HeadBottom,
    Weapon,
    Shield,
    Robe,
}

/// Generates equipment sprite path (framework for future equipment support)
pub fn generate_equipment_sprite_path(item_id: u16, gender: Gender, slot: EquipmentSlot) -> String {
    let sex_prefix = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };

    match slot {
        EquipmentSlot::HeadTop | EquipmentSlot::HeadMid | EquipmentSlot::HeadBottom => {
            // TODO: Map item_id to actual item name
            format!(
                "data/sprite/악세사리/{}/{}_{}.spr",
                sex_prefix, sex_prefix, "item_name"
            )
        }
        EquipmentSlot::Weapon | EquipmentSlot::Shield => {
            // TODO: Handle weapon/shield sprites per job
            format!(
                "data/sprite/방패/{}/{}_{}.spr",
                sex_prefix, sex_prefix, "shield_name"
            )
        }
        _ => String::new(),
    }
}
