use std::collections::HashMap;

use bevy::prelude::*;
use net_contract::events::{ChatHeard, PickupOutcome, PickupResult};

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
}
