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
        "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
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
    format!("ro://data/sprite/몬스터/{}.spr", sprite_name.to_lowercase())
}

/// Generate mob action path
pub fn mob_action_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/몬스터/{}.act", sprite_name.to_lowercase())
}

/// Generate NPC sprite path
pub fn npc_sprite_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/npc/{}.spr", sprite_name.to_lowercase())
}

/// Generate NPC action path
pub fn npc_action_path(sprite_name: &str) -> String {
    format!("ro://data/sprite/npc/{}.act", sprite_name.to_lowercase())
}

/// Generate the inventory icon BMP path for an item resource name.
pub fn item_icon_path(resource_name: &str) -> String {
    format!("ro://data/texture/유저인터페이스/item/{resource_name}.bmp")
}

/// Generate headgear (accessory) sprite path.
/// `accname` comes from the accessory db and already carries its leading separator (e.g. `"_고글"`).
pub fn headgear_sprite_path(gender: Gender, accname: &str) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!("ro://data/sprite/악세사리/{}/{}{}.spr", sex, sex, accname)
}

/// Generate headgear (accessory) action path.
pub fn headgear_action_path(gender: Gender, accname: &str) -> String {
    headgear_sprite_path(gender, accname).replace(".spr", ".act")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_icon_path_builds_correct_url() {
        assert_eq!(
            item_icon_path("apple"),
            "ro://data/texture/유저인터페이스/item/apple.bmp"
        );
    }

    #[test]
    fn headgear_sprite_path_builds_correct_url() {
        assert_eq!(
            headgear_sprite_path(Gender::Male, "_고글"),
            "ro://data/sprite/악세사리/남/남_고글.spr"
        );
        assert_eq!(
            headgear_sprite_path(Gender::Female, "_고글"),
            "ro://data/sprite/악세사리/여/여_고글.spr"
        );
    }

    #[test]
    fn headgear_action_path_builds_correct_url() {
        assert_eq!(
            headgear_action_path(Gender::Male, "_고글"),
            "ro://data/sprite/악세사리/남/남_고글.act"
        );
        assert_eq!(
            headgear_action_path(Gender::Female, "_고글"),
            "ro://data/sprite/악세사리/여/여_고글.act"
        );
    }
}
