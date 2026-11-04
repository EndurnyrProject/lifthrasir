use bevy::prelude::*;
use lifthrasir_data::WeaponData;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Resource, Default)]
pub struct WeaponDb {
    names: BTreeMap<u16, String>,
    hit_sounds: BTreeMap<u16, String>,
    bow_types: BTreeSet<u16>,
}

impl WeaponDb {
    pub fn from_weapon_data(data: WeaponData) -> Self {
        Self {
            names: data.names,
            hit_sounds: data.hit_sounds,
            bow_types: data.bow_types,
        }
    }

    pub fn suffix(&self, view_id: u16) -> Option<&str> {
        self.names.get(&view_id).map(String::as_str)
    }

    pub fn hit_sound(&self, view_id: u16) -> Option<&str> {
        self.hit_sounds.get(&view_id).map(String::as_str)
    }

    pub fn is_bow(&self, view_id: u16) -> bool {
        self.bow_types.contains(&view_id)
    }

    /// Retail swing sound for a weapon type id. Returns `None` for classes that
    /// ship no `_attack_*.wav` (fists, knuckles, instruments, guns, dual wield),
    /// so the caller keeps the class ACT sound instead.
    pub fn attack_sound(&self, view_id: u16) -> Option<&'static str> {
        if self.is_bow(view_id) {
            return Some("_attack_bow.wav");
        }

        Some(match view_id {
            1 => "_attack_dagger.wav",
            2 | 3 => "_attack_sword.wav",
            4 | 5 => "_attack_spear.wav",
            6 | 7 => "_attack_axe.wav",
            8 | 9 => "_attack_mace.wav",
            10 | 23 => "_attack_rod.wav",
            15 => "_attack_book.wav",
            16 => "_attack_katar.wav",
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> WeaponDb {
        let mut data = WeaponData::default();
        data.names.insert(2, "_검".to_string());
        data.names.insert(3, "_양손검".to_string());
        data.hit_sounds.insert(2, "_hit_sword.wav".to_string());
        data.bow_types.insert(11);
        WeaponDb::from_weapon_data(data)
    }

    #[test]
    fn suffix_returns_known_sprite_suffix() {
        let db = fixture();
        assert_eq!(db.suffix(2), Some("_검"));
        assert_eq!(db.suffix(3), Some("_양손검"));
    }

    #[test]
    fn suffix_absent_view_id_returns_none() {
        let db = fixture();
        assert_eq!(db.suffix(9999), None);
    }

    #[test]
    fn hit_sound_returns_known_wav() {
        let db = fixture();
        assert_eq!(db.hit_sound(2), Some("_hit_sword.wav"));
    }

    #[test]
    fn hit_sound_absent_view_id_returns_none() {
        let db = fixture();
        assert_eq!(db.hit_sound(9999), None);
    }

    #[test]
    fn is_bow_true_for_bow_type() {
        let db = fixture();
        assert!(db.is_bow(11));
    }

    #[test]
    fn is_bow_false_for_non_bow_type() {
        let db = fixture();
        assert!(!db.is_bow(2));
    }

    #[test]
    fn attack_sound_maps_weapon_classes() {
        let db = fixture();
        assert_eq!(db.attack_sound(1), Some("_attack_dagger.wav"));
        assert_eq!(db.attack_sound(3), Some("_attack_sword.wav"));
        assert_eq!(db.attack_sound(10), Some("_attack_rod.wav"));
        assert_eq!(db.attack_sound(16), Some("_attack_katar.wav"));
    }

    #[test]
    fn attack_sound_follows_bow_type_list() {
        let mut data = WeaponData::default();
        data.bow_types.insert(11);
        data.bow_types.insert(74);
        let db = WeaponDb::from_weapon_data(data);

        assert_eq!(db.attack_sound(11), Some("_attack_bow.wav"));
        assert_eq!(db.attack_sound(74), Some("_attack_bow.wav"));
    }

    #[test]
    fn attack_sound_absent_for_fists_and_guns() {
        let db = fixture();
        assert_eq!(db.attack_sound(0), None);
        assert_eq!(db.attack_sound(17), None);
        assert_eq!(db.attack_sound(9999), None);
    }
}
