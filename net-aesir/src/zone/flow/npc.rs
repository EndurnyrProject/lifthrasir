use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::npc::npc_dialog;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::NpcDialogReceived;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_npc(
    mut incoming: MessageReader<IncomingMessage>,
    mut dialog: MessageWriter<NpcDialogReceived>,
) {
    for msg in incoming.read() {
        if let Body::NpcDialog(d) = msg.body.clone() {
            dialog.write(npc_dialog(d));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<NpcDialogReceived>()
            .add_systems(Update, zone_drain_npc);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn npc_dialog_produces_one_dialog_received() {
        let app = drain(vec![(
            WORLD,
            Body::NpcDialog(net::NpcDialog {
                npc_id: 150001,
                text: "hello".into(),
                expect: net::npc_dialog::Expect::Next as i32,
                options: vec![],
            }),
        )]);

        let dialog = app.world().resource::<Messages<NpcDialogReceived>>();
        let events: Vec<_> = dialog.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "hello");
        assert_eq!(events[0].npc_id, 150001);
    }

    #[test]
    fn unrelated_body_produces_no_dialog_received() {
        let app = drain(vec![(
            WORLD,
            Body::ChatMessage(net::ChatMessage {
                gid: 150001,
                message: "hello".into(),
            }),
        )]);

        let dialog = app.world().resource::<Messages<NpcDialogReceived>>();
        let events: Vec<_> = dialog.iter_current_update_messages().collect();
        assert_eq!(events.len(), 0);
    }
}
