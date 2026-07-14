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
    if session.char_id != zone.auth.char_id
        || session.account_id != zone.auth.account_id
        || session.map_name != zone.map_name
    {
        generation.0 = generation.0.saturating_add(1);
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
    fn advances_generation_only_when_the_published_session_changes() {
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
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(1)
        );

        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(1)
        );

        app.world_mut().resource_mut::<QuicZoneState>().auth.char_id = 43;
        app.update();
        assert_eq!(
            *app.world().resource::<ZoneSessionGeneration>(),
            ZoneSessionGeneration(2)
        );
    }
}
