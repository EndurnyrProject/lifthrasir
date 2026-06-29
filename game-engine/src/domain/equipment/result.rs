use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::sprite_rendering::EquipmentChangeEvent;
use crate::domain::equipment::decode_wear_location;
use crate::domain::inventory::Inventory;
use crate::infrastructure::networking::zone_messages::{ChatHeard, ItemEquipped, ItemUnequipped};

const EQUIP_RESULT_OK: u32 = 0;
const UNEQUIP_RESULT_OK: u32 = 0;

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn apply_equip_results(
    mut equipped: MessageReader<ItemEquipped>,
    mut unequipped: MessageReader<ItemUnequipped>,
    mut changes: MessageWriter<EquipmentChangeEvent>,
    mut chat: MessageWriter<ChatHeard>,
    mut inventory: ResMut<Inventory>,
    local_player: Query<Entity, With<LocalPlayer>>,
) {
    let player = local_player.single().ok();

    for result in equipped.read() {
        if result.result != EQUIP_RESULT_OK {
            chat.write(ChatHeard {
                gid: 0,
                message: equip_failure_message(result.result).to_string(),
            });
            continue;
        }

        inventory.set_wear_state(result.index as u16, result.wear_location);
        emit_headgear_changes(
            &mut changes,
            player,
            result.wear_location,
            Some(result.view_id as u16),
        );
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
        emit_headgear_changes(&mut changes, player, result.wear_location, None);
    }
}

fn equip_failure_message(result: u32) -> &'static str {
    match result {
        1 => "You cannot equip this item at your level.",
        _ => "You cannot equip this item.",
    }
}

fn is_headgear_slot(slot: &EquipmentSlot) -> bool {
    matches!(
        slot,
        EquipmentSlot::HeadTop | EquipmentSlot::HeadMid | EquipmentSlot::HeadBottom
    )
}

fn emit_headgear_changes(
    changes: &mut MessageWriter<EquipmentChangeEvent>,
    player: Option<Entity>,
    wear_location: u32,
    view_id: Option<u16>,
) {
    let Some(character) = player else {
        warn!("apply_equip_results: no local player; skipping headgear render update");
        return;
    };

    for slot in decode_wear_location(wear_location)
        .into_iter()
        .filter(is_headgear_slot)
    {
        changes.write(EquipmentChangeEvent {
            character,
            slot,
            view_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::equipment::location::EQP_HEAD_TOP;
    use crate::domain::inventory::Item;

    fn setup() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Inventory>()
            .add_message::<ItemEquipped>()
            .add_message::<ItemUnequipped>()
            .add_message::<EquipmentChangeEvent>()
            .add_message::<ChatHeard>()
            .add_systems(Update, apply_equip_results);

        let player = app.world_mut().spawn(LocalPlayer).id();

        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            ..Default::default()
        });

        (app, player)
    }

    fn emitted_changes(app: &App) -> Vec<EquipmentChangeEvent> {
        app.world()
            .resource::<Messages<EquipmentChangeEvent>>()
            .iter_current_update_messages()
            .map(|e| EquipmentChangeEvent {
                character: e.character,
                slot: e.slot,
                view_id: e.view_id,
            })
            .collect()
    }

    #[test]
    fn equip_success_sets_wear_state_and_emits_change() {
        let (mut app, player) = setup();

        app.world_mut()
            .resource_mut::<Messages<ItemEquipped>>()
            .write(ItemEquipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                view_id: 42,
                result: 0,
            });

        app.update();

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .get(7)
                .unwrap()
                .wear_state,
            EQP_HEAD_TOP
        );

        let changes = emitted_changes(&app);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].character, player);
        assert_eq!(changes[0].slot, EquipmentSlot::HeadTop);
        assert_eq!(changes[0].view_id, Some(42));
    }

    #[test]
    fn unequip_success_clears_wear_state_and_emits_removal() {
        let (mut app, _player) = setup();
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

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .get(7)
                .unwrap()
                .wear_state,
            0
        );

        let changes = emitted_changes(&app);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].slot, EquipmentSlot::HeadTop);
        assert_eq!(changes[0].view_id, None);
    }

    #[test]
    fn equip_failure_leaves_state_and_reports_chat() {
        let (mut app, _player) = setup();

        app.world_mut()
            .resource_mut::<Messages<ItemEquipped>>()
            .write(ItemEquipped {
                index: 7,
                wear_location: EQP_HEAD_TOP,
                view_id: 42,
                result: 1,
            });

        app.update();

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .get(7)
                .unwrap()
                .wear_state,
            0
        );
        assert!(emitted_changes(&app).is_empty());

        let chat = app.world().resource::<Messages<ChatHeard>>();
        assert_eq!(chat.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn unequip_failure_leaves_state_and_reports_chat() {
        let (mut app, _player) = setup();
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

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .get(7)
                .unwrap()
                .wear_state,
            EQP_HEAD_TOP
        );
        assert!(emitted_changes(&app).is_empty());

        let chat = app.world().resource::<Messages<ChatHeard>>();
        assert_eq!(chat.iter_current_update_messages().count(), 1);
    }
}
