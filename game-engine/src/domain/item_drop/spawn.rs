use super::animation::FallingDrop;
use super::components::{FloorItem, FloorItemRegistry};
use crate::domain::entities::character::components::core::Grounded;
use crate::domain::entities::sprite_rendering::components::{EntitySpriteData, EntitySpriteInfo};
use crate::domain::entities::sprite_rendering::events::RequestSpriteSpawn;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::item::ItemDb;
use crate::utils::coordinates::spawn_coords_to_world_position;
use bevy::prelude::*;
use net_contract::events::{ItemOnGround, ItemVanished};

/// Sub-cell scatter offset in world units. The server picks one of `{3,6,9,12}`
/// on each axis; `7.5` is the cell centre and `5.0` is `RO_UNITS_PER_CELL` (the
/// same constant `spawn_coords_to_world_position` uses), so the four values map
/// to a symmetric `{-1.5,-0.5,0.5,1.5}` within the 5.0-wide cell.
pub fn sub_cell_offset(sub: u8) -> f32 {
    ((sub as f32 - 7.5) / 15.0) * 5.0
}

pub fn spawn_floor_items(
    mut events: MessageReader<ItemOnGround>,
    mut commands: Commands,
    mut registry: ResMut<FloorItemRegistry>,
    item_db: Res<ItemDb>,
) {
    for ev in events.read() {
        if registry.0.contains_key(&ev.ground_id) {
            continue;
        }

        let mut pos = spawn_coords_to_world_position(ev.x, ev.y, 0, 0);
        pos.x += sub_cell_offset(ev.sub_x);
        pos.z += sub_cell_offset(ev.sub_y);

        let mut entity_commands = commands.spawn((
            FloorItem {
                ground_id: ev.ground_id,
                nameid: ev.nameid,
                amount: ev.amount,
                identified: ev.identified,
            },
            Transform::from_translation(pos),
            Visibility::default(),
            Name::new(format!("FloorItem({})", ev.ground_id)),
            MapScoped,
        ));

        if ev.is_falling {
            entity_commands.insert(FallingDrop::default());
        } else {
            entity_commands.insert(Grounded);
        }

        let entity = entity_commands.id();

        match item_db.icon_resource(ev.nameid, ev.identified) {
            Some(resource) => {
                commands.trigger(RequestSpriteSpawn {
                    entity,
                    position: pos,
                    sprite_info: EntitySpriteInfo {
                        sprite_data: EntitySpriteData::Item {
                            sprite_name: resource.to_string(),
                        },
                    },
                });
            }
            None => {
                warn!(
                    "No collection sprite for item {} (identified={}); spawning interactable drop without sprite",
                    ev.nameid, ev.identified
                );
            }
        }

        registry.0.insert(ev.ground_id, entity);
    }
}

pub fn despawn_floor_items(
    mut events: MessageReader<ItemVanished>,
    mut commands: Commands,
    mut registry: ResMut<FloorItemRegistry>,
) {
    for ev in events.read() {
        if let Some(entity) = registry.0.remove(&ev.ground_id) {
            commands.entity(entity).try_despawn();
        }
    }
}

/// `MapScoped` already reaps the entities on map exit; this just clears the map.
pub fn clear_floor_item_registry(mut registry: ResMut<FloorItemRegistry>) {
    registry.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::VanishReason;

    fn on_ground(ground_id: u64) -> ItemOnGround {
        ItemOnGround {
            ground_id,
            nameid: 501,
            amount: 1,
            x: 100,
            y: 120,
            identified: true,
            is_falling: false,
            sub_x: 6,
            sub_y: 9,
        }
    }

    fn vanished(ground_id: u64) -> ItemVanished {
        ItemVanished {
            ground_id,
            reason: VanishReason::PickedUp,
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<ItemOnGround>();
        app.add_message::<ItemVanished>();
        app.init_resource::<FloorItemRegistry>();
        app.init_resource::<ItemDb>();
        app.add_systems(Update, (spawn_floor_items, despawn_floor_items));
        app
    }

    #[test]
    fn sub_cell_offset_is_symmetric() {
        assert_eq!(sub_cell_offset(3), -1.5);
        assert_eq!(sub_cell_offset(6), -0.5);
        assert_eq!(sub_cell_offset(9), 0.5);
        assert_eq!(sub_cell_offset(12), 1.5);
    }

    #[test]
    fn duplicate_ground_id_yields_one_entity() {
        let mut app = test_app();

        app.world_mut().write_message(on_ground(42));
        app.update();
        app.world_mut().write_message(on_ground(42));
        app.update();

        let registry = app.world().resource::<FloorItemRegistry>();
        assert_eq!(registry.0.len(), 1);
        assert!(registry.0.contains_key(&42));

        let count = app
            .world_mut()
            .query::<&FloorItem>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn vanish_removes_entry_and_despawns() {
        let mut app = test_app();

        app.world_mut().write_message(on_ground(7));
        app.update();
        let entity = *app
            .world()
            .resource::<FloorItemRegistry>()
            .0
            .get(&7)
            .expect("registered");

        app.world_mut().write_message(vanished(7));
        app.update();

        assert!(
            !app.world()
                .resource::<FloorItemRegistry>()
                .0
                .contains_key(&7)
        );
        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn vanish_for_unknown_id_is_noop() {
        let mut app = test_app();

        app.world_mut().write_message(on_ground(1));
        app.update();

        app.world_mut().write_message(vanished(999));
        app.update();

        let registry = app.world().resource::<FloorItemRegistry>();
        assert_eq!(registry.0.len(), 1);
        assert!(registry.0.contains_key(&1));
    }

    #[test]
    fn falling_drop_gets_falling_drop_not_grounded() {
        let mut app = test_app();

        let mut ev = on_ground(50);
        ev.is_falling = true;
        app.world_mut().write_message(ev);
        app.update();

        let entity = *app
            .world()
            .resource::<FloorItemRegistry>()
            .0
            .get(&50)
            .expect("registered");

        assert!(app.world().get::<FallingDrop>(entity).is_some());
        assert!(app.world().get::<Grounded>(entity).is_none());
    }

    #[test]
    fn resting_drop_gets_grounded_not_falling_drop() {
        let mut app = test_app();

        app.world_mut().write_message(on_ground(51));
        app.update();

        let entity = *app
            .world()
            .resource::<FloorItemRegistry>()
            .0
            .get(&51)
            .expect("registered");

        assert!(app.world().get::<Grounded>(entity).is_some());
        assert!(app.world().get::<FallingDrop>(entity).is_none());
    }
}
