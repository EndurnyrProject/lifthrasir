use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use super::QuicZoneState;

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
pub(crate) fn publish_zone_session(
    zone: Res<QuicZoneState>,
    mut session: ResMut<ZoneSession>,
    mut generation: ResMut<ZoneSessionGeneration>,
) {
    if generation.0 != zone.connection_epoch {
        generation.0 = zone.connection_epoch;
    }

    if session.char_id != zone.auth.char_id
        || session.account_id != zone.auth.account_id
        || session.map_name != zone.map_name
    {
        session.char_id = zone.auth.char_id;
        session.account_id = zone.auth.account_id;
        session.map_name = zone.map_name.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::ZoneAuth;

    #[test]
    fn mirrors_auth_identity_into_session() {
        let mut app = App::new();
        app.insert_resource(QuicZoneState {
            auth: ZoneAuth {
                char_id: 42,
                account_id: 7,
                ..Default::default()
            },
            map_name: "prontera".into(),
            ..Default::default()
        })
        .init_resource::<ZoneSession>()
        .init_resource::<ZoneSessionGeneration>()
        .add_systems(Update, publish_zone_session);

        app.update();

        let session = app.world().resource::<ZoneSession>();
        assert_eq!(session.char_id, 42);
        assert_eq!(session.account_id, 7);
        assert_eq!(session.map_name, "prontera");
    }

    #[test]
    fn fresh_same_identity_connection_advances_generation() {
        let mut app = App::new();
        app.init_resource::<QuicZoneState>()
            .init_resource::<ZoneSession>()
            .init_resource::<ZoneSessionGeneration>()
            .add_systems(Update, publish_zone_session);

        let auth = ZoneAuth {
            char_id: 42,
            account_id: 7,
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<QuicZoneState>()
            .start_connecting(auth.clone(), "prontera".into());
        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(1)
        );

        app.world_mut()
            .resource_mut::<QuicZoneState>()
            .start_connecting(auth, "prontera".into());
        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(2)
        );
    }

    #[test]
    fn same_connection_map_change_does_not_advance_generation() {
        let mut app = App::new();
        app.insert_resource(QuicZoneState {
            auth: ZoneAuth {
                char_id: 42,
                account_id: 7,
                ..Default::default()
            },
            map_name: "prontera".into(),
            ..Default::default()
        })
        .init_resource::<ZoneSession>()
        .init_resource::<ZoneSessionGeneration>()
        .add_systems(Update, publish_zone_session);

        app.world_mut()
            .resource_mut::<QuicZoneState>()
            .start_connecting(
                ZoneAuth {
                    char_id: 42,
                    account_id: 7,
                    ..Default::default()
                },
                "prontera".into(),
            );
        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(1)
        );

        app.world_mut().resource_mut::<QuicZoneState>().map_name = "geffen".into();
        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(1)
        );
        assert_eq!(app.world().resource::<ZoneSession>().map_name, "geffen");
    }
}
