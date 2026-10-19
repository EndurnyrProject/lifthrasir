use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::announcement::announcement;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::AnnouncementReceived;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_announcements(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<AnnouncementReceived>,
) {
    for msg in incoming.read() {
        if let Body::Announcement(a) = msg.body.clone() {
            out.write(announcement(a));
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
            .add_message::<AnnouncementReceived>()
            .add_systems(Update, zone_drain_announcements);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn announcement_produces_one_announcement_received() {
        let app = drain(vec![(
            WORLD,
            Body::Announcement(net::Announcement {
                text: "server restart in 5 minutes".into(),
                color: 0x00ff00,
                style: net::announcement::Style::Top as i32,
                source_name: "GM".into(),
            }),
        )]);

        let received = app.world().resource::<Messages<AnnouncementReceived>>();
        let events: Vec<_> = received.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "server restart in 5 minutes");
        assert_eq!(
            events[0].style,
            net_contract::events::AnnouncementStyle::Top
        );
    }
}
