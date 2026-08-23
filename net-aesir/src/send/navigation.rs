use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{NavigationCancelRequested, NavigationRequested};
use net_contract::dto::NavigationTarget;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    NavigationCancel, NavigationCoordinate, NavigationRequest, navigation_request,
};
use crate::zone::{QuicZoneState, ZonePhase};

fn navigation_request_body(command: &NavigationRequested) -> Body {
    let target = match &command.target {
        NavigationTarget::Coord { map, x, y } => {
            navigation_request::Target::Coord(NavigationCoordinate {
                map: map.clone(),
                x: (*x).into(),
                y: (*y).into(),
            })
        }
        NavigationTarget::Map(map) => navigation_request::Target::Map(map.clone()),
        NavigationTarget::Npc(npc) => navigation_request::Target::Npc(npc.clone()),
        NavigationTarget::Monster(monster) => navigation_request::Target::Monster(*monster),
    };

    Body::NavigationRequest(NavigationRequest {
        flag: command.flag,
        hide_window: command.hide_window,
        target: Some(target),
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_navigation_requests(
    mut events: MessageReader<NavigationRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for event in events.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, navigation_request_body(event)) {
            error!("failed to send NavigationRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_navigation_cancels(
    mut events: MessageReader<NavigationCancelRequested>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for _ in events.read() {
        if let Err(error) = zone.send(
            &mut client,
            GAMEPLAY,
            Body::NavigationCancel(NavigationCancel {}),
        ) {
            error!("failed to send NavigationCancel: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::commands::NavigationRequested;
    use net_contract::dto::NavigationTarget;

    #[test]
    fn navigation_request_body_maps_coordinate_target() {
        let body = navigation_request_body(&NavigationRequested {
            target: NavigationTarget::Coord {
                map: "prontera".into(),
                x: 150,
                y: 99,
            },
            flag: 100,
            hide_window: true,
        });

        match body {
            Body::NavigationRequest(request) => {
                assert_eq!(request.flag, 100);
                assert!(request.hide_window);
                assert!(matches!(
                    request.target,
                    Some(navigation_request::Target::Coord(NavigationCoordinate {
                        map,
                        x: 150,
                        y: 99,
                    })) if map == "prontera"
                ));
            }
            other => panic!("expected Body::NavigationRequest, got {other:?}"),
        }
    }

    #[test]
    fn navigation_request_body_maps_map_target() {
        let body = navigation_request_body(&NavigationRequested {
            target: NavigationTarget::Map("geffen".into()),
            flag: 1,
            hide_window: false,
        });

        match body {
            Body::NavigationRequest(request) => {
                assert_eq!(request.flag, 1);
                assert!(!request.hide_window);
                assert!(matches!(
                    request.target,
                    Some(navigation_request::Target::Map(map)) if map == "geffen"
                ));
            }
            other => panic!("expected Body::NavigationRequest, got {other:?}"),
        }
    }

    #[test]
    fn navigation_request_body_maps_npc_target() {
        let body = navigation_request_body(&NavigationRequested {
            target: NavigationTarget::Npc("Kafra Employee".into()),
            flag: 10,
            hide_window: true,
        });

        match body {
            Body::NavigationRequest(request) => {
                assert_eq!(request.flag, 10);
                assert!(request.hide_window);
                assert!(matches!(
                    request.target,
                    Some(navigation_request::Target::Npc(npc)) if npc == "Kafra Employee"
                ));
            }
            other => panic!("expected Body::NavigationRequest, got {other:?}"),
        }
    }

    #[test]
    fn navigation_request_body_maps_monster_target() {
        let body = navigation_request_body(&NavigationRequested {
            target: NavigationTarget::Monster(1_000_002),
            flag: 0,
            hide_window: false,
        });

        match body {
            Body::NavigationRequest(request) => {
                assert_eq!(request.flag, 0);
                assert!(!request.hide_window);
                assert!(matches!(
                    request.target,
                    Some(navigation_request::Target::Monster(1_000_002))
                ));
            }
            other => panic!("expected Body::NavigationRequest, got {other:?}"),
        }
    }

    fn navigation_app() -> App {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>();
        app.init_resource::<QuicZoneState>();
        app.add_message::<NavigationRequested>();
        app.add_message::<NavigationCancelRequested>();
        app.add_systems(Update, (send_navigation_requests, send_navigation_cancels));
        app
    }

    fn assert_no_navigation_frame_was_sent(app: &mut App) {
        let frame = app
            .world_mut()
            .resource_mut::<QuicZoneState>()
            .conn
            .next_frame(Body::NavigationCancel(NavigationCancel {}));
        assert_eq!(crate::envelope::decode(&frame).unwrap().seq, 0);
    }

    #[test]
    fn navigation_requests_are_cleared_without_sending_outside_playing() {
        let mut app = navigation_app();
        app.update();
        app.world_mut().write_message(NavigationRequested {
            target: NavigationTarget::Map("geffen".into()),
            flag: 0,
            hide_window: false,
        });

        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.update();

        assert_no_navigation_frame_was_sent(&mut app);
    }

    #[test]
    fn navigation_cancels_are_cleared_without_sending_outside_playing() {
        let mut app = navigation_app();
        app.update();
        app.world_mut().write_message(NavigationCancelRequested);

        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.update();

        assert_no_navigation_frame_was_sent(&mut app);
    }
}
