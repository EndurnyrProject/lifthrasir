use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::{NavigationEnded, NavigationFailed, RouteUpdated};

use super::super::mapping::navigation::{navigate_to, navigation_ended, navigation_failed};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_navigation(
    mut incoming: MessageReader<IncomingMessage>,
    mut route_updates: MessageWriter<RouteUpdated>,
    mut failures: MessageWriter<NavigationFailed>,
    mut endings: MessageWriter<NavigationEnded>,
) {
    for message in incoming.read() {
        match message.body.clone() {
            Body::NavigateTo(message) => {
                if let Some(event) = navigate_to(message) {
                    route_updates.write(event);
                }
            }
            Body::NavigationFailed(message) => {
                if let Some(event) = navigation_failed(message) {
                    failures.write(event);
                }
            }
            Body::NavigationEnded(message) => {
                if let Some(event) = navigation_ended(message) {
                    endings.write(event);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use net_contract::dto::{NavigationEnd, NavigationFailure};
    use net_contract::events::{NavigationEnded, NavigationFailed, RouteUpdated};

    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::dispatch::IncomingMessage;
    use crate::envelope::Body;
    use crate::proto::aesir::net;

    #[test]
    fn navigation_bodies_drain_to_neutral_messages() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationFailed>()
            .add_message::<NavigationEnded>()
            .add_systems(Update, zone_drain_navigation);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        incoming.write(IncomingMessage {
            channel: GAMEPLAY,
            body: Body::NavigateTo(net::NavigateTo {
                map: "ignored".into(),
                x: 0,
                y: 0,
                flag: 0,
                hide_window: false,
                monster_id: 0,
                legs: vec![net::NavigationLeg {
                    index: 9,
                    map: "prontera".into(),
                    cells: vec![net::NavigationCell { x: 10, y: 20 }],
                    exit_portal: String::new(),
                    next_map: String::new(),
                    arrive: None,
                }],
                destination: Some(net::NavigationCoordinate {
                    map: "prontera".into(),
                    x: 10,
                    y: 20,
                }),
            }),
        });
        incoming.write(IncomingMessage {
            channel: GAMEPLAY,
            body: Body::NavigationFailed(net::NavigationFailed {
                reason: net::NavigationFailureReason::Unreachable as i32,
            }),
        });
        incoming.write(IncomingMessage {
            channel: GAMEPLAY,
            body: Body::NavigationEnded(net::NavigationEnded {
                reason: net::NavigationEndReason::Cancelled as i32,
            }),
        });
        app.update();

        let routes = app.world().resource::<Messages<RouteUpdated>>();
        assert_eq!(routes.iter_current_update_messages().count(), 1);
        let failures = app.world().resource::<Messages<NavigationFailed>>();
        assert_eq!(
            failures
                .iter_current_update_messages()
                .next()
                .map(|event| event.reason),
            Some(NavigationFailure::Unreachable)
        );
        let endings = app.world().resource::<Messages<NavigationEnded>>();
        assert_eq!(
            endings
                .iter_current_update_messages()
                .next()
                .map(|event| event.reason),
            Some(NavigationEnd::Cancelled)
        );
    }

    #[test]
    fn non_navigation_bodies_are_ignored() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<RouteUpdated>()
            .add_message::<NavigationFailed>()
            .add_message::<NavigationEnded>()
            .add_systems(Update, zone_drain_navigation);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::SoundEffect(net::SoundEffect {
                    name: "effect\\door.wav".into(),
                    r#type: 0,
                }),
            });
        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<RouteUpdated>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Messages<NavigationFailed>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Messages<NavigationEnded>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
}
