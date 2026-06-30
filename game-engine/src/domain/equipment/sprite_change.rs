use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::sprite_rendering::EquipmentChangeEvent;
use crate::infrastructure::networking::zone_messages::UnitSpriteChanged;

const LOOK_HEAD_BOTTOM: u32 = 3;
const LOOK_HEAD_TOP: u32 = 4;
const LOOK_HEAD_MID: u32 = 5;

fn headgear_slot(look_type: u32) -> Option<EquipmentSlot> {
    match look_type {
        LOOK_HEAD_BOTTOM => Some(EquipmentSlot::HeadBottom),
        LOOK_HEAD_TOP => Some(EquipmentSlot::HeadTop),
        LOOK_HEAD_MID => Some(EquipmentSlot::HeadMid),
        _ => None,
    }
}

/// Drive *remote* headgear rendering from the server's `SpriteChange`, the
/// authoritative carrier of appearance view ids (`EquipResult.view_id` is always 0).
/// The local player is skipped here and driven from its own `Inventory` instead
/// (see `local_headgear::sync_local_player_headgear`), so its appearance never
/// depends on the self-targeted broadcast round-tripping. `val == 0` means the slot
/// cleared, mapped to `view_id: None`. Non-headgear look types are skipped.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn apply_sprite_changes(
    mut sprite_changes: MessageReader<UnitSpriteChanged>,
    mut changes: MessageWriter<EquipmentChangeEvent>,
    registry: Res<EntityRegistry>,
) {
    for change in sprite_changes.read() {
        let Some(slot) = headgear_slot(change.type_) else {
            continue;
        };
        let Some(character) = registry.get_entity(change.gid) else {
            continue;
        };
        if registry.local_player_entity() == Some(character) {
            continue;
        }

        changes.write(EquipmentChangeEvent {
            character,
            slot,
            view_id: (change.val != 0).then_some(change.val as u16),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (App, Entity, u32) {
        let mut app = App::new();
        app.init_resource::<EntityRegistry>()
            .add_message::<UnitSpriteChanged>()
            .add_message::<EquipmentChangeEvent>()
            .add_systems(Update, apply_sprite_changes);

        let character = app.world_mut().spawn_empty().id();
        let gid = 150001;
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(gid, character);

        (app, character, gid)
    }

    fn emitted(app: &App) -> Vec<(Entity, EquipmentSlot, Option<u16>)> {
        app.world()
            .resource::<Messages<EquipmentChangeEvent>>()
            .iter_current_update_messages()
            .map(|e| (e.character, e.slot, e.view_id))
            .collect()
    }

    #[test]
    fn headgear_slot_maps_look_types() {
        assert_eq!(
            headgear_slot(LOOK_HEAD_BOTTOM),
            Some(EquipmentSlot::HeadBottom)
        );
        assert_eq!(headgear_slot(LOOK_HEAD_TOP), Some(EquipmentSlot::HeadTop));
        assert_eq!(headgear_slot(LOOK_HEAD_MID), Some(EquipmentSlot::HeadMid));
        assert_eq!(headgear_slot(2), None);
    }

    #[test]
    fn head_top_sprite_change_emits_equipment_change() {
        let (mut app, character, gid) = setup();
        app.world_mut()
            .resource_mut::<Messages<UnitSpriteChanged>>()
            .write(UnitSpriteChanged {
                gid,
                type_: LOOK_HEAD_TOP,
                val: 42,
                val2: 0,
            });

        app.update();

        assert_eq!(
            emitted(&app),
            vec![(character, EquipmentSlot::HeadTop, Some(42))]
        );
    }

    #[test]
    fn zero_val_emits_removal() {
        let (mut app, character, gid) = setup();
        app.world_mut()
            .resource_mut::<Messages<UnitSpriteChanged>>()
            .write(UnitSpriteChanged {
                gid,
                type_: LOOK_HEAD_MID,
                val: 0,
                val2: 0,
            });

        app.update();

        assert_eq!(
            emitted(&app),
            vec![(character, EquipmentSlot::HeadMid, None)]
        );
    }

    #[test]
    fn non_headgear_look_type_is_ignored() {
        let (mut app, _character, gid) = setup();
        app.world_mut()
            .resource_mut::<Messages<UnitSpriteChanged>>()
            .write(UnitSpriteChanged {
                gid,
                type_: 2,
                val: 13,
                val2: 0,
            });

        app.update();

        assert!(emitted(&app).is_empty());
    }

    #[test]
    fn unregistered_gid_is_skipped() {
        let (mut app, _character, _gid) = setup();
        app.world_mut()
            .resource_mut::<Messages<UnitSpriteChanged>>()
            .write(UnitSpriteChanged {
                gid: 999999,
                type_: LOOK_HEAD_TOP,
                val: 42,
                val2: 0,
            });

        app.update();

        assert!(emitted(&app).is_empty());
    }
}
