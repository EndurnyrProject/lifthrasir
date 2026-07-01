use crate::domain::entities::character::components::equipment::{EquipmentSet, EquipmentSlot};
use crate::domain::entities::sprite_rendering::EquipmentChangeEvent;
use crate::domain::system_sets::EntityLifecycleSystems;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

/// Headgear slots of an `EquipmentSet` that carry a non-zero view id, paired
/// with the slot that view id drives. Absent or zero view ids are skipped, so a
/// bare-headed player yields an empty list.
pub fn headgear_view_ids(equipment: &EquipmentSet) -> Vec<(EquipmentSlot, u16)> {
    [
        (EquipmentSlot::HeadTop, equipment.head_top.as_ref()),
        (EquipmentSlot::HeadMid, equipment.head_mid.as_ref()),
        (EquipmentSlot::HeadBottom, equipment.head_bottom.as_ref()),
    ]
    .into_iter()
    .filter_map(|(slot, item)| {
        let view_id = item?.sprite_id;
        (view_id != 0).then_some((slot, view_id))
    })
    .collect()
}

/// Weapon and shield slots of an `EquipmentSet` that carry a non-zero view id,
/// paired with the slot that view id drives. Mirrors `headgear_view_ids`.
pub fn weapon_shield_view_ids(equipment: &EquipmentSet) -> Vec<(EquipmentSlot, u16)> {
    [
        (EquipmentSlot::Weapon, equipment.weapon.as_ref()),
        (EquipmentSlot::Shield, equipment.shield.as_ref()),
    ]
    .into_iter()
    .filter_map(|(slot, item)| {
        let view_id = item?.sprite_id;
        (view_id != 0).then_some((slot, view_id))
    })
    .collect()
}

/// Drive remote players' equipped headgear, weapon and shield through the same
/// renderer the local player uses. A remote PC is render-ready once its sprite
/// hierarchy spawned its
/// first child (`Added<Children>`), at which point `PlayerAppearance`/`Gender` are
/// already present, so `handle_equipment_changes` can resolve the sprite. Only
/// remote spawns carry an `EquipmentSet`, so the local player is excluded by the
/// query without an explicit marker.
#[auto_add_system(
    plugin = crate::app::entity_spawning_plugin::EntitySpawningDomainPlugin,
    schedule = Update,
    config(in_set = EntityLifecycleSystems::Spawning)
)]
pub fn emit_remote_equipment_events(
    new_players: Query<(Entity, &EquipmentSet), Added<Children>>,
    mut changes: MessageWriter<EquipmentChangeEvent>,
) {
    for (entity, equipment) in new_players.iter() {
        let worn = headgear_view_ids(equipment)
            .into_iter()
            .chain(weapon_shield_view_ids(equipment));
        for (slot, view_id) in worn {
            changes.write(EquipmentChangeEvent {
                character: entity,
                slot,
                view_id: Some(view_id),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::components::equipment::EquipmentItem;

    fn item(sprite_id: u16) -> EquipmentItem {
        EquipmentItem {
            item_id: sprite_id as u32,
            sprite_id,
            refinement: 0,
            enchantments: vec![],
            options: vec![],
        }
    }

    #[test]
    fn bare_headed_player_emits_nothing() {
        assert!(headgear_view_ids(&EquipmentSet::default()).is_empty());
    }

    #[test]
    fn non_zero_headgear_slots_emit_their_view_id() {
        let equipment = EquipmentSet {
            head_top: Some(item(5)),
            head_mid: Some(item(7)),
            head_bottom: Some(item(9)),
            ..EquipmentSet::default()
        };

        assert_eq!(
            headgear_view_ids(&equipment),
            vec![
                (EquipmentSlot::HeadTop, 5),
                (EquipmentSlot::HeadMid, 7),
                (EquipmentSlot::HeadBottom, 9),
            ]
        );
    }

    #[test]
    fn zero_view_id_slots_are_skipped() {
        let equipment = EquipmentSet {
            head_top: Some(item(0)),
            head_mid: None,
            head_bottom: Some(item(42)),
            ..EquipmentSet::default()
        };

        assert_eq!(
            headgear_view_ids(&equipment),
            vec![(EquipmentSlot::HeadBottom, 42)]
        );
    }

    #[test]
    fn weapon_shield_view_ids_returns_equipped_slots_and_skips_zero_or_absent() {
        let equipped = EquipmentSet {
            weapon: Some(item(1116)),
            shield: Some(item(2)),
            ..EquipmentSet::default()
        };

        assert_eq!(
            weapon_shield_view_ids(&equipped),
            vec![(EquipmentSlot::Weapon, 1116), (EquipmentSlot::Shield, 2)]
        );

        let unequipped = EquipmentSet {
            weapon: Some(item(0)),
            shield: None,
            ..EquipmentSet::default()
        };

        assert!(weapon_shield_view_ids(&unequipped).is_empty());
    }

    #[test]
    fn render_ready_remote_player_emits_one_event_per_headgear() {
        let mut app = App::new();
        app.add_message::<EquipmentChangeEvent>();
        app.add_systems(Update, emit_remote_equipment_events);

        let equipment = EquipmentSet {
            head_top: Some(item(5)),
            head_bottom: Some(item(9)),
            ..EquipmentSet::default()
        };
        let player = app.world_mut().spawn(equipment).id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(player).add_child(child);

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<EquipmentChangeEvent>>()
            .drain()
            .map(|e| (e.character, e.slot, e.view_id))
            .collect();

        assert_eq!(
            emitted,
            vec![
                (player, EquipmentSlot::HeadTop, Some(5)),
                (player, EquipmentSlot::HeadBottom, Some(9)),
            ]
        );
    }
}
