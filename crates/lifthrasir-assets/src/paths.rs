use crate::Gender;

/// Generate hair sprite path
pub fn hair_sprite_path(gender: Gender, style_id: u16) -> String {
    let sex = gender.as_korean();
    format!(
        "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
        sex, style_id, sex
    )
}

/// Generate hair palette path
pub fn hair_palette_path(style_id: u16, gender: Gender, color_id: u16) -> String {
    let sex = gender.as_korean();
    format!(
        "ro://data/palette/머리/머리{}_{}_{}.pal",
        style_id, sex, color_id
    )
}

/// Generate body sprite path
pub fn body_sprite_path(gender: Gender, job_name: &str) -> String {
    let sex = gender.as_korean();
    format!(
        "ro://data/sprite/인간족/몸통/{}/{}_{}.spr",
        sex, job_name, sex
    )
}

/// Generate head sprite path
pub fn head_sprite_path(gender: Gender, style_id: u16) -> String {
    let sex = gender.as_korean();
    format!(
        "ro://data/sprite/인간족/머리통/{}/{}_{}.spr",
        sex, style_id, sex
    )
}

/// Generate head action path
pub fn head_action_path(gender: Gender, style_id: u16) -> String {
    let sex = gender.as_korean();
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

/// Generate the item-details illustration BMP path for an item resource name.
pub fn item_collection_path(resource_name: &str) -> String {
    format!("ro://data/texture/유저인터페이스/collection/{resource_name}.bmp")
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

/// Generate the falcon sprite path.
pub fn falcon_sprite_path() -> String {
    mob_sprite_path("매")
}

/// Generate the falcon action path.
pub fn falcon_action_path() -> String {
    mob_action_path("매")
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

/// Generate the shared frozen-status ice-block overlay sprite path
/// (`얼음땡`, "ice stuck"), drawn over a unit while its `body_state` reports
/// Freeze. See `domain::effects::status_visuals`.
pub fn frozen_ice_sprite_path() -> String {
    "ro://data/sprite/이팩트/얼음땡.spr".to_string()
}

/// Generate the shared frozen-status ice-block overlay action path.
pub fn frozen_ice_action_path() -> String {
    frozen_ice_sprite_path().replace(".spr", ".act")
}

/// Generate headgear (accessory) sprite path.
/// `accname` comes from the accessory db and already carries its leading separator (e.g. `"_고글"`).
pub fn headgear_sprite_path(gender: Gender, accname: &str) -> String {
    let sex = gender.as_korean();
    format!("ro://data/sprite/악세사리/{}/{}{}.spr", sex, sex, accname)
}

/// Generate headgear (accessory) action path.
pub fn headgear_action_path(gender: Gender, accname: &str) -> String {
    headgear_sprite_path(gender, accname).replace(".spr", ".act")
}

/// Generate weapon sprite path.
/// `suffix` comes from the weapon db and already carries its leading separator (e.g. `"_검"`).
pub fn weapon_sprite_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    let sex = gender.as_korean();
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
    let sex = gender.as_korean();
    format!(
        "ro://data/sprite/방패/{}/{}_{}_{}_방패.spr",
        job_name, job_name, sex, suffix
    )
}

/// Generate shield action path.
pub fn shield_action_path(gender: Gender, job_name: &str, suffix: &str) -> String {
    shield_sprite_path(gender, job_name, suffix).replace(".spr", ".act")
}
