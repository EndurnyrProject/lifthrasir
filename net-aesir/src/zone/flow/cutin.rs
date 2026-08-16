use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::CutinDisplayChanged;

use super::super::mapping::cutin::cutin;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_cutins(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<CutinDisplayChanged>,
) {
    for msg in incoming.read() {
        let Body::Cutin(message) = msg.body.clone() else {
            continue;
        };
        if let Some(event) = cutin(message) {
            out.write(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;
    use net_contract::events::CutinPlacement;

    fn cutin_message(image: &str, r#type: u32) -> Body {
        Body::Cutin(net::Cutin {
            image: image.into(),
            r#type,
        })
    }

    #[test]
    fn cutins_drain_in_arrival_order_skipping_unsupported() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<CutinDisplayChanged>()
            .add_systems(Update, zone_drain_cutins);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for body in [
            cutin_message("first", 0),
            cutin_message("bogus", 9),
            Body::SoundEffect(net::SoundEffect {
                name: "effect\\door.wav".into(),
                r#type: 0,
            }),
            cutin_message("stale", 255),
            cutin_message("second", 4),
        ] {
            incoming.write(IncomingMessage {
                channel: GAMEPLAY,
                body,
            });
        }
        app.update();

        let events = app.world().resource::<Messages<CutinDisplayChanged>>();
        let drained: Vec<_> = events.iter_current_update_messages().cloned().collect();
        assert_eq!(
            drained,
            [
                CutinDisplayChanged::Show {
                    image: "first".into(),
                    placement: CutinPlacement::BottomLeft,
                },
                CutinDisplayChanged::Clear,
                CutinDisplayChanged::Show {
                    image: "second".into(),
                    placement: CutinPlacement::CenterChromeless,
                },
            ]
        );
    }
}
