use crate::domain::entities::character::components::Gender;
use regex::Regex;
use std::sync::LazyLock;

// Hair sprite pattern: data[\\/]sprite[\\/]인간족[\\/]머리통[\\/]{sex}[\\/]{id}_{sex}.spr
pub static HAIR_SPRITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"data[\\/]sprite[\\/]인간족[\\/]머리통[\\/](남|여)[\\/](\d+)_(남|여)\.spr")
        .expect("Invalid hair sprite regex")
});

// Hair action pattern: data[\\/]sprite[\\/]인간족[\\/]머리통[\\/]{sex}[\\/]{id}_{sex}.act
pub static HAIR_ACTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"data[\\/]sprite[\\/]인간족[\\/]머리통[\\/](남|여)[\\/](\d+)_(남|여)\.act")
        .expect("Invalid hair action regex")
});

// Hair palette pattern: data[\\/]palette[\\/]머리[\\/]{id}_{sex}_{color}.pal
pub static HAIR_PALETTE: LazyLock<Regex> = LazyLock::new(|| {
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

/// Generate the minimap BMP path for a map name.
pub fn minimap_path(map_name: &str) -> String {
    format!("ro://data/texture/유저인터페이스/map/{map_name}.bmp")
}

/// Generate the ground-drop collection sprite path for an item resource name.
pub fn item_drop_sprite_path(resource_name: &str) -> String {
    format!("ro://data/sprite/아이템/{resource_name}.spr")
}

/// Generate the ground-drop collection action path for an item resource name.
pub fn item_drop_action_path(resource_name: &str) -> String {
    format!("ro://data/sprite/아이템/{resource_name}.act")
}

/// Generate the pushcart sprite path (single gender-agnostic sprite).
/// 손수레 ("handcart") is the tier-1 cart; 손수레1-4 are the higher tiers,
/// unused since we render one sprite regardless of tier.
pub fn cart_sprite_path() -> String {
    "ro://data/sprite/이팩트/손수레.spr".to_string()
}

/// Generate the pushcart action path.
pub fn cart_action_path() -> String {
    cart_sprite_path().replace(".spr", ".act")
}

/// Generate the shared emote sprite path. `emotion.spr` carries every emote's
/// animation and an embedded palette (no external `.pal`).
pub fn emotion_sprite_path() -> String {
    "ro://data/sprite/이팩트/emotion.spr".to_string()
}

/// Generate the shared emote action path.
pub fn emotion_action_path() -> String {
    emotion_sprite_path().replace(".spr", ".act")
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

/// Generate weapon sprite path.
/// `suffix` comes from the weapon db and already carries its leading separator (e.g. `"_검"`).
pub fn weapon_sprite_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/인간족/{}/{}_{}{}.spr",
        job_name, job_name, sex, suffix
    )
}

/// Generate weapon action path.
pub fn weapon_action_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    weapon_sprite_path(gender, job_name, suffix).replace(".spr", ".act")
}

/// Generate shield sprite path.
/// `suffix` is a classic shield name (e.g. `"가드"`) or a raw renewal view id (e.g. `"28901"`),
/// without a leading separator.
pub fn shield_sprite_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    let sex = match gender {
        Gender::Male => "남",
        Gender::Female => "여",
    };
    format!(
        "ro://data/sprite/방패/{}/{}_{}_{}_방패.spr",
        job_name, job_name, sex, suffix
    )
}

/// Generate shield action path.
pub fn shield_action_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    shield_sprite_path(gender, job_name, suffix).replace(".spr", ".act")
}

/// Classic shield view id -> sprite suffix, with a numeric fallback for renewal shields.
pub fn shield_suffix(view_id: u16) -> String {
    match view_id {
        1 => "가드".to_string(),
        2 => "쉴드".to_string(),
        3 => "버클러".to_string(),
        4 => "미러쉴드".to_string(),
        other => other.to_string(),
    }
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
    fn minimap_path_builds_correct_url() {
        assert_eq!(
            minimap_path("prontera"),
            "ro://data/texture/유저인터페이스/map/prontera.bmp"
        );
    }

    #[test]
    fn cart_sprite_path_builds_correct_url() {
        assert_eq!(cart_sprite_path(), "ro://data/sprite/이팩트/손수레.spr");
    }

    #[test]
    fn cart_action_path_builds_correct_url() {
        assert_eq!(cart_action_path(), "ro://data/sprite/이팩트/손수레.act");
    }

    #[test]
    fn emotion_sprite_path_builds_correct_url() {
        assert_eq!(emotion_sprite_path(), "ro://data/sprite/이팩트/emotion.spr");
    }

    #[test]
    fn emotion_action_path_builds_correct_url() {
        assert_eq!(emotion_action_path(), "ro://data/sprite/이팩트/emotion.act");
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

    #[test]
    fn item_drop_sprite_path_builds_correct_url() {
        assert_eq!(
            item_drop_sprite_path("RED_POTION"),
            "ro://data/sprite/아이템/RED_POTION.spr"
        );
    }

    #[test]
    fn item_drop_action_path_builds_correct_url() {
        assert_eq!(
            item_drop_action_path("RED_POTION"),
            "ro://data/sprite/아이템/RED_POTION.act"
        );
    }

    #[test]
    fn weapon_sprite_path_builds_correct_url() {
        assert_eq!(
            weapon_sprite_path(Gender::Male, "검사", "_검"),
            "ro://data/sprite/인간족/검사/검사_남_검.spr"
        );
        assert_eq!(
            weapon_sprite_path(Gender::Male, "검사", "_1116"),
            "ro://data/sprite/인간족/검사/검사_남_1116.spr"
        );
        assert_eq!(
            weapon_sprite_path(Gender::Female, "검사", "_검"),
            "ro://data/sprite/인간족/검사/검사_여_검.spr"
        );
    }

    #[test]
    fn weapon_action_path_builds_correct_url() {
        assert_eq!(
            weapon_action_path(Gender::Male, "검사", "_검"),
            "ro://data/sprite/인간족/검사/검사_남_검.act"
        );
        assert_eq!(
            weapon_action_path(Gender::Male, "검사", "_1116"),
            "ro://data/sprite/인간족/검사/검사_남_1116.act"
        );
    }

    #[test]
    fn shield_suffix_maps_classic_ids() {
        assert_eq!(shield_suffix(1), "가드");
        assert_eq!(shield_suffix(2), "쉴드");
        assert_eq!(shield_suffix(3), "버클러");
        assert_eq!(shield_suffix(4), "미러쉴드");
        assert_eq!(shield_suffix(28901), "28901");
    }

    #[test]
    fn shield_sprite_path_builds_correct_url() {
        assert_eq!(
            shield_sprite_path(Gender::Male, "검사", &shield_suffix(1)),
            "ro://data/sprite/방패/검사/검사_남_가드_방패.spr"
        );
        assert_eq!(
            shield_sprite_path(Gender::Male, "검사", "28901"),
            "ro://data/sprite/방패/검사/검사_남_28901_방패.spr"
        );
        assert_eq!(
            shield_sprite_path(Gender::Female, "검사", &shield_suffix(1)),
            "ro://data/sprite/방패/검사/검사_여_가드_방패.spr"
        );
    }

    #[test]
    fn shield_action_path_builds_correct_url() {
        assert_eq!(
            shield_action_path(Gender::Male, "검사", &shield_suffix(1)),
            "ro://data/sprite/방패/검사/검사_남_가드_방패.act"
        );
    }
}
