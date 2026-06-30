use std::collections::HashMap;

use bevy::prelude::*;
use net_contract::commands::PickupRequested;
use net_contract::events::{ChatHeard, PickupOutcome, PickupResult};

use super::components::FloorItem;
use super::hover::HoveredFloorItem;
use crate::domain::input::ForwardedMouseClick;
use crate::infrastructure::item::ItemDb;

/// Snapshot of the floor item a pickup request was sent for, kept until the
/// server's `PickupResult` arrives so the chat line can name the item.
pub struct PickupInfo {
    pub nameid: u32,
    pub amount: u32,
    pub identified: bool,
}

/// Pickup requests awaiting a `PickupResult`, keyed by `ground_id`.
#[derive(Resource, Default)]
pub struct PendingPickups(pub HashMap<u64, PickupInfo>);

/// Clicking a hovered floor item requests its pickup (server walks + picks up).
pub fn handle_floor_item_click(
    mut mouse_click: ResMut<ForwardedMouseClick>,
    hovered: Res<HoveredFloorItem>,
    floor_items: Query<&FloorItem>,
    mut pickups: MessageWriter<PickupRequested>,
    mut pending: ResMut<PendingPickups>,
) {
    if mouse_click.position.is_none() {
        return;
    }

    let Some(entity) = hovered.0 else {
        return;
    };

    let Ok(floor_item) = floor_items.get(entity) else {
        return;
    };

    let ground_id = floor_item.ground_id;
    pickups.write(PickupRequested { ground_id });
    pending.0.insert(
        ground_id,
        PickupInfo {
            nameid: floor_item.nameid,
            amount: floor_item.amount,
            identified: floor_item.identified,
        },
    );

    mouse_click.position.take();
}

fn pickup_error_message(outcome: PickupOutcome) -> Option<&'static str> {
    match outcome {
        PickupOutcome::Ok => None,
        PickupOutcome::TooFar => Some("Too far away"),
        PickupOutcome::Overweight => Some("It's too heavy"),
        PickupOutcome::InventoryFull => Some("Inventory is full"),
        PickupOutcome::Gone => Some("The item is gone"),
        PickupOutcome::Failed => Some("Failed to pick up"),
    }
}

/// Surfaces a pickup's outcome as a chat line; inventory itself updates via
/// the existing `ItemAdded` path.
pub fn handle_pickup_result(
    mut results: MessageReader<PickupResult>,
    mut pending: ResMut<PendingPickups>,
    item_db: Res<ItemDb>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for result in results.read() {
        let info = pending.0.remove(&result.ground_id);

        match result.outcome {
            PickupOutcome::Ok => {
                let Some(info) = info else {
                    continue;
                };
                let name = item_db.name(info.nameid, info.identified).unwrap_or("item");
                chat.write(ChatHeard {
                    gid: 0,
                    message: format!("You got {} {}", info.amount, name),
                });
            }
            outcome => {
                if let Some(message) = pickup_error_message(outcome) {
                    chat.write(ChatHeard {
                        gid: 0,
                        message: message.to_string(),
                    });
                }
            }
        }
    }
}

pub fn clear_pending_pickups(mut pending: ResMut<PendingPickups>) {
    pending.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_app() -> App {
        let mut app = App::new();
        app.add_message::<PickupResult>();
        app.add_message::<ChatHeard>();
        app.init_resource::<PendingPickups>();
        app.init_resource::<ItemDb>();
        app.add_systems(Update, handle_pickup_result);
        app
    }

    fn chat_messages(app: &App) -> Vec<ChatHeard> {
        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn pickup_error_message_maps_each_variant() {
        assert_eq!(pickup_error_message(PickupOutcome::Ok), None);
        assert_eq!(
            pickup_error_message(PickupOutcome::TooFar),
            Some("Too far away")
        );
        assert_eq!(
            pickup_error_message(PickupOutcome::Overweight),
            Some("It's too heavy")
        );
        assert_eq!(
            pickup_error_message(PickupOutcome::InventoryFull),
            Some("Inventory is full")
        );
        assert_eq!(
            pickup_error_message(PickupOutcome::Gone),
            Some("The item is gone")
        );
        assert_eq!(
            pickup_error_message(PickupOutcome::Failed),
            Some("Failed to pick up")
        );
    }

    #[test]
    fn ok_outcome_writes_chat_line_and_clears_pending() {
        let mut app = result_app();
        app.world_mut().resource_mut::<PendingPickups>().0.insert(
            7,
            PickupInfo {
                nameid: 501,
                amount: 3,
                identified: true,
            },
        );

        app.world_mut()
            .resource_mut::<Messages<PickupResult>>()
            .write(PickupResult {
                ground_id: 7,
                outcome: PickupOutcome::Ok,
            });
        app.update();

        let msgs = chat_messages(&app);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].message.contains("You got"));
        assert!(msgs[0].message.contains("3"));
        assert!(msgs[0].message.contains("item"));
        assert!(!app.world().resource::<PendingPickups>().0.contains_key(&7));
    }

    #[test]
    fn error_outcome_writes_mapped_line_and_clears_pending() {
        let mut app = result_app();
        app.world_mut().resource_mut::<PendingPickups>().0.insert(
            9,
            PickupInfo {
                nameid: 501,
                amount: 1,
                identified: true,
            },
        );

        app.world_mut()
            .resource_mut::<Messages<PickupResult>>()
            .write(PickupResult {
                ground_id: 9,
                outcome: PickupOutcome::TooFar,
            });
        app.update();

        let msgs = chat_messages(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message, "Too far away");
        assert!(!app.world().resource::<PendingPickups>().0.contains_key(&9));
    }

    #[test]
    fn click_on_hovered_floor_item_requests_pickup_and_consumes_click() {
        let mut app = App::new();
        app.add_message::<PickupRequested>();
        app.init_resource::<ForwardedMouseClick>();
        app.init_resource::<HoveredFloorItem>();
        app.init_resource::<PendingPickups>();
        app.add_systems(Update, handle_floor_item_click);

        let entity = app
            .world_mut()
            .spawn(FloorItem {
                ground_id: 42,
                nameid: 501,
                amount: 5,
                identified: false,
            })
            .id();

        app.world_mut().resource_mut::<HoveredFloorItem>().0 = Some(entity);
        app.world_mut()
            .resource_mut::<ForwardedMouseClick>()
            .position = Some(Vec2::new(1.0, 2.0));

        app.update();

        let requests: Vec<_> = app
            .world()
            .resource::<Messages<PickupRequested>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].ground_id, 42);

        let pending = app.world().resource::<PendingPickups>();
        assert!(pending.0.contains_key(&42));

        assert!(app
            .world()
            .resource::<ForwardedMouseClick>()
            .position
            .is_none());
    }
}
