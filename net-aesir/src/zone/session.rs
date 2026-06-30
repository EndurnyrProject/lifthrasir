use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::state::ZoneSession;

use super::QuicZoneState;

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update)]
fn publish_zone_session(zone: Res<QuicZoneState>, mut session: ResMut<ZoneSession>) {
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
        .add_systems(Update, publish_zone_session);

        app.update();

        let session = app.world().resource::<ZoneSession>();
        assert_eq!(session.char_id, 42);
        assert_eq!(session.account_id, 7);
        assert_eq!(session.map_name, "prontera");
    }
}
