use bevy::prelude::*;
use game_engine::domain::inventory::item::item_category;
use game_engine::domain::inventory::{Inventory, Item, ItemCategory};
use game_engine::domain::storage::Storage;
use game_engine::infrastructure::item::ItemDb;
use net_contract::dto::StorageItem;
use net_contract::events::StorageRejection;
use std::time::Duration;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageCategory {
    #[default]
    All,
    Use,
    Etc,
    Equip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSelection {
    Bag(u16),
    Vault(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTransfer {
    pub source: StorageSelection,
    pub amount: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastStorageClick {
    pub selection: StorageSelection,
    pub at: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferIntent {
    Deposit { inventory_index: u32, amount: u32 },
    Withdraw { storage_index: u32, amount: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmountValidationError {
    Empty,
    NotNumber,
    OutOfRange,
    SourceMissing,
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct StorageUi {
    pub category: StorageCategory,
    query: String,
    pub selection: Option<StorageSelection>,
    pub pending_transfer: Option<PendingTransfer>,
    pub awaiting_result: bool,
    pub panel_error: Option<String>,
    pub last_click: Option<LastStorageClick>,
    pub previous_open: bool,
}

impl StorageUi {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
    }
}

pub(crate) fn matches_filter(
    selected: StorageCategory,
    query: &str,
    item_category: ItemCategory,
    item_name: &str,
) -> bool {
    let category_matches = match selected {
        StorageCategory::All => true,
        StorageCategory::Use => item_category == ItemCategory::Use,
        StorageCategory::Etc => item_category == ItemCategory::Etc,
        StorageCategory::Equip => item_category == ItemCategory::Equip,
    };
    category_matches
        && item_name
            .to_lowercase()
            .contains(&query.trim().to_lowercase())
}

pub(crate) fn bag_projection<'a>(
    inventory: &'a Inventory,
    item_db: &ItemDb,
    category: StorageCategory,
    query: &str,
) -> Vec<&'a Item> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped())
        .filter(|item| {
            let name = item_db
                .name(item.item_id, item.identified)
                .expect("open Storage item must exist in ItemDb");
            matches_filter(category, query, item.category(), name)
        })
        .collect()
}

pub(crate) fn vault_projection<'a>(
    storage: &'a Storage,
    item_db: &ItemDb,
    category: StorageCategory,
    query: &str,
) -> Vec<&'a StorageItem> {
    storage
        .iter()
        .filter(|item| {
            let name = item_db
                .name(item.nameid, item.identified)
                .expect("open Storage item must exist in ItemDb");
            matches_filter(category, query, item_category(item.type_), name)
        })
        .collect()
}

pub(crate) fn validated_selection(
    selection: Option<StorageSelection>,
    bag: &[&Item],
    vault: &[&StorageItem],
) -> Option<StorageSelection> {
    selection.filter(|selection| match selection {
        StorageSelection::Bag(index) => bag.iter().any(|item| item.index == *index),
        StorageSelection::Vault(index) => vault.iter().any(|item| item.index == *index),
    })
}

pub(crate) fn transfer_intent(source: StorageSelection, amount: u32) -> TransferIntent {
    match source {
        StorageSelection::Bag(index) => TransferIntent::Deposit {
            inventory_index: u32::from(index),
            amount,
        },
        StorageSelection::Vault(index) => TransferIntent::Withdraw {
            storage_index: index,
            amount,
        },
    }
}

pub(crate) fn validate_live_amount(
    source: StorageSelection,
    input: &str,
    inventory: &Inventory,
    storage: &Storage,
) -> Result<u32, AmountValidationError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AmountValidationError::Empty);
    }
    let amount = input
        .parse::<u32>()
        .map_err(|_| AmountValidationError::NotNumber)?;
    let available = match source {
        StorageSelection::Bag(index) => inventory
            .get(index)
            .filter(|item| !item.is_equipped())
            .map(|item| u32::from(item.amount)),
        StorageSelection::Vault(index) => storage.get(index).map(|item| item.amount),
    }
    .ok_or(AmountValidationError::SourceMissing)?;

    if amount == 0 || amount > available {
        return Err(AmountValidationError::OutOfRange);
    }
    Ok(amount)
}

pub(crate) fn rejection_message(rejection: StorageRejection) -> String {
    match rejection {
        StorageRejection::Full => "Storage is full.".to_string(),
        StorageRejection::InventoryFull => "Your inventory is full.".to_string(),
        StorageRejection::Overweight => "You are carrying too much weight.".to_string(),
        StorageRejection::NotStorable => "This item cannot be stored.".to_string(),
        StorageRejection::ItemEquipped => "Equipped items cannot be stored.".to_string(),
        StorageRejection::InvalidAmount => "Enter a valid amount.".to_string(),
        StorageRejection::NotOpen => "Storage is not open.".to_string(),
        StorageRejection::BasicSkillRequired => "Basic Skill level 6 is required.".to_string(),
        StorageRejection::Unknown(code) => format!("Storage request failed (code {code})."),
    }
}

pub(crate) fn is_double_click(
    last: Option<LastStorageClick>,
    selection: StorageSelection,
    now: Duration,
) -> bool {
    last.is_some_and(|last| {
        last.selection == selection && now.saturating_sub(last.at) <= DOUBLE_CLICK
    })
}

pub struct StorageWindowPlugin;

impl Plugin for StorageWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StorageUi>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{ItemData, ItemInfo};

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        for (id, name) in [(501, "Red Potion"), (2104, "Buckler")] {
            data.items.insert(
                id,
                ItemInfo {
                    identified_name: name.to_string(),
                    unidentified_name: format!("Unknown {name}"),
                    ..Default::default()
                },
            );
        }
        ItemDb::from_item_data(data)
    }

    fn storage_item(index: u32, nameid: u32, type_: u32, amount: u32) -> StorageItem {
        StorageItem {
            index,
            nameid,
            amount,
            type_,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight: 0,
            identified: true,
            cards: vec![],
        }
    }

    #[test]
    fn plugin_initializes_presentation_state_only() {
        let mut app = App::new();

        app.add_plugins(StorageWindowPlugin);

        assert_eq!(app.world().resource::<StorageUi>(), &StorageUi::default());
        let ui = app.world().resource::<StorageUi>();
        assert_eq!(ui.category, StorageCategory::All);
        assert_eq!(ui.query, "");
        assert_eq!(ui.selection, None);
        assert_eq!(ui.pending_transfer, None);
        assert!(!ui.awaiting_result);
        assert_eq!(ui.panel_error, None);
        assert_eq!(ui.last_click, None);
        assert!(!ui.previous_open);
    }

    #[test]
    fn search_query_is_trimmed_and_case_normalized() {
        let mut ui = StorageUi::default();

        ui.set_query("  Red POTION  ");

        assert_eq!(ui.query(), "red potion");
    }

    #[test]
    fn category_and_search_use_one_case_insensitive_predicate() {
        assert!(matches_filter(
            StorageCategory::All,
            "red potion",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(matches_filter(
            StorageCategory::Use,
            "POTION",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(!matches_filter(
            StorageCategory::Equip,
            "potion",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(!matches_filter(
            StorageCategory::Use,
            "blue",
            ItemCategory::Use,
            "Red Potion"
        ));
    }

    #[test]
    fn bag_and_vault_share_projection_without_mutating_sources() {
        let db = item_db();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 1,
            item_id: 501,
            item_type: 0,
            identified: true,
            ..Default::default()
        });
        inventory.upsert(Item {
            index: 2,
            item_id: 501,
            item_type: 0,
            wear_state: 1,
            identified: true,
            ..Default::default()
        });
        inventory.upsert(Item {
            index: 3,
            item_id: 2104,
            item_type: 4,
            identified: true,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(
            100,
            vec![
                storage_item(70_000, 501, 0, 10),
                storage_item(70_001, 2104, 4, 1),
            ],
        );

        let bag = bag_projection(&inventory, &db, StorageCategory::Use, "POTION");
        let vault = vault_projection(&storage, &db, StorageCategory::Use, "POTION");

        assert_eq!(bag.iter().map(|item| item.index).collect::<Vec<_>>(), [1]);
        assert_eq!(
            vault.iter().map(|item| item.index).collect::<Vec<_>>(),
            [70_000]
        );
        assert_eq!(inventory.len(), 3);
        assert_eq!(storage.len(), 2);
        assert!(inventory.get(2).unwrap().is_equipped());
    }

    #[test]
    fn selection_is_cleared_when_missing_from_the_filtered_pane() {
        let bag_item = Item {
            index: 7,
            ..Default::default()
        };
        let vault_item = storage_item(70_000, 501, 0, 1);

        assert_eq!(
            validated_selection(Some(StorageSelection::Bag(7)), &[&bag_item], &[&vault_item]),
            Some(StorageSelection::Bag(7))
        );
        assert_eq!(
            validated_selection(Some(StorageSelection::Bag(8)), &[&bag_item], &[&vault_item]),
            None
        );
        assert_eq!(
            validated_selection(
                Some(StorageSelection::Vault(70_001)),
                &[&bag_item],
                &[&vault_item]
            ),
            None
        );
    }

    #[test]
    fn transfer_intent_preserves_direction_indices_and_amount() {
        assert_eq!(
            transfer_intent(StorageSelection::Bag(u16::MAX), 70_000),
            TransferIntent::Deposit {
                inventory_index: u16::MAX as u32,
                amount: 70_000,
            }
        );
        assert_eq!(
            transfer_intent(StorageSelection::Vault(70_001), 80_000),
            TransferIntent::Withdraw {
                storage_index: 70_001,
                amount: 80_000,
            }
        );
    }

    #[test]
    fn amount_validation_uses_the_live_source_stack() {
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 7,
            amount: 5,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(100, vec![storage_item(70_000, 501, 0, 4)]);

        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "5", &inventory, &storage),
            Ok(5)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Vault(70_000), "4", &inventory, &storage),
            Ok(4)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "", &inventory, &storage),
            Err(AmountValidationError::Empty)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "0", &inventory, &storage),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "five", &inventory, &storage),
            Err(AmountValidationError::NotNumber)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "6", &inventory, &storage),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Vault(9), "1", &inventory, &storage),
            Err(AmountValidationError::SourceMissing)
        );
    }

    #[test]
    fn every_storage_rejection_has_a_panel_message() {
        let cases = [
            (StorageRejection::Full, "Storage is full."),
            (StorageRejection::InventoryFull, "Your inventory is full."),
            (
                StorageRejection::Overweight,
                "You are carrying too much weight.",
            ),
            (StorageRejection::NotStorable, "This item cannot be stored."),
            (
                StorageRejection::ItemEquipped,
                "Equipped items cannot be stored.",
            ),
            (StorageRejection::InvalidAmount, "Enter a valid amount."),
            (StorageRejection::NotOpen, "Storage is not open."),
            (
                StorageRejection::BasicSkillRequired,
                "Basic Skill level 6 is required.",
            ),
        ];

        for (rejection, expected) in cases {
            assert_eq!(rejection_message(rejection), expected);
        }
        assert_eq!(
            rejection_message(StorageRejection::Unknown(-37)),
            "Storage request failed (code -37)."
        );
    }

    #[test]
    fn double_click_requires_the_same_selection_within_300ms() {
        let last = LastStorageClick {
            selection: StorageSelection::Vault(70_000),
            at: Duration::from_millis(100),
        };

        assert!(is_double_click(
            Some(last),
            StorageSelection::Vault(70_000),
            Duration::from_millis(400)
        ));
        assert!(!is_double_click(
            Some(last),
            StorageSelection::Vault(70_000),
            Duration::from_millis(401)
        ));
        assert!(!is_double_click(
            Some(last),
            StorageSelection::Bag(70),
            Duration::from_millis(200)
        ));
        assert!(!is_double_click(
            None,
            StorageSelection::Vault(70_000),
            Duration::from_millis(200)
        ));
    }
}
