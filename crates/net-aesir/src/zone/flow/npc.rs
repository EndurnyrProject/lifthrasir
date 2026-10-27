use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::npc::{npc_dialog, progress_bar};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{NpcDialogReceived, ProgressBarIngress};

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

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_progress_bar(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<ProgressBarIngress>,
) {
    for msg in incoming.read() {
        match &msg.body {
            Body::ProgressBar(p) => {
                out.write(ProgressBarIngress::Started(progress_bar(*p)));
            }
            Body::ProgressBarAbort(p) => {
                out.write(ProgressBarIngress::Aborted { npc_id: p.npc_id });
            }
            _ => {}
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

    #[test]
    fn progress_starts_and_aborts_keep_their_receive_order() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ProgressBarIngress>()
            .add_systems(Update, zone_drain_progress_bar);
        for body in [
            Body::ProgressBar(net::ProgressBar {
                seconds: 3,
                color: 0,
                npc_id: 7,
            }),
            Body::ProgressBarAbort(net::ProgressBarAbort { npc_id: 7 }),
            Body::ProgressBar(net::ProgressBar {
                seconds: 5,
                color: 0xffffff,
                npc_id: 9,
            }),
        ] {
            app.world_mut().write_message(IncomingMessage {
                channel: WORLD,
                body,
            });
        }
        app.update();
        let events: Vec<_> = app
            .world()
            .resource::<Messages<ProgressBarIngress>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], ProgressBarIngress::Started(p) if p.npc_id == 7));
        assert!(matches!(
            events[1],
            ProgressBarIngress::Aborted { npc_id: 7 }
        ));
        assert!(matches!(events[2], ProgressBarIngress::Started(p) if p.npc_id == 9));
    }

    #[test]
    fn progress_bar_produces_one_started_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ProgressBarIngress>()
            .add_systems(Update, zone_drain_progress_bar);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: WORLD,
                body: Body::ProgressBar(net::ProgressBar {
                    seconds: 2,
                    color: 0xffff00,
                    npc_id: 150001,
                }),
            });
        app.update();

        let started = app.world().resource::<Messages<ProgressBarIngress>>();
        let events: Vec<_> = started.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        let ProgressBarIngress::Started(event) = events[0] else {
            panic!("expected start");
        };
        assert_eq!(event.seconds, 2);
        assert_eq!(event.color, 0xffff00);
        assert_eq!(event.npc_id, 150001);
    }
}
