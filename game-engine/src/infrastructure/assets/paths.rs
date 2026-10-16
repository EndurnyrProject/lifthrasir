pub use lifthrasir_assets::paths::*;

#[cfg(test)]
use crate::domain::entities::character::components::Gender;

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
    fn falcon_sprite_path_builds_correct_url() {
        assert_eq!(falcon_sprite_path(), "ro://data/sprite/몬스터/매.spr");
    }

    #[test]
    fn falcon_action_path_builds_correct_url() {
        assert_eq!(falcon_action_path(), "ro://data/sprite/몬스터/매.act");
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
    fn frozen_ice_sprite_path_builds_correct_url() {
        assert_eq!(
            frozen_ice_sprite_path(),
            "ro://data/sprite/이팩트/얼음땡.spr"
        );
    }

    #[test]
    fn frozen_ice_action_path_builds_correct_url() {
        assert_eq!(
            frozen_ice_action_path(),
            "ro://data/sprite/이팩트/얼음땡.act"
        );
    }

    #[test]
    fn hair_palette_path_matches_retail_filename() {
        let path = hair_palette_path(1, Gender::Male, 4);

        assert_eq!(path, "ro://data/palette/머리/머리1_남_4.pal");
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
    fn item_collection_path_builds_correct_url() {
        assert_eq!(
            item_collection_path("RED_POTION"),
            "ro://data/texture/유저인터페이스/collection/RED_POTION.bmp"
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
    fn shield_sprite_path_builds_correct_url() {
        assert_eq!(
            shield_sprite_path(Gender::Male, "검사", "가드"),
            "ro://data/sprite/방패/검사/검사_남_가드_방패.spr"
        );
        assert_eq!(
            shield_sprite_path(Gender::Male, "검사", "28901"),
            "ro://data/sprite/방패/검사/검사_남_28901_방패.spr"
        );
        assert_eq!(
            shield_sprite_path(Gender::Female, "검사", "가드"),
            "ro://data/sprite/방패/검사/검사_여_가드_방패.spr"
        );
    }

    #[test]
    fn shield_action_path_builds_correct_url() {
        assert_eq!(
            shield_action_path(Gender::Male, "검사", "가드"),
            "ro://data/sprite/방패/검사/검사_남_가드_방패.act"
        );
    }
}
