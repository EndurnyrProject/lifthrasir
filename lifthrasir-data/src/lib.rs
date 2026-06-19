use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobData {
    /// Lua JobNameTable: job id -> sprite name (NPC + monster job sprites).
    pub npc_sprites: BTreeMap<u32, String>,
    /// Lua PCJobNameTable: job id -> display name.
    pub display_names: BTreeMap<u32, String>,
}

/// Per-item presentation metadata decoded from `iteminfo.lub`.
/// All strings are valid UTF-8 (EUC-KR decoded by the CLI converter).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemInfo {
    pub identified_name: String,
    pub identified_resource: String,
    pub identified_description: Vec<String>,
    pub unidentified_name: String,
    pub unidentified_resource: String,
    pub unidentified_description: Vec<String>,
    pub slot_count: u8,
}

/// Full item database: item id -> presentation metadata.
/// Keyed by `BTreeMap` for stable, key-ordered RON diffs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemData {
    pub items: BTreeMap<u32, ItemInfo>,
}

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
}
