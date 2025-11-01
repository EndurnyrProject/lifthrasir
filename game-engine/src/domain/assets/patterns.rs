use crate::domain::entities::character::components::Gender;
use once_cell::sync::Lazy;
use regex::Regex;

// Hair sprite pattern: data[\\/]sprite[\\/]인간족[\\/]머리통[\\/]{sex}[\\/]{id}_{sex}.spr
pub static HAIR_SPRITE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data[\\/]sprite[\\/]인간족[\\/]머리통[\\/](남|여)[\\/](\d+)_(남|여)\.spr")
        .expect("Invalid hair sprite regex")
});

// Hair action pattern: data[\\/]sprite[\\/]인간족[\\/]머리통[\\/]{sex}[\\/]{id}_{sex}.act
pub static HAIR_ACTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data[\\/]sprite[\\/]인간족[\\/]머리통[\\/](남|여)[\\/](\d+)_(남|여)\.act")
        .expect("Invalid hair action regex")
});

// Hair palette pattern: data[\\/]palette[\\/]머리[\\/]{id}_{sex}_{color}.pal
pub static HAIR_PALETTE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data[\\/]palette[\\/]머리[\\/](\d+)_(남|여)_(\d+)\.pal")
        .expect("Invalid hair palette regex")
});

// Future patterns for equipment, monsters, effects can be added here

/// Normalize path separators for cross-platform compatibility
/// Converts backslashes to forward slashes
/// CRITICAL: Must be called before regex matching
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Helper to convert Korean gender strings to enum
pub fn parse_gender(s: &str) -> Option<Gender> {
    match s {
        "남" => Some(Gender::Male),
        "여" => Some(Gender::Female),
        _ => None,
    }
}

/// Generate hair sprite path (for consistency with parsing)
pub fn hair_sprite_path(gender: Gender, style_id: u16) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "data\\sprite\\인간족\\머리통\\{}\\{}_{}.spr",
        sex, style_id, sex
    )
}

/// Generate hair action path
pub fn hair_action_path(gender: Gender, style_id: u16) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "data\\sprite\\인간족\\머리통\\{}\\{}_{}.act",
        sex, style_id, sex
    )
}

/// Generate hair palette path
pub fn hair_palette_path(style_id: u16, gender: Gender, color_id: u16) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/palette/머리/{}_{}_{}.pal",
        style_id, sex, color_id
    )
}

/// Generate body sprite path
pub fn body_sprite_path(gender: Gender, job_name: &str) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/인간족/몸통/{}/{}_{}.spr",
        sex, job_name, sex
    )
}

/// Generate body action path
pub fn body_action_path(gender: Gender, job_name: &str) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/인간족/몸통/{}/{}_{}.act",
        sex, job_name, sex
    )
}

/// Generate head sprite path
pub fn head_sprite_path(gender: Gender, style_id: u16) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
        sex, style_id, sex
    )
}

/// Generate head action path
pub fn head_action_path(gender: Gender, style_id: u16) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/인간족/머리통/{}/{}_{}.act",
        sex, style_id, sex
    )
}

/// Generate mob sprite path
pub fn mob_sprite_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/몬스터/{}.spr", sprite_name)
}

/// Generate mob action path
pub fn mob_action_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/몬스터/{}.act", sprite_name)
}

/// Generate NPC sprite path
pub fn npc_sprite_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/npc/{}.spr", sprite_name)
}

/// Generate NPC action path
pub fn npc_action_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/npc/{}.act", sprite_name)
}
