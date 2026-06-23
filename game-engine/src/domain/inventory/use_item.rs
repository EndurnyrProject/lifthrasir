use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_quinnet::client::QuinnetClient;

use crate::core::state::GameState;
use crate::infrastructure::networking::quic::channels::GAMEPLAY;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::UseItem;
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};
use crate::infrastructure::networking::zone_messages::{ChatHeard, ItemUseFailed};

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct UseItemRequested {
    pub index: u32,
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_use_item_send(
    mut events: MessageReader<UseItemRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }

    for event in events.read() {
        let body = Body::UseItem(UseItem { index: event.index });
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("Failed to send use-item request: {e}");
        }
    }
}

fn use_failure_message(reason: u32) -> &'static str {
    match reason {
        1 => "Item not found.",
        2 => "You cannot use this item.",
        _ => "You cannot use that right now.",
    }
}

#[auto_add_system(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin, schedule = Update)]
pub fn report_item_use_failure(
    mut failures: MessageReader<ItemUseFailed>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for failure in failures.read() {
        chat.write(ChatHeard {
            gid: 0,
            message: use_failure_message(failure.reason).to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_failure_message_known_codes() {
        assert_eq!(use_failure_message(1), "Item not found.");
        assert_eq!(use_failure_message(2), "You cannot use this item.");
    }

    #[test]
    fn use_failure_message_unknown_code_is_generic() {
        assert_eq!(use_failure_message(3), "You cannot use that right now.");
        assert_eq!(use_failure_message(999), "You cannot use that right now.");
    }

    #[test]
    fn report_item_use_failure_writes_chat_heard() {
        let mut app = App::new();
        app.add_message::<ItemUseFailed>()
            .add_message::<ChatHeard>()
            .add_systems(Update, report_item_use_failure);

        app.world_mut()
            .resource_mut::<Messages<ItemUseFailed>>()
            .write(ItemUseFailed {
                index: 3,
                reason: 2,
            });

        app.update();

        let chat = app.world().resource::<Messages<ChatHeard>>();
        let msgs: Vec<_> = chat.iter_current_update_messages().collect();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].gid, 0);
        assert_eq!(msgs[0].message, "You cannot use this item.");
    }
}
