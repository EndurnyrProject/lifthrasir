use crate::dto::{StorageItem, StorageKind};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// The complete server-authoritative Storage snapshot.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StorageOpened {
    pub kind: StorageKind,
    pub capacity: u32,
    pub items: Vec<StorageItem>,
}

/// An item was added to Storage or its server-reported amount changed.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StorageItemAdded {
    pub kind: StorageKind,
    pub item: StorageItem,
}

/// An amount was removed from a Storage item.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StorageItemRemoved {
    pub kind: StorageKind,
    pub index: u32,
    pub amount: u32,
    pub reason: u32,
}

/// Why the server rejected opening storage or transferring an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRejection {
    Full,
    InventoryFull,
    Overweight,
    NotStorable,
    ItemEquipped,
    InvalidAmount,
    NotOpen,
    BasicSkillRequired,
    NoGuild,
    GuildNoSkill,
    GuildNoPermission,
    GuildInUse,
    OtherStorageOpen,
    Rental,
    NoGuildStorage,
    Stale,
    Unknown(i32),
}

/// The server's outcome of opening storage, a deposit, or a withdrawal.
/// The wire result has no storage kind or request identifier.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct StorageResult {
    pub outcome: Result<(), StorageRejection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_storage_rejection_retains_its_numeric_code() {
        let rejection = StorageRejection::Unknown(-37);

        assert_eq!(rejection, StorageRejection::Unknown(-37));
        assert_ne!(rejection, StorageRejection::Unknown(37));
    }
}
