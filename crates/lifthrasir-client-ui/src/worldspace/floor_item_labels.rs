//! Labels under ground item drops: every drop whose name resolves from `ItemDb`
//! shows its name, with the count when there is more than one. The drop under the
//! pointer (`HoveredFloorItem`) gets the dark backing a hovered unit's plate gets.

use std::collections::HashSet;

use bevy::prelude::*;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::item_drop::HoveredFloorItem;
use lifthrasir_client::domain::item_drop::components::FloorItem;
use lifthrasir_ui::theme::ro_dark;

use crate::screens::shared::outlined_text;
use crate::worldspace::{WorldAnchor, WorldOverlaySystems, hung_root, plate_frame};

const LABEL_WIDTH: f32 = 220.0;
/// Pixels below the item's projected origin, so the label reads under the drop.
const LABEL_GAP: f32 = 6.0;
/// Level with nameplates; see [`super::WORLD_OVERLAY_Z`].
const LABEL_Z: i32 = super::WORLD_OVERLAY_Z + 100;

pub struct FloorItemLabelPlugin;

impl Plugin for FloorItemLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_floor_item_labels.in_set(WorldOverlaySystems::Sync),
        );
    }
}

/// A drop's label; its [`WorldAnchor`] names the drop.
#[derive(Component)]
struct FloorItemLabel {
    /// Drawn with the hover backing.
    backed: bool,
}

fn label_text(name: &str, amount: u32) -> String {
    if amount > 1 {
        format!("{name} ×{amount}")
    } else {
        name.to_string()
    }
}

fn label(text: String, backed: bool) -> impl Scene {
    bsn! {
        @hung_root(LABEL_WIDTH, LABEL_Z)
        Children [
            @plate_frame(backed)
            Children [ @outlined_text(text, 12.0, ro_dark::STRONG) ]
        ]
    }
}

/// Keeps one label per named drop, replacing the label of a drop whose hover
/// changed and dropping the labels of drops that are gone.
fn sync_floor_item_labels(
    mut commands: Commands,
    hovered: Res<HoveredFloorItem>,
    items: Query<(Entity, &FloorItem)>,
    item_db: Option<Res<ItemDb>>,
    labels: Query<(Entity, &WorldAnchor, &FloorItemLabel)>,
    mut labelled: Local<HashSet<Entity>>,
) {
    labelled.clear();
    for (entity, anchor, label) in &labels {
        let current =
            items.contains(anchor.target) && label.backed == (hovered.0 == Some(anchor.target));
        if current {
            labelled.insert(anchor.target);
        } else {
            commands.entity(entity).despawn();
        }
    }

    let Some(item_db) = item_db else {
        return;
    };
    for (target, item) in &items {
        if labelled.contains(&target) {
            continue;
        }
        let Some(name) = item_db.name(item.nameid, item.identified) else {
            continue;
        };
        let backed = hovered.0 == Some(target);
        commands
            .spawn_scene(label(label_text(name, item.amount), backed))
            .insert((
                WorldAnchor {
                    target,
                    width: LABEL_WIDTH,
                    drop: LABEL_GAP,
                },
                FloorItemLabel { backed },
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
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
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        app.init_resource::<HoveredFloorItem>();
        if let Some(item_db) = item_db {
            app.insert_resource(item_db);
        }
        app.add_systems(Update, sync_floor_item_labels);
        app
    }

    fn drop_item(app: &mut App, nameid: u32, amount: u32) -> Entity {
        app.world_mut()
            .spawn(FloorItem {
                ground_id: 1,
                nameid,
                amount,
                identified: true,
            })
            .id()
    }

    /// Each label's drop and whether it is backed.
    fn labels(app: &mut App) -> Vec<(Entity, bool)> {
        let world = app.world_mut();
        world
            .query::<(&WorldAnchor, &FloorItemLabel)>()
            .iter(world)
            .map(|(anchor, label)| (anchor.target, label.backed))
            .collect()
    }

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn a_drop_is_labelled_without_hover_and_loses_the_label_with_the_drop() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        let potion = drop_item(&mut app, 501, 1);

        app.update();
        app.update();

        assert_eq!(labels(&mut app), [(potion, false)]);
        assert_eq!(texts(&mut app), ["Red Potion"]);

        app.world_mut().despawn(potion);
        app.update();

        assert!(labels(&mut app).is_empty());
    }

    #[test]
    fn a_stack_shows_its_count() {
        let mut app = test_app(Some(item_db_with(985, "Elunium")));
        drop_item(&mut app, 985, 2);

        app.update();

        assert_eq!(texts(&mut app), ["Elunium ×2"]);
    }

    #[test]
    fn hovering_backs_the_label_and_leaving_clears_it() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        let potion = drop_item(&mut app, 501, 1);
        app.update();

        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(potion);
        app.update();
        app.update();
        assert_eq!(labels(&mut app), [(potion, true)]);

        app.world_mut().resource_mut::<HoveredFloorItem>().0 = None;
        app.update();
        app.update();
        assert_eq!(labels(&mut app), [(potion, false)]);
    }

    #[test]
    fn a_drop_with_an_unresolved_name_shows_nothing() {
        let mut app = test_app(Some(item_db_with(501, "Red Potion")));
        drop_item(&mut app, 9999, 1);

        app.update();

        assert!(labels(&mut app).is_empty());
    }

    #[test]
    fn a_drop_without_an_item_db_shows_nothing() {
        let mut app = test_app(None);
        drop_item(&mut app, 501, 1);

        app.update();

        assert!(labels(&mut app).is_empty());
    }
}
