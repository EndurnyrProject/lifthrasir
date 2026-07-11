use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::party::{
    party_action_result, party_disbanded, party_info, party_invite_notify,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    PartyActionResulted, PartyDisbanded, PartyInfoReceived, PartyInviteNotified,
};

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_party(
    mut incoming: MessageReader<IncomingMessage>,
    mut info_out: MessageWriter<PartyInfoReceived>,
    mut invite_out: MessageWriter<PartyInviteNotified>,
    mut result_out: MessageWriter<PartyActionResulted>,
    mut disbanded_out: MessageWriter<PartyDisbanded>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::PartyInfo(p) => {
                info_out.write(party_info(p));
            }
            Body::PartyInviteNotify(n) => {
                invite_out.write(party_invite_notify(n));
            }
            Body::PartyActionResult(r) => {
                result_out.write(party_action_result(r));
            }
            Body::PartyDisbanded(d) => {
                disbanded_out.write(party_disbanded(d));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<PartyInfoReceived>()
            .add_message::<PartyInviteNotified>()
            .add_message::<PartyActionResulted>()
            .add_message::<PartyDisbanded>()
            .add_systems(Update, zone_drain_party);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn party_info_produces_one_party_info_received() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::PartyInfo(net::PartyInfo {
                party_id: 7,
                name: "Vikings".into(),
                leader_char_id: 42,
                exp_share: true,
                members: vec![],
            }),
        )]);

        let received = app.world().resource::<Messages<PartyInfoReceived>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].party_id, 7);
        assert_eq!(events[0].name, "Vikings");
    }

    #[test]
    fn party_invite_notify_produces_one_party_invite_notified() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::PartyInviteNotify(net::PartyInviteNotify {
                party_id: 7,
                party_name: "Vikings".into(),
                inviter_name: "Odin".into(),
            }),
        )]);

        let received = app.world().resource::<Messages<PartyInviteNotified>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].inviter_name, "Odin");
    }

    #[test]
    fn party_action_result_produces_one_party_action_resulted() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::PartyActionResult(net::PartyActionResult {
                action: "create".into(),
                success: true,
                error: net::PartyError::None as i32,
            }),
        )]);

        let received = app.world().resource::<Messages<PartyActionResulted>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "create");
        assert!(events[0].success);
    }

    #[test]
    fn party_disbanded_produces_one_party_disbanded() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::PartyDisbanded(net::PartyDisbanded {
                party_id: 7,
                reason: "leader left".into(),
            }),
        )]);

        let received = app.world().resource::<Messages<PartyDisbanded>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].reason, "leader left");
    }
}
