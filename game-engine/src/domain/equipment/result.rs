use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::inventory::Inventory;
use net_contract::events::{ChatHeard, ItemEquipped, ItemUnequipped};

const EQUIP_RESULT_OK: u32 = 0;
const UNEQUIP_RESULT_OK: u32 = 0;

/// Apply equip/unequip acks to the inventory `wear_state`. These results are pure
/// inventory acks: the server always sends `view_id: 0` here and drives the
/// rendered appearance through a separate `SpriteChange` (see `sprite_change.rs`).
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn apply_equip_results(
    mut equipped: MessageReader<ItemEquipped>,
    mut unequipped: MessageReader<ItemUnequipped>,
    mut chat: MessageWriter<ChatHeard>,
    mut inventory: ResMut<Inventory>,
) {
    for result in equipped.read() {
        if result.result != EQUIP_RESULT_OK {
            chat.write(ChatHeard {
                gid: 0,
                message: equip_failure_message(result.result).to_string(),
            });
            continue;
        }

        inventory.set_wear_state(result.index as u16, result.wear_location);
    }

    for result in unequipped.read() {
        if result.result != UNEQUIP_RESULT_OK {
            chat.write(ChatHeard {
                gid: 0,
                message: "You cannot unequip this item.".to_string(),
            });
            continue;
        }

        inventory.set_wear_state(result.index as u16, 0);
    }
}

fn equip_failure_message(result: u32) -> &'static str {
    match result {
        1 => "You cannot equip this item at your level.",
        _ => "You cannot equip this item.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::equipment::location::EQP_HEAD_TOP;
    use crate::domain::inventory::Item;

    fn setup() -> App {
        let mut app = App::new();
        app.init_resource::<Inventory>()
            .add_message::<ItemEquipped>()
            .add_message::<ItemUnequipped>()
            .add_message::<ChatHeard>()
            .add_systems(Update, apply_equip_results);

        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            ..Default::default()
        });

        app
    }

    fn wear_state(app: &App) -> u32 {
        app.world()
            .resource::<Inventory>()
            .get(7)
            .unwrap()
            .wear_state
    }

    fn chat_count(app: &App) -> usize {
        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .count()
    }

    #[test]
    fn equip_success_sets_wear_state() {
        let mut app = setup();

        app.world_mut()
            .resource_mut::<Messages<ItemEquipped>>()
            .write(ItemEquipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                view_id: 0,
                result: 0,
            });

        app.update();

        assert_eq!(wear_state(&app), EQP_HEAD_TOP);
        assert_eq!(chat_count(&app), 0);
    }

    #[test]
    fn unequip_success_clears_wear_state() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<Inventory>()
            .set_wear_state(7, EQP_HEAD_TOP);

        app.world_mut()
            .resource_mut::<Messages<ItemUnequipped>>()
            .write(ItemUnequipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                result: 0,
            });

        app.update();

        assert_eq!(wear_state(&app), 0);
        assert_eq!(chat_count(&app), 0);
    }

    #[test]
    fn equip_failure_leaves_state_and_reports_chat() {
        let mut app = setup();

        app.world_mut()
            .resource_mut::<Messages<ItemEquipped>>()
            .write(ItemEquipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                view_id: 0,
                result: 1,
            });

        app.update();

        assert_eq!(wear_state(&app), 0);
        assert_eq!(chat_count(&app), 1);
    }

    #[test]
    fn unequip_failure_leaves_state_and_reports_chat() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<Inventory>()
            .set_wear_state(7, EQP_HEAD_TOP);

        app.world_mut()
            .resource_mut::<Messages<ItemUnequipped>>()
            .write(ItemUnequipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                result: 1,
            });

        app.update();

        assert_eq!(wear_state(&app), EQP_HEAD_TOP);
        assert_eq!(chat_count(&app), 1);
    }
}
