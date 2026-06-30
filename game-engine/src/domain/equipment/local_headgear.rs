use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::sprite_rendering::EquipmentChangeEvent;
use crate::domain::equipment::location::{EQP_HEAD_LOW, EQP_HEAD_MID, EQP_HEAD_TOP};
use crate::domain::inventory::Inventory;

/// Tracks the headgear view ids currently rendered on the local player so the
/// inventory-driven sync only emits an `EquipmentChangeEvent` when the worn set
/// actually changes. Lives on the local-player entity, so it resets with it on
/// character switch / relogin.
#[derive(Component, Default)]
pub struct LocalHeadgearApplied {
    head_top: Option<u16>,
    head_mid: Option<u16>,
    head_bottom: Option<u16>,
}

/// Render the local player's headgear from its own `Inventory` — the authoritative
/// worn state the server sends on login (the `equip` list, with `wear_state` and the
/// `view_sprite` view id) and updates on every equip/unequip ack. One path covers
/// login, live equip and unequip and, unlike the remote `SpriteChange` route, never
/// depends on the self-targeted broadcast round-tripping. A headgear with view id 0
/// (no sprite) maps to `None`.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn sync_local_player_headgear(
    mut commands: Commands,
    inventory: Res<Inventory>,
    mut changes: MessageWriter<EquipmentChangeEvent>,
    // `With<Children>` gates on the sprite hierarchy existing (the same access
    // `handle_equipment_changes` requires), so emitted events are never dropped
    // on a bare entity that has not finished spawning its render layers.
    mut player: Query<
        (Entity, Option<&mut LocalHeadgearApplied>),
        (With<LocalPlayer>, With<Children>),
    >,
) {
    if !inventory.is_ready() {
        return;
    }

    let Ok((entity, applied)) = player.single_mut() else {
        return;
    };

    let Some(mut applied) = applied else {
        commands
            .entity(entity)
            .insert(LocalHeadgearApplied::default());
        return;
    };

    let (top, mid, bottom) = desired_headgear(&inventory);

    reconcile(
        &mut changes,
        entity,
        EquipmentSlot::HeadTop,
        top,
        &mut applied.head_top,
    );
    reconcile(
        &mut changes,
        entity,
        EquipmentSlot::HeadMid,
        mid,
        &mut applied.head_mid,
    );
    reconcile(
        &mut changes,
        entity,
        EquipmentSlot::HeadBottom,
        bottom,
        &mut applied.head_bottom,
    );
}

/// The view ids worn in the three head slots, as `(top, mid, bottom)`. A slot with
/// no worn headgear, or a headgear with view id 0, is `None`.
fn desired_headgear(inventory: &Inventory) -> (Option<u16>, Option<u16>, Option<u16>) {
    let (mut top, mut mid, mut bottom) = (None, None, None);
    for item in inventory.equipped() {
        let view = (item.view_sprite != 0).then_some(item.view_sprite);
        if item.wear_state & EQP_HEAD_TOP != 0 {
            top = view;
        }
        if item.wear_state & EQP_HEAD_MID != 0 {
            mid = view;
        }
        if item.wear_state & EQP_HEAD_LOW != 0 {
            bottom = view;
        }
    }
    (top, mid, bottom)
}

fn reconcile(
    changes: &mut MessageWriter<EquipmentChangeEvent>,
    character: Entity,
    slot: EquipmentSlot,
    desired: Option<u16>,
    applied: &mut Option<u16>,
) {
    if *applied == desired {
        return;
    }
    changes.write(EquipmentChangeEvent {
        character,
        slot,
        view_id: desired,
    });
    *applied = desired;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::inventory::Item;

    fn headgear(index: u16, wear: u32, view: u16) -> Item {
        Item {
            index,
            wear_state: wear,
            view_sprite: view,
            amount: 1,
            item_type: 5,
            ..Default::default()
        }
    }

    fn set_inventory(app: &mut App, items: impl IntoIterator<Item = Item>) {
        let mut inventory = app.world_mut().resource_mut::<Inventory>();
        inventory.begin();
        for item in items {
            inventory.upsert(item);
        }
        inventory.finish();
    }

    fn app_with_local_player() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Inventory>()
            .add_message::<EquipmentChangeEvent>()
            .add_systems(Update, sync_local_player_headgear);

        let player = app
            .world_mut()
            .spawn(LocalPlayer)
            .with_children(|parent| {
                parent.spawn_empty();
            })
            .id();

        (app, player)
    }

    fn emitted(app: &App) -> Vec<(EquipmentSlot, Option<u16>)> {
        app.world()
            .resource::<Messages<EquipmentChangeEvent>>()
            .iter_current_update_messages()
            .map(|e| (e.slot, e.view_id))
            .collect()
    }

    #[test]
    fn desired_headgear_reads_each_head_slot() {
        let mut inv = Inventory::default();
        inv.upsert(headgear(2, EQP_HEAD_TOP, 42));
        inv.upsert(headgear(3, EQP_HEAD_MID, 7));
        inv.upsert(headgear(4, EQP_HEAD_LOW, 99));
        inv.finish();

        assert_eq!(desired_headgear(&inv), (Some(42), Some(7), Some(99)));
    }

    #[test]
    fn zero_view_headgear_is_treated_as_no_sprite() {
        let mut inv = Inventory::default();
        inv.upsert(headgear(2, EQP_HEAD_TOP, 0));
        inv.finish();

        assert_eq!(desired_headgear(&inv), (None, None, None));
    }

    #[test]
    fn worn_headgear_emits_equipment_change() {
        let (mut app, player) = app_with_local_player();
        set_inventory(&mut app, [headgear(2, EQP_HEAD_TOP, 42)]);

        // First tick inserts the tracker, second reconciles against it.
        app.update();
        app.update();

        assert_eq!(emitted(&app), vec![(EquipmentSlot::HeadTop, Some(42))]);
        assert!(app.world().get::<LocalHeadgearApplied>(player).is_some());
    }

    #[test]
    fn unchanged_inventory_emits_once() {
        let (mut app, _player) = app_with_local_player();
        set_inventory(&mut app, [headgear(2, EQP_HEAD_TOP, 42)]);

        app.update();
        app.update();
        app.update();

        // The third tick sees no change, so nothing is re-emitted.
        assert!(emitted(&app).is_empty());
    }

    #[test]
    fn unequip_emits_removal() {
        let (mut app, _player) = app_with_local_player();
        set_inventory(&mut app, [headgear(2, EQP_HEAD_TOP, 42)]);
        app.update();
        app.update();

        set_inventory(&mut app, []);
        app.update();

        assert_eq!(emitted(&app), vec![(EquipmentSlot::HeadTop, None)]);
    }

    #[test]
    fn bare_entity_without_children_is_skipped() {
        let mut app = App::new();
        app.init_resource::<Inventory>()
            .add_message::<EquipmentChangeEvent>()
            .add_systems(Update, sync_local_player_headgear);
        app.world_mut().spawn(LocalPlayer);
        set_inventory(&mut app, [headgear(2, EQP_HEAD_TOP, 42)]);

        app.update();
        app.update();

        assert!(emitted(&app).is_empty());
    }
}
