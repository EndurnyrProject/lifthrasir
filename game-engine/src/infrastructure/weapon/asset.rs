use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct WeaponDataAsset(pub lifthrasir_data::WeaponData);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ron_into_weapon_data() {
        let ron = r#"(names:{2:"_검"},hit_sounds:{2:"_hit_sword.wav"},bow_types:[11])"#;
        let asset = ron::from_str::<WeaponDataAsset>(ron).expect("deserialize");

        assert_eq!(asset.0.names[&2], "_검");
        assert_eq!(asset.0.hit_sounds[&2], "_hit_sword.wav");
        assert!(asset.0.bow_types.contains(&11));
    }
}
