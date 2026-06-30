//! Hover label for ground item drops: a screen-space text node showing the
//! item's display name while `HoveredFloorItem` points at it. Mirrors
//! `nameplates.rs` (spawn-on-hover, project-and-follow, despawn-on-exit) but
//! is keyed off the single `HoveredFloorItem` resource rather than a
//! per-entity hover marker, since at most one drop can be hovered at a time.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::item_drop::components::FloorItem;
use game_engine::domain::item_drop::HoveredFloorItem;
use game_engine::infrastructure::item::ItemDb;

use crate::theme;
use crate::worldspace::{viewport_to_ui, WorldCameraFilter, WorldspaceFont};

const FLOOR_ITEM_LABEL_WIDTH: f32 = 220.0;
const FLOOR_ITEM_LABEL_FONT_SIZE: f32 = 13.0;
/// Pixels above the item's projected origin, so the label reads over the drop.
const FLOOR_ITEM_LABEL_GAP: f32 = 18.0;
/// Above the world camera, below the fade overlay (`i32::MAX - 1`) and cursor.
const FLOOR_ITEM_LABEL_Z: i32 = 100;

pub struct FloorItemLabelPlugin;

impl Plugin for FloorItemLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_floor_item_labels, follow_floor_item_labels).run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_floor_item_labels);
    }
}

#[derive(Component)]
struct FloorItemLabel {
    target: Entity,
}

fn spawn_floor_item_label(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    name: &str,
) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(FLOOR_ITEM_LABEL_WIDTH),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(FLOOR_ITEM_LABEL_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
        FloorItemLabel { target },
        children![(
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            children![(
                Text::new(name),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FLOOR_ITEM_LABEL_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(theme::TEXT),
                Pickable::IGNORE,
            )],
        )],
    ));
}

/// Keeps at most one label, for the currently hovered floor item. Despawns
/// when the hover target changes or clears; spawns only once the item's name
/// resolves from `ItemDb`.
fn sync_floor_item_labels(
    mut commands: Commands,
    hovered: Res<HoveredFloorItem>,
    floor_items: Query<&FloorItem>,
    item_db: Option<Res<ItemDb>>,
    font: Res<WorldspaceFont>,
    labels: Query<(Entity, &FloorItemLabel)>,
) {
    if let Some(target) = hovered.0 {
        let already_labeled = labels.iter().any(|(_, label)| label.target == target);
        let name = floor_items
            .get(target)
            .ok()
            .zip(item_db.as_deref())
            .and_then(|(item, db)| db.name(item.nameid, item.identified));
        if !already_labeled {
            if let Some(name) = name {
                spawn_floor_item_label(&mut commands, &font, target, name);
            }
        }
    }

    for (entity, label) in &labels {
        if hovered.0 != Some(label.target) {
            commands.entity(entity).despawn();
        }
    }
}

fn follow_floor_item_labels(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
    mut labels: Query<(Entity, &FloorItemLabel, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(target_transform) = targets.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - FLOOR_ITEM_LABEL_WIDTH / 2.0);
                node.top = Val::Px(pos.y - FLOOR_ITEM_LABEL_GAP);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all_floor_item_labels(
    mut commands: Commands,
    labels: Query<Entity, With<FloorItemLabel>>,
) {
    for entity in &labels {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{ItemData, ItemInfo};

    fn item_db_with(nameid: u32, identified_name: &str) -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            nameid,
            ItemInfo {
                identified_name: identified_name.to_string(),
                identified_resource: "RESOURCE".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    fn test_app(item_db: Option<ItemDb>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(WorldspaceFont(Handle::default()));
        app.init_resource::<HoveredFloorItem>();
        if let Some(item_db) = item_db {
            app.insert_resource(item_db);
        }
        app.add_systems(Update, sync_floor_item_labels);
        app
    }

    fn label_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&FloorItemLabel>().iter(world).count()
    }

    #[test]
    fn hovered_item_gets_label_then_loses_it_on_unhover() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        let target = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 1,
                nameid: 501,
                amount: 1,
                identified: true,
            })
            .id();
        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(target);

        app.update();

        let world = app.world_mut();
        let labels: Vec<&FloorItemLabel> = world.query::<&FloorItemLabel>().iter(world).collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].target, target);
        let text = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .next();
        assert_eq!(text.as_deref(), Some("Red Potion"));

        app.world_mut().resource_mut::<HoveredFloorItem>().0 = None;
        app.update();

        assert_eq!(label_count(&mut app), 0);
    }

    #[test]
    fn hovering_a_different_item_moves_the_label() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        let first = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 1,
                nameid: 501,
                amount: 1,
                identified: true,
            })
            .id();
        let second = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 2,
                nameid: 501,
                amount: 1,
                identified: true,
            })
            .id();
        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(first);
        app.update();
        assert_eq!(label_count(&mut app), 1);

        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(second);
        app.update();

        let world = app.world_mut();
        let labels: Vec<&FloorItemLabel> = world.query::<&FloorItemLabel>().iter(world).collect();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].target, second);
    }

    #[test]
    fn hovered_item_with_unresolved_name_spawns_nothing() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        let target = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 1,
                nameid: 9999,
                amount: 1,
                identified: true,
            })
            .id();
        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(target);

        app.update();

        assert_eq!(label_count(&mut app), 0);
    }

    #[test]
    fn hovered_item_without_item_db_spawns_nothing() {
        let mut app = test_app(None);
        let target = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 1,
                nameid: 501,
                amount: 1,
                identified: true,
            })
            .id();
        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(target);

        app.update();

        assert_eq!(label_count(&mut app), 0);
    }
}
